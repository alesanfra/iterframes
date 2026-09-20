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
    /// A frame was asked for by a number the video does not have.
    OutOfRange(isize, usize),
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

/// Which frames to decode, and in which order.
pub enum Selection {
    /// Every frame, from the first to the last: the video is read
    /// straight through, with no index and no seeking.
    All,
    /// The frames up to this many, from the first on: the video is read
    /// straight through as well, and stops early.
    First(usize),
    /// These frames, in this order; a negative number counts from the end
    /// of the video. With `approximate`, each of them is read as the
    /// nearest key frame within that many frames of it, which costs one
    /// decoded frame instead of the ones from the key frame on.
    Frames {
        wanted: Vec<isize>,
        approximate: Option<usize>,
    },
    /// The frames from `start` to `stop`, every `step` of them, as a
    /// Python slice: negative bounds count from the end, and `stop` of
    /// `None` is the end of the video.
    Range {
        start: isize,
        stop: Option<isize>,
        step: usize,
    },
}

impl Selection {
    /// The frames asked for, by number, in a video of `frames` frames.
    fn indices(&self, frames: usize) -> Result<Vec<usize>, Error> {
        let resolve = |index: isize| {
            let wrapped = if index < 0 {
                index + frames as isize
            } else {
                index
            };
            usize::try_from(wrapped)
                .ok()
                .filter(|index| *index < frames)
                .ok_or(Error::OutOfRange(index, frames))
        };
        match self {
            Selection::All => Ok((0..frames).collect()),
            Selection::First(count) => Ok((0..frames.min(*count)).collect()),
            Selection::Frames { wanted, .. } => wanted.iter().copied().map(resolve).collect(),
            // Out of range bounds clamp, as a slice of a list does.
            Selection::Range { start, stop, step } => {
                let clamp = |index: isize| {
                    let wrapped = if index < 0 {
                        index + frames as isize
                    } else {
                        index
                    };
                    wrapped.clamp(0, frames as isize) as usize
                };
                let stop = stop.map_or(frames, clamp);
                Ok((clamp(*start)..stop).step_by(*step).collect())
            }
        }
    }
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
/// `selection` is which frames to decode, in which order. The channel
/// holds at most `prefetch` frames, rounded up to whole batches; it is
/// closed after the last frame or right after an error.
#[allow(clippy::too_many_arguments)]
pub fn start(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    device: Device,
    on_device: bool,
    batching: Option<Batching>,
    selection: Selection,
    prefetch: usize,
) -> Receiver<Message> {
    let capacity = batching.map_or(prefetch, |batching| prefetch.div_ceil(batching.size));
    let (tx, rx) = bounded(capacity);
    thread::spawn(move || {
        let result = decode(
            &path, height, width, device, on_device, batching, selection, &tx,
        );
        if let Err(err) = result {
            // Nobody is listening once the reader has been dropped.
            let _ = tx.send(Err(err));
        }
    });
    rx
}

#[allow(clippy::too_many_arguments)]
fn decode(
    path: &str,
    height: Option<u32>,
    width: Option<u32>,
    device: Device,
    on_device: bool,
    batching: Option<Batching>,
    selection: Selection,
    tx: &Sender<Message>,
) -> Result<(), Error> {
    let source = Source::new(path.to_owned(), height, width, device)?;
    let (input, decoder) = source.open()?;
    let mut converter = Converter {
        height,
        width,
        on_device,
        batching,
        batch: None,
        scaler: None,
        remaining: None,
    };
    let sent = match selection {
        Selection::All => all(input, decoder, &mut converter, tx)?,
        // Frames counted from the first need neither the index nor a
        // seek: the video is read straight through and cut short.
        Selection::First(count) => {
            converter.remaining = Some(count);
            count == 0 || all(input, decoder, &mut converter, tx)?
        }
        selection => selected(&source, input, decoder, &mut converter, selection, tx)?,
    };
    if sent && let Some(batch) = converter.last_batch() {
        // Nobody is listening once the reader has been dropped.
        let _ = tx.send(Ok(Decoded::Batch(batch)));
    }
    Ok(())
}

/// Send every frame of the video, in order. Return `false` when the
/// receiver is gone.
fn all(
    mut input: Input,
    mut decoder: Decoder,
    converter: &mut Converter,
    tx: &Sender<Message>,
) -> Result<bool, Error> {
    let mut packet = Packet::new();
    while input.read(&mut packet).map_err(Error::Decode)? {
        if packet.stream() == decoder.stream() {
            decoder.send(&packet).map_err(Error::Decode)?;
            if !converter.drain(&mut decoder, tx)? {
                return Ok(false);
            }
            if converter.is_done() {
                return Ok(true);
            }
        }
    }
    decoder.send_eof().map_err(Error::Decode)?;
    converter.drain(&mut decoder, tx)
}

/// Send the frames that `selection` asks for, in the order it asks for
/// them. Return `false` when the receiver is gone.
fn selected(
    source: &Source,
    input: Input,
    decoder: Decoder,
    converter: &mut Converter,
    selection: Selection,
    tx: &Sender<Message>,
) -> Result<bool, Error> {
    let mut seeker = Seeker::new(source, input, decoder)?;
    let mut indices = selection.indices(seeker.len())?;
    if let Selection::Frames {
        approximate: Some(tolerance),
        ..
    } = selection
    {
        for index in &mut indices {
            *index = seeker.nearest_key(*index, tolerance);
        }
    }
    let mut frame = Frame::new();
    for index in indices {
        seeker.frame(index, &mut frame)?;
        if !converter.send(&mut frame, tx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

struct Converter {
    height: Option<u32>,
    width: Option<u32>,
    on_device: bool,
    batching: Option<Batching>,
    /// The batch being filled.
    batch: Option<Batch>,
    scaler: Option<Scaler>,
    /// How many frames are still wanted, when the video is cut short.
    remaining: Option<usize>,
}

impl Converter {
    /// Send every frame the decoder has ready. Return `false` when the
    /// receiver is gone, so that decoding can stop early.
    fn drain(&mut self, decoder: &mut Decoder, tx: &Sender<Message>) -> Result<bool, Error> {
        let mut decoded = Frame::new();
        while !self.is_done() && decoder.receive(&mut decoded) {
            if !self.send(&mut decoded, tx)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Convert `decoded` and send it, or hold it in the batch being
    /// filled. Return `false` when the receiver is gone.
    fn send(&mut self, decoded: &mut Frame, tx: &Sender<Message>) -> Result<bool, Error> {
        if let Some(remaining) = &mut self.remaining {
            *remaining -= 1;
        }
        let message = if self.on_device {
            // The frame leaves with the message; decode into a new one.
            keep_on_gpu(std::mem::replace(decoded, Frame::new()))?
        } else if let Some(batching) = self.batching {
            match self.add_to_batch(decoded, batching.size)? {
                Some(batch) => Decoded::Batch(batch),
                None => return Ok(true),
            }
        } else {
            Decoded::Rgb(self.convert(decoded)?)
        };
        Ok(tx.send(Ok(message)).is_ok())
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

    /// Whether every frame wanted has been sent.
    fn is_done(&self) -> bool {
        self.remaining == Some(0)
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

/// The file and the decoder settings, which random access opens more than
/// once: a stream with no index, such as raw H.264, is read again from the
/// start instead of seeking.
struct Source {
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    device: Option<HwDevice>,
}

impl Source {
    fn new(
        path: String,
        height: Option<u32>,
        width: Option<u32>,
        device: Device,
    ) -> Result<Self, Error> {
        let device = match device {
            Device::Cpu => None,
            Device::Auto => DeviceType::all()
                .into_iter()
                .find_map(|kind| HwDevice::new(kind).ok()),
            Device::Hardware(kind, name) => {
                Some(HwDevice::new(kind).map_err(|err| Error::Device(name, err))?)
            }
        };
        Ok(Self {
            path,
            height,
            width,
            device,
        })
    }

    /// Open the file and a decoder for its video stream.
    fn open(&self) -> Result<(Input, Decoder), Error> {
        let open = |err| Error::Open(self.path.clone(), err);
        let input = Input::open(&self.path).map_err(open)?;
        let decoder = input
            .video_decoder(self.device.as_ref(), self.width, self.height)
            .map_err(open)?;
        Ok((input, decoder))
    }
}

/// Where a frame of the video is, and whether decoding may start there.
struct Entry {
    pts: i64,
    key: bool,
}

/// A video read frame by number instead of from start to end.
///
/// The file is indexed once, by demuxing every packet without decoding
/// it, which maps each frame to its timestamp and marks the key frames.
/// A frame is then decoded from the key frame before it, or straight on
/// from the frame decoded last when that one comes earlier in the video,
/// so that frames asked for in order cost no more than iterating.
struct Seeker<'a> {
    source: &'a Source,
    input: Input,
    decoder: Decoder,
    packet: Packet,
    /// Every frame of the video, in display order.
    frames: Vec<Entry>,
    /// The numbers of the key frames, which decoding can start from.
    keys: Vec<usize>,
    /// The frame decoded last, when the decoder can go on from it.
    position: Option<usize>,
    /// Whether the demuxer reached the end of the file.
    eof: bool,
    /// Whether the stream seeks; a raw one does not, and is read again
    /// from its first frame instead.
    seekable: bool,
}

impl Seeker<'_> {
    fn new(source: &Source, mut input: Input, decoder: Decoder) -> Result<Seeker<'_>, Error> {
        let mut packet = Packet::new();
        let frames = index(&mut input, decoder.stream(), &mut packet).map_err(Error::Decode)?;
        let keys = frames
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.key)
            .map(|(number, _)| number)
            .collect();
        let mut seeker = Seeker {
            source,
            input,
            decoder,
            packet,
            frames,
            keys,
            position: None,
            eof: false,
            seekable: true,
        };
        if !seeker.frames.is_empty() {
            // The index pass left the demuxer at the end of the file.
            seeker.seek(0)?;
        }
        Ok(seeker)
    }

    /// How many frames the video has.
    fn len(&self) -> usize {
        self.frames.len()
    }

    /// Decode the frame numbered `index` into `frame`, seeking to the key
    /// frame before it unless the decoder can reach it by going on.
    fn frame(&mut self, index: usize, frame: &mut Frame) -> Result<(), Error> {
        let key = self.key_frame_before(index);
        // Going on from where decoding stopped, as long as no key frame in
        // between would make a seek shorter. A stream that does not seek
        // goes on whenever the frame is ahead, since starting over means
        // reading the file again from its first frame.
        let goes_on = self
            .position
            .is_some_and(|position| position < index && (position >= key || !self.seekable));
        if !goes_on {
            self.seek(key)?;
        }
        loop {
            if !self.next(frame)? {
                // The frame is in the index, so the file ended early.
                return Err(Error::Decode(ffmpeg::Error::INVALID_DATA));
            }
            // A frame whose timestamp the index does not know, or repeats
            // from the frame before, keeps its place by counting on.
            let position = match frame.pts().and_then(|pts| self.frame_of(pts)) {
                Some(position) if Some(position) != self.position => position,
                _ => self.position.map_or(0, |position| position + 1),
            };
            self.position = Some(position);
            match position.cmp(&index) {
                std::cmp::Ordering::Less => continue,
                std::cmp::Ordering::Equal => return Ok(()),
                // Seeking lands at or before the frame asked for, so a
                // later one means the timestamps of the file disagree
                // with the order its frames are decoded in.
                std::cmp::Ordering::Greater => {
                    return Err(Error::Decode(ffmpeg::Error::INVALID_DATA));
                }
            }
        }
    }

    /// The key frame nearest to `index`, when no more than `tolerance`
    /// frames away from it, else `index` itself. A tie goes to the key
    /// frame before, which decoding is more likely to reach by going on.
    fn nearest_key(&self, index: usize, tolerance: usize) -> usize {
        let before = self.key_frame_before(index);
        let nearest = match self
            .keys
            .get(self.keys.partition_point(|key| *key <= index))
        {
            Some(after) if after - index < index - before => *after,
            _ => before,
        };
        if nearest.abs_diff(index) <= tolerance {
            nearest
        } else {
            index
        }
    }

    /// The last key frame at or before `index`. The numbers rise, so a
    /// long video costs a binary search rather than a scan.
    fn key_frame_before(&self, index: usize) -> usize {
        match self.keys.partition_point(|key| *key <= index) {
            0 => 0,
            keys => self.keys[keys - 1],
        }
    }

    /// The frame shown at `pts`.
    fn frame_of(&self, pts: i64) -> Option<usize> {
        self.frames
            .binary_search_by_key(&pts, |frame| frame.pts)
            .ok()
    }

    /// Move the demuxer to the frame numbered `index`, which must be a key
    /// frame, and throw away what the decoder holds. A stream that cannot
    /// seek is opened again instead, and read from its first frame.
    fn seek(&mut self, index: usize) -> Result<(), Error> {
        let seeked = self
            .input
            .seek(self.decoder.stream(), self.frames[index].pts)
            .is_ok();
        if seeked {
            self.decoder.flush();
        } else {
            self.seekable = false;
            let (input, decoder) = self.source.open()?;
            self.input = input;
            self.decoder = decoder;
        }
        self.position = None;
        self.eof = false;
        Ok(())
    }

    /// Decode the next frame into `frame`, reading packets until one comes
    /// out. Return `false` at the end of the video.
    fn next(&mut self, frame: &mut Frame) -> Result<bool, Error> {
        loop {
            if self.decoder.receive(frame) {
                return Ok(true);
            }
            if self.eof {
                return Ok(false);
            }
            if self.input.read(&mut self.packet).map_err(Error::Decode)? {
                if self.packet.stream() == self.decoder.stream() {
                    self.decoder.send(&self.packet).map_err(Error::Decode)?;
                }
            } else {
                self.decoder.send_eof().map_err(Error::Decode)?;
                self.eof = true;
            }
        }
    }
}

/// Demux the video stream of `input` and return its frames in display
/// order. Nothing is decoded, so this costs one pass over the packets.
fn index(
    input: &mut Input,
    stream: usize,
    packet: &mut Packet,
) -> Result<Vec<Entry>, ffmpeg::Error> {
    let mut frames = Vec::new();
    let mut last = 0;
    while input.read(packet)? {
        if packet.stream() != stream {
            continue;
        }
        // A file without timestamps still has its frames in order, so the
        // one after `last` keeps them apart.
        let pts = packet.pts().or_else(|| packet.dts()).unwrap_or(last + 1);
        last = pts;
        frames.push(Entry {
            pts,
            key: packet.is_key(),
        });
    }
    // Packets come in decode order, which B-frames make differ from the
    // order the frames are shown in.
    frames.sort_by_key(|frame| frame.pts);
    // A file cut in the middle of a group of pictures starts with frames
    // that no key frame can be decoded from, and that reading the video
    // from start to end does not return either; leaving them out of the
    // index numbers the frames the same way.
    if let Some(first) = frames.iter().position(|frame| frame.key) {
        frames.drain(..first);
    }
    Ok(frames)
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
