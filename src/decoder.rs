use std::thread;

use crossbeam_channel::{Receiver, Sender, bounded};
use ffmpeg::codec::threading;
use ffmpeg::format::{Pixel, input};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{Context as Scaler, Flags};
use ffmpeg::util::frame::video::Video;

pub enum Error {
    /// The file could not be opened or has no video stream.
    Open(String, ffmpeg::Error),
    /// The file was opened, but decoding failed half way.
    Decode(ffmpeg::Error),
}

pub type Message = Result<Video, Error>;

/// Decode `path` on a background thread and return the channel that
/// receives its RGB24 frames. The channel holds at most `prefetch` frames;
/// it is closed after the last frame or right after an error.
pub fn start(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    prefetch: usize,
) -> Receiver<Message> {
    let (tx, rx) = bounded(prefetch);
    thread::spawn(move || {
        if let Err(err) = decode(&path, height, width, &tx) {
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
    tx: &Sender<Message>,
) -> Result<(), Error> {
    let open = |err| Error::Open(path.to_owned(), err);
    let mut ictx = input(path).map_err(open)?;
    let stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)
        .map_err(open)?;
    let index = stream.index();

    let mut context =
        ffmpeg::codec::context::Context::from_parameters(stream.parameters()).map_err(open)?;
    // libavcodec decodes on a single thread unless asked otherwise;
    // a count of 0 lets it pick one thread per core.
    context.set_threading(threading::Config::kind(threading::Type::Frame));
    let mut decoder = context.decoder().video().map_err(open)?;

    let mut converter = Converter {
        height,
        width,
        scaler: None,
    };
    for (stream, packet) in ictx.packets() {
        if stream.index() == index {
            decoder.send_packet(&packet).map_err(Error::Decode)?;
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
    fn drain(
        &mut self,
        decoder: &mut ffmpeg::decoder::Video,
        tx: &Sender<Message>,
    ) -> Result<bool, Error> {
        let mut decoded = Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgb = Video::empty();
            self.scaler(&decoded)?
                .run(&decoded, &mut rgb)
                .map_err(Error::Decode)?;
            if tx.send(Ok(rgb)).is_err() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The scaler for `frame`, rebuilt whenever the input size or pixel
    /// format changes within the stream.
    fn scaler(&mut self, frame: &Video) -> Result<&mut Scaler, Error> {
        let stale = self.scaler.as_ref().is_none_or(|scaler| {
            let input = scaler.input();
            (input.format, input.width, input.height)
                != (frame.format(), frame.width(), frame.height())
        });
        if stale {
            let scaler = Scaler::get(
                frame.format(),
                frame.width(),
                frame.height(),
                Pixel::RGB24,
                self.width.unwrap_or(frame.width()),
                self.height.unwrap_or(frame.height()),
                Flags::BILINEAR,
            )
            .map_err(Error::Decode)?;
            self.scaler = Some(scaler);
        }
        Ok(self.scaler.as_mut().expect("scaler was just set"))
    }
}
