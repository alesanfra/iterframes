use std::sync::mpsc;
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread;

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
    let bound = prefetch_frames.unwrap_or(1);
    let (tx, rx) = mpsc::sync_channel(bound);

    // Start decoder thread
    thread::spawn(move || {
        match decode_video(&path, &tx, height, width) {
            Ok(_) => tx.send(None).unwrap(),
            Err(e) => {
                if let Some(_) = e.downcast_ref::<ffmpeg::Error>() {
                    tx.send(None).unwrap();
                }
            }
        };
    });

    rx
}

fn decode_video(
    path: &String,
    tx: &SyncSender<Option<Video>>,
    height: Option<u32>,
    width: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    ffmpeg::init().unwrap();

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
            width.unwrap_or(decoder.width()),
            height.unwrap_or(decoder.height()),
            Flags::BILINEAR,
        )?;

        let mut frame_index = 0;

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                process_frames(&mut frame_index, &mut decoder, &mut scaler, tx)?;
            }
        }
        decoder.send_eof()?;
        process_frames(&mut frame_index, &mut decoder, &mut scaler, tx)?;
    }

    Ok(())
}

fn process_frames(
    frame_index: &mut i32,
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut Context,
    tx: &SyncSender<Option<Video>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoded = Video::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut rgb_frame = Video::empty();
        scaler.run(&decoded, &mut rgb_frame)?;
        tx.send(Some(rgb_frame))?;
        *frame_index += 1;
    }
    Ok(())
}
