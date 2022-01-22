use std::thread;

use crossbeam::channel::{bounded, Receiver, Sender};
use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::util::frame::video::Video;

pub fn start(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    prefetch_frames: Option<usize>,
) -> Receiver<Option<Video>> {
    // Create channels
    let (tx, rx) = bounded(prefetch_frames.unwrap_or(1));

    // Start decoder thread
    thread::spawn(move || {
        match decode_video(&path, &tx, height, width) {
            Ok(_) => tx.send(None).unwrap(),
            Err(e) => {
                if e.downcast_ref::<ffmpeg::Error>().is_some() {
                    tx.send(None).unwrap();
                }
            }
        };
    });

    rx
}

fn decode_video(
    path: &String,
    tx: &Sender<Option<Video>>,
    height: Option<u32>,
    width: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    ffmpeg::init()?;
    if let Ok(mut ictx) = input(path) {
        let input = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();

        let mut decoder = input.codec().decoder().video()?;

        let mut scaler = Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGB24,
            width.unwrap_or_else(|| decoder.width()),
            height.unwrap_or_else(|| decoder.height()),
            Flags::BILINEAR,
        )?;

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                process_frames(&mut decoder, &mut scaler, tx)?;
            }
        }
        decoder.send_eof()?;
        process_frames(&mut decoder, &mut scaler, tx)?;
    }

    Ok(())
}

fn process_frames(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut Context,
    tx: &Sender<Option<Video>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoded = Video::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut rgb_frame = Video::empty();
        scaler.run(&decoded, &mut rgb_frame)?;
        tx.send(Some(rgb_frame))?;
    }
    Ok(())
}

pub fn decode_all_video(
    path: &str,
    height: Option<u32>,
    width: Option<u32>,
) -> Result<Vec<Video>, ffmpeg::Error> {
    let mut result = Vec::new();
    ffmpeg::init().unwrap();
    if let Ok(mut ictx) = input(&path.to_owned()) {
        let input = ictx
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)
            .unwrap();
        let video_stream_index = input.index();

        let mut decoder = input.codec().decoder().video().unwrap();

        let mut scaler = Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            Pixel::RGB24,
            width.unwrap_or_else(|| decoder.width()),
            height.unwrap_or_else(|| decoder.height()),
            Flags::BILINEAR,
        )?;

        let mut packets = ictx.packets();

        loop {
            let mut stop = false;

            // Send packet
            if let Some((stream, packet)) = packets.next() {
                if stream.index() == video_stream_index {
                    decoder.send_packet(&packet).unwrap();
                }
            } else {
                decoder.send_eof().unwrap();
                stop = true;
            }

            let mut decoded = Video::empty();

            // Receive frames
            while decoder.receive_frame(&mut decoded).is_ok() {
                let mut rgb_frame = Video::empty();
                scaler.run(&decoded, &mut rgb_frame).unwrap();
                result.push(rgb_frame);
            }

            if stop {
                break;
            }
        }
    }
    Ok(result)
}
