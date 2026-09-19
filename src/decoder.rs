use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};

use crate::ffmpeg::{self, Decoder, DeviceType, Frame, HwDevice, Input, Packet, Scaler};

pub enum Error {
    /// The file could not be opened or has no video stream.
    Open(String, ffmpeg::Error),
    /// The file was opened, but decoding failed half way.
    Decode(ffmpeg::Error),
    /// The hardware decoder asked for by name cannot be used.
    Hardware(DeviceType, ffmpeg::Error),
}

/// Where to decode.
#[derive(Clone, Copy)]
pub enum Hwaccel {
    /// On the CPU.
    None,
    /// On the first hardware device that opens, else on the CPU.
    Auto,
    /// On a device of this kind; failing to open one is an error.
    Device(DeviceType),
}

pub type Message = Result<Frame, Error>;

/// Decode `path` on a background thread and return the channel that
/// receives its RGB24 frames. The channel holds at most `prefetch` frames;
/// it is closed after the last frame or right after an error.
pub fn start(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    hwaccel: Hwaccel,
    prefetch: usize,
) -> Receiver<Message> {
    let (tx, rx) = bounded(prefetch);
    thread::spawn(move || {
        if let Err(err) = decode(&path, height, width, hwaccel, &tx) {
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
    hwaccel: Hwaccel,
    tx: &Sender<Message>,
) -> Result<(), Error> {
    let open = |err| Error::Open(path.to_owned(), err);
    let mut input = Input::open(path).map_err(open)?;
    let device = match hwaccel {
        Hwaccel::None => None,
        Hwaccel::Auto => DeviceType::all()
            .into_iter()
            .find_map(|kind| HwDevice::new(kind).ok()),
        Hwaccel::Device(kind) => {
            Some(HwDevice::new(kind).map_err(|err| Error::Hardware(kind, err))?)
        }
    };
    let mut decoder = input
        .video_decoder(device.as_ref(), width, height)
        .map_err(open)?;

    let mut converter = Converter {
        height,
        width,
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
    scaler: Option<Scaler>,
}

impl Converter {
    /// Send every frame the decoder has ready. Return `false` when the
    /// receiver is gone, so that decoding can stop early.
    fn drain(&mut self, decoder: &mut Decoder, tx: &Sender<Message>) -> Result<bool, Error> {
        let mut decoded = Frame::new();
        while decoder.receive(&mut decoded) {
            let downloaded;
            let frame = if decoded.is_hardware() {
                downloaded = decoded.download().map_err(Error::Decode)?;
                &downloaded
            } else {
                &decoded
            };
            let rgb = self.scaler(frame)?.run(frame).map_err(Error::Decode)?;
            if tx.send(Ok(rgb)).is_err() {
                return Ok(false);
            }
        }
        Ok(true)
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
