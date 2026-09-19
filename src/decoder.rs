use std::ffi::c_int;
use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};

use crate::ffmpeg::{self, Decoder, DeviceType, Frame, HwDevice, Input, Packet, Scaler};

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
/// receives its frames: RGB24 in memory, or with `on_device`, left on the
/// CUDA GPU that decoded them. The channel holds at most `prefetch` frames;
/// it is closed after the last frame or right after an error.
pub fn start(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    device: Device,
    on_device: bool,
    prefetch: usize,
) -> Receiver<Message> {
    let (tx, rx) = bounded(prefetch);
    thread::spawn(move || {
        if let Err(err) = decode(&path, height, width, device, on_device, &tx) {
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
    converter.drain(&mut decoder, tx)?;
    Ok(())
}

struct Converter {
    height: Option<u32>,
    width: Option<u32>,
    on_device: bool,
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
        let downloaded;
        let frame = if decoded.is_hardware() {
            downloaded = decoded.download().map_err(Error::Decode)?;
            &downloaded
        } else {
            decoded
        };
        self.scaler(frame)?.run(frame).map_err(Error::Decode)
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
