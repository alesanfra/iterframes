use std::ffi::c_int;
use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};

use crate::ffmpeg::{self, Buffer, Decoder, DeviceType, Frame, HwDevice, Input, Packet, Scaler};

pub enum Error {
    /// The file could not be opened or has no video stream.
    Open(String, ffmpeg::Error),
    /// The file was opened, but decoding failed half way.
    Decode(ffmpeg::Error),
    /// The device asked for by name, such as `"cuda"`, cannot be opened.
    Device(&'static str, ffmpeg::Error),
    /// Frames were asked to stay on the GPU, but this one cannot.
    NotOnDevice(&'static str),
}

/// Where to decode.
#[derive(Clone, Copy)]
pub enum Device {
    Cpu,
    /// On the first hardware device that opens, else on the CPU.
    Auto,
    /// On a device of this kind, called `name` in Python; failing to open
    /// one is an error.
    Hardware(DeviceType, &'static str),
}

/// A decoded frame.
pub enum Decoded {
    /// Packed RGB24 pixels in memory.
    Rgb(Frame),
    /// NV12 (or P010, P016) pixels on the CUDA GPU numbered `gpu`.
    Cuda { frame: Frame, gpu: c_int },
    /// Packed RGB24 frames of one size, one after the other.
    Batch(Batch),
}

/// Frames decoded into one buffer, the way Python reads them: an array of
/// shape `(frames, height, width, 3)`.
pub struct Batch {
    pub buffer: Buffer,
    pub frames: usize,
    pub height: u32,
    pub width: u32,
}

impl Batch {
    fn new(capacity: usize, width: u32, height: u32) -> Result<Self, Error> {
        let len = (height as usize * width as usize * 3)
            .checked_mul(capacity)
            .ok_or(Error::Decode(ffmpeg::Error::OUT_OF_MEMORY))?;
        Ok(Self {
            buffer: Buffer::new(len).map_err(Error::Decode)?,
            frames: 0,
            height,
            width,
        })
    }

    fn frame_len(&self) -> usize {
        self.height as usize * self.width as usize * 3
    }

    fn is_full(&self) -> bool {
        (self.frames + 1) * self.frame_len() > self.buffer.len()
    }
}

/// How to group frames into batches.
#[derive(Clone, Copy)]
pub struct Batching {
    pub size: usize,
    /// Whether to drop the last batch when the video ends before it is
    /// full, instead of sending it with fewer frames.
    pub drop_last: bool,
}

/// The pixel formats that frames left on the GPU may have: a plane of
/// luma, then one of interleaved chroma at half the width and height.
pub const CUDA_FORMATS: [(ffmpeg::PixelFormat, &str, u8); 3] = [
    (ffmpeg::PixelFormat::AV_PIX_FMT_NV12, "nv12", 8),
    (ffmpeg::PixelFormat::AV_PIX_FMT_P010LE, "p010", 16),
    (ffmpeg::PixelFormat::AV_PIX_FMT_P016LE, "p016", 16),
];

pub type Message = Result<Decoded, Error>;

/// Decode `path` on a background thread and return the channel that
/// receives its frames: RGB24 in memory, grouped in batches with
/// `batching`, or with `on_device`, left on the CUDA GPU that decoded them.
/// The channel holds at most `prefetch` frames, rounded up to whole
/// batches; it is closed after the last frame or right after an error.
pub fn start(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    device: Device,
    on_device: bool,
    batching: Option<Batching>,
    prefetch: usize,
) -> Receiver<Message> {
    let capacity = batching.map_or(prefetch, |batching| prefetch.div_ceil(batching.size));
    let (tx, rx) = bounded(capacity);
    thread::spawn(move || {
        let result = decode(&path, height, width, device, on_device, batching, &tx);
        if let Err(err) = result {
            // Nobody is listening once the reader has been dropped.
            let _ = tx.send(Err(err));
        }
    });
    rx
}

fn decode(
    path: &str,
    height: Option<u32>,
    width: Option<u32>,
    device: Device,
    on_device: bool,
    batching: Option<Batching>,
    tx: &Sender<Message>,
) -> Result<(), Error> {
    let open = |err| Error::Open(path.to_owned(), err);
    let mut input = Input::open(path).map_err(open)?;
    let device = match device {
        Device::Cpu => None,
        Device::Auto => DeviceType::all()
            .into_iter()
            .find_map(|kind| HwDevice::new(kind).ok()),
        Device::Hardware(kind, name) => {
            Some(HwDevice::new(kind).map_err(|err| Error::Device(name, err))?)
        }
    };
    let mut decoder = input
        .video_decoder(device.as_ref(), width, height)
        .map_err(open)?;

    let mut converter = Converter {
        height,
        width,
        on_device,
        batching,
        batch: None,
        scaler: None,
    };
    let mut packet = Packet::new();
    while input.read(&mut packet).map_err(Error::Decode)? {
        if packet.stream() == decoder.stream() {
            decoder.send(&packet).map_err(Error::Decode)?;
            if !converter.drain(&mut decoder, tx)? {
                return Ok(());
            }
        }
    }
    decoder.send_eof().map_err(Error::Decode)?;
    if converter.drain(&mut decoder, tx)?
        && let Some(batch) = converter.last_batch()
    {
        // Nobody is listening once the reader has been dropped.
        let _ = tx.send(Ok(Decoded::Batch(batch)));
    }
    Ok(())
}

struct Converter {
    height: Option<u32>,
    width: Option<u32>,
    on_device: bool,
    batching: Option<Batching>,
    /// The batch being filled.
    batch: Option<Batch>,
    scaler: Option<Scaler>,
}

impl Converter {
    /// Send every frame the decoder has ready. Return `false` when the
    /// receiver is gone, so that decoding can stop early.
    fn drain(&mut self, decoder: &mut Decoder, tx: &Sender<Message>) -> Result<bool, Error> {
        let mut decoded = Frame::new();
        while decoder.receive(&mut decoded) {
            let message = if self.on_device {
                // The frame leaves with the message; decode into a new one.
                keep_on_gpu(std::mem::replace(&mut decoded, Frame::new()))?
            } else if let Some(batching) = self.batching {
                match self.add_to_batch(&decoded, batching.size)? {
                    Some(batch) => Decoded::Batch(batch),
                    None => continue,
                }
            } else {
                Decoded::Rgb(self.convert(&decoded)?)
            };
            if tx.send(Ok(message)).is_err() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn convert(&mut self, decoded: &Frame) -> Result<Frame, Error> {
        let downloaded = download(decoded)?;
        let frame = downloaded.as_ref().unwrap_or(decoded);
        self.scaler(frame)?.run(frame).map_err(Error::Decode)
    }

    /// Convert `decoded` into the batch being filled, and return the batch
    /// once it holds `size` frames.
    fn add_to_batch(&mut self, decoded: &Frame, size: usize) -> Result<Option<Batch>, Error> {
        let downloaded = download(decoded)?;
        let frame = downloaded.as_ref().unwrap_or(decoded);
        let (width, height) = self.scaler(frame)?.output();
        // A batch is a single array, so frames that change size within the
        // stream are scaled to the size of the first one.
        self.width = Some(width);
        self.height = Some(height);
        let mut batch = match self.batch.take() {
            Some(batch) => batch,
            None => Batch::new(size, width, height)?,
        };
        let offset = batch.frames * batch.frame_len();
        self.scaler(frame)?
            .run_into(frame, &batch.buffer, offset)
            .map_err(Error::Decode)?;
        batch.frames += 1;
        if batch.is_full() {
            Ok(Some(batch))
        } else {
            self.batch = Some(batch);
            Ok(None)
        }
    }

    /// The batch left unfilled at the end of the video, unless it is to be
    /// dropped.
    fn last_batch(&mut self) -> Option<Batch> {
        let drop_last = self.batching.is_some_and(|batching| batching.drop_last);
        self.batch.take().filter(|_| !drop_last)
    }

    /// The scaler for `frame`, rebuilt whenever the input size or pixel
    /// format changes within the stream.
    fn scaler(&mut self, frame: &Frame) -> Result<&mut Scaler, Error> {
        if !self
            .scaler
            .as_ref()
            .is_some_and(|scaler| scaler.accepts(frame))
        {
            let scaler = Scaler::new(
                frame,
                self.width.unwrap_or(frame.width()),
                self.height.unwrap_or(frame.height()),
            )
            .map_err(Error::Decode)?;
            self.scaler = Some(scaler);
        }
        Ok(self.scaler.as_mut().expect("scaler was just set"))
    }
}

/// The pixels of `decoded` copied or mapped from the hardware that holds
/// them, or `None` when they are in memory already.
fn download(decoded: &Frame) -> Result<Option<Frame>, Error> {
    decoded
        .is_hardware()
        .then(|| decoded.download().map_err(Error::Decode))
        .transpose()
}

/// Check that `frame` is a CUDA frame in a format Python can use, and wait
/// until its pixels are in place.
fn keep_on_gpu(frame: Frame) -> Result<Decoded, Error> {
    let format = frame.hardware_format().ok_or(Error::NotOnDevice(
        "the video's codec is not decoded on the GPU",
    ))?;
    if !CUDA_FORMATS.iter().any(|(known, _, _)| *known == format) {
        return Err(Error::NotOnDevice(
            "the GPU decodes the video to a pixel format other than NV12, P010, or P016",
        ));
    }
    let gpu = ffmpeg::cuda::synchronize(&frame).map_err(|err| Error::Device("cuda", err))?;
    Ok(Decoded::Cuda { frame, gpu })
}
