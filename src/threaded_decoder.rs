extern crate ffmpeg_next as ffmpeg;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::util::frame::video::Video;
use ndarray::{Array, Array3};

static PREFETCH_FRAMES: usize = 1;

pub fn start(path: String) -> Receiver<Option<Array3<u8>>> {
    // Create channels
    let (tx, rx) = mpsc::sync_channel(PREFETCH_FRAMES);

    // Start decoder thread
    thread::spawn(move || {
        decode_video(&path, &tx);

        // Send None to stop iteration
        tx.send(None).unwrap();
        println!("thread finished");
    });

    rx
}

fn decode_video(path: &String, tx: &SyncSender<Option<Array3<u8>>>) -> Result<(), ffmpeg::Error> {
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
            decoder.width(),
            decoder.height(),
            Flags::BILINEAR,
        )?;

        let mut frame_index = 0;

        let mut receive_and_process_decoded_frames =
            |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
                let mut decoded = Video::empty();
                while decoder.receive_frame(&mut decoded).is_ok() {
                    let mut rgb_frame = Video::empty();
                    scaler.run(&decoded, &mut rgb_frame)?;

                    let tensor = Array::from_shape_vec((3, 480, 270), rgb_frame.data(0).to_vec())
                        .unwrap()
                        .into();

                    tx.send(Some(tensor)).unwrap();
                    frame_index += 1;
                }
                Ok(())
            };

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                receive_and_process_decoded_frames(&mut decoder)?;
            }
        }
        decoder.send_eof()?;
        receive_and_process_decoded_frames(&mut decoder)?;
    }

    Ok(())
}
