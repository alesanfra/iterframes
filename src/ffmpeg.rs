//! Safe wrappers over the few parts of FFmpeg that iterframes uses:
//! demuxing, decoding, and converting frames to RGB24. Every call into
//! FFmpeg lives in this module.

use std::ffi::{CStr, CString, c_char, c_int};
use std::fmt;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

/// An FFmpeg error code, always negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(c_int);

impl Error {
    const EOF: Error = Error(sys::AVERROR_EOF);
    const INVALID_DATA: Error = Error(sys::AVERROR_INVALIDDATA);

    /// The POSIX error number, when the error comes from the system rather
    /// than from FFmpeg itself.
    pub fn errno(self) -> Option<c_int> {
        // FFmpeg's own errors are four-letter tags, far below any errno.
        let errno = sys::AVUNERROR(self.0);
        (1..4096).contains(&errno).then_some(errno)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = [0 as c_char; sys::AV_ERROR_MAX_STRING_SIZE];
        // SAFETY: av_strerror writes a NUL-terminated string that fits in
        // the buffer, even for unknown codes.
        let message = unsafe {
            sys::av_strerror(self.0, buffer.as_mut_ptr(), buffer.len());
            CStr::from_ptr(buffer.as_ptr())
        };
        f.write_str(&message.to_string_lossy())
    }
}

/// Turn a negative return value into an error.
fn check(code: c_int) -> Result<c_int, Error> {
    if code < 0 { Err(Error(code)) } else { Ok(code) }
}

/// Keep FFmpeg's warnings off stderr: errors reach Python as exceptions.
pub fn init() {
    // SAFETY: plain setter of a global.
    unsafe { sys::av_log_set_level(sys::AV_LOG_ERROR) };
}

/// Version of the FFmpeg that the module is linked to, such as `9.0.2`.
pub fn version() -> String {
    // SAFETY: av_version_info returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(sys::av_version_info()) }
        .to_string_lossy()
        .into_owned()
}

/// An open media file.
pub struct Input(NonNull<sys::AVFormatContext>);

impl Input {
    pub fn open(path: &str) -> Result<Self, Error> {
        let path = CString::new(path).map_err(|_| Error(sys::AVERROR(libc::EINVAL)))?;
        let mut context = ptr::null_mut();
        // SAFETY: on failure avformat_open_input frees the context and
        // leaves the pointer null; on success `Input` owns it.
        unsafe {
            check(sys::avformat_open_input(
                &mut context,
                path.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
            ))?;
            let input = Self(NonNull::new(context).ok_or(Error(sys::AVERROR(libc::ENOMEM)))?);
            check(sys::avformat_find_stream_info(
                input.0.as_ptr(),
                ptr::null_mut(),
            ))?;
            Ok(input)
        }
    }

    /// Open a decoder for the best video stream of the file.
    pub fn video_decoder(&self) -> Result<Decoder, Error> {
        let mut codec = ptr::null();
        // SAFETY: the context is open; the stream index returned is valid
        // for its `streams` array, and the codec context is freed by
        // `Decoder` whether or not opening it succeeds.
        unsafe {
            let index = check(sys::av_find_best_stream(
                self.0.as_ptr(),
                sys::AVMediaType::AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                &mut codec,
                0,
            ))?;
            let stream = *(*self.0.as_ptr()).streams.add(index as usize);
            let context = NonNull::new(sys::avcodec_alloc_context3(codec))
                .ok_or(Error(sys::AVERROR(libc::ENOMEM)))?;
            let decoder = Decoder {
                context,
                stream: index as usize,
            };
            check(sys::avcodec_parameters_to_context(
                context.as_ptr(),
                (*stream).codecpar,
            ))?;
            // libavcodec decodes on a single thread unless asked otherwise;
            // a count of 0 lets it pick one thread per core.
            (*context.as_ptr()).thread_type = sys::FF_THREAD_FRAME;
            (*context.as_ptr()).thread_count = 0;
            check(sys::avcodec_open2(context.as_ptr(), codec, ptr::null_mut()))?;
            Ok(decoder)
        }
    }

    /// Read the next packet into `packet`. Return `false` at the end of
    /// the file.
    pub fn read(&mut self, packet: &mut Packet) -> Result<bool, Error> {
        loop {
            // SAFETY: both pointers are valid; av_read_frame expects a blank
            // packet, hence the unref.
            let code = unsafe {
                sys::av_packet_unref(packet.0.as_ptr());
                sys::av_read_frame(self.0.as_ptr(), packet.0.as_ptr())
            };
            match check(code) {
                Ok(_) => return Ok(true),
                Err(Error::EOF) => return Ok(false),
                // The demuxer resyncs past a corrupt packet.
                Err(Error::INVALID_DATA) => continue,
                Err(err) => return Err(err),
            }
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        let mut context = self.0.as_ptr();
        // SAFETY: the context was opened by avformat_open_input.
        unsafe { sys::avformat_close_input(&mut context) };
    }
}

pub struct Packet(NonNull<sys::AVPacket>);

impl Packet {
    pub fn new() -> Self {
        // SAFETY: allocation only; a null result means out of memory, which
        // Rust treats as fatal too.
        Self(NonNull::new(unsafe { sys::av_packet_alloc() }).expect("out of memory"))
    }

    pub fn stream(&self) -> usize {
        // SAFETY: the packet is allocated.
        unsafe { (*self.0.as_ptr()).stream_index as usize }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        let mut packet = self.0.as_ptr();
        // SAFETY: allocated by av_packet_alloc.
        unsafe { sys::av_packet_free(&mut packet) };
    }
}

/// An open video decoder.
pub struct Decoder {
    context: NonNull<sys::AVCodecContext>,
    stream: usize,
}

impl Decoder {
    /// Index of the stream this decoder reads.
    pub fn stream(&self) -> usize {
        self.stream
    }

    pub fn send(&mut self, packet: &Packet) -> Result<(), Error> {
        // SAFETY: the decoder is open and the packet allocated.
        check(unsafe { sys::avcodec_send_packet(self.context.as_ptr(), packet.0.as_ptr()) })
            .map(drop)
    }

    /// Signal the end of the stream, so that the buffered frames come out.
    pub fn send_eof(&mut self) -> Result<(), Error> {
        // SAFETY: a null packet is how libavcodec is told to drain.
        check(unsafe { sys::avcodec_send_packet(self.context.as_ptr(), ptr::null()) }).map(drop)
    }

    /// Move the next decoded frame into `frame`. Return `false` when the
    /// decoder needs more input, is drained, or failed on this frame; like
    /// the ffmpeg command, decoding goes on after a broken frame.
    pub fn receive(&mut self, frame: &mut Frame) -> bool {
        // SAFETY: the decoder is open and the frame allocated.
        unsafe { sys::avcodec_receive_frame(self.context.as_ptr(), frame.0.as_ptr()) >= 0 }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let mut context = self.context.as_ptr();
        // SAFETY: allocated by avcodec_alloc_context3.
        unsafe { sys::avcodec_free_context(&mut context) };
    }
}

/// A video frame.
pub struct Frame(NonNull<sys::AVFrame>);

// SAFETY: an AVFrame has no thread affinity, and `Frame` only hands out
// shared access to its pixels through `&self`.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    pub fn new() -> Self {
        // SAFETY: allocation only.
        Self(NonNull::new(unsafe { sys::av_frame_alloc() }).expect("out of memory"))
    }

    fn get(&self) -> &sys::AVFrame {
        // SAFETY: the frame is allocated for as long as `self` lives.
        unsafe { self.0.as_ref() }
    }

    pub fn width(&self) -> u32 {
        self.get().width as u32
    }

    pub fn height(&self) -> u32 {
        self.get().height as u32
    }

    fn format(&self) -> c_int {
        self.get().format
    }

    /// Bytes from the start of one row of `plane` to the next.
    pub fn stride(&self, plane: usize) -> usize {
        self.get().linesize[plane] as usize
    }

    /// Pointer to the first pixel of `plane`. Writing through it is up to
    /// the caller, who must know nothing else reads the pixels meanwhile.
    pub fn data(&self, plane: usize) -> *mut u8 {
        self.get().data[plane]
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let mut frame = self.0.as_ptr();
        // SAFETY: allocated by av_frame_alloc.
        unsafe { sys::av_frame_free(&mut frame) };
    }
}

/// Converts frames of one size and pixel format to packed RGB24.
pub struct Scaler {
    context: NonNull<sys::SwsContext>,
    input: (c_int, u32, u32),
    output: (u32, u32),
}

impl Scaler {
    /// A scaler from frames like `frame` to RGB24 frames of the given size.
    pub fn new(frame: &Frame, width: u32, height: u32) -> Result<Self, Error> {
        let context = NonNull::new(unsafe { sys::sws_alloc_context() })
            .ok_or(Error(sys::AVERROR(libc::ENOMEM)))?;
        let scaler = Self {
            context,
            input: (frame.format(), frame.width(), frame.height()),
            output: (width, height),
        };
        let options: [(&CStr, i64); 8] = [
            (c"srcw", frame.width().into()),
            (c"srch", frame.height().into()),
            (c"src_format", frame.format().into()),
            (c"dstw", width.into()),
            (c"dsth", height.into()),
            (c"dst_format", sys::AVPixelFormat::AV_PIX_FMT_RGB24 as i64),
            (c"sws_flags", SWS_BILINEAR.into()),
            // Converting to RGB takes longer than decoding, which already
            // runs on several threads; 0 is one slice thread per core.
            (c"threads", 0),
        ];
        // SAFETY: the options exist on every SwsContext and take integers;
        // `scaler` frees the context if initializing it fails.
        unsafe {
            for (name, value) in options {
                check(sys::av_opt_set_int(
                    context.as_ptr().cast(),
                    name.as_ptr(),
                    value,
                    0,
                ))?;
            }
            check(sys::sws_init_context(
                context.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
            ))?;
        }
        Ok(scaler)
    }

    /// Whether `frame` has the size and pixel format this scaler expects.
    pub fn accepts(&self, frame: &Frame) -> bool {
        self.input == (frame.format(), frame.width(), frame.height())
    }

    /// Convert `frame`, which the scaler must accept, to a new RGB24 frame
    /// whose rows follow each other with no padding.
    pub fn run(&mut self, frame: &Frame) -> Result<Frame, Error> {
        debug_assert!(self.accepts(frame));
        let rgb = Frame::new();
        // SAFETY: `rgb` gets buffers for the output size and format before
        // sws_scale_frame writes into them; `frame` is a decoded frame of
        // the input size and format.
        unsafe {
            let output = rgb.0.as_ptr();
            (*output).format = sys::AVPixelFormat::AV_PIX_FMT_RGB24 as c_int;
            (*output).width = self.output.0 as c_int;
            (*output).height = self.output.1 as c_int;
            // An alignment of 1 packs the rows, so that Python gets a
            // contiguous array without copying.
            check(sys::av_frame_get_buffer(output, 1))?;
            let input = frame.0.as_ptr();
            check(sys::sws_scale_frame(self.context.as_ptr(), output, input))?;
        }
        Ok(rgb)
    }
}

impl Drop for Scaler {
    fn drop(&mut self) {
        // SAFETY: allocated by sws_getContext.
        unsafe { sys::sws_freeContext(self.context.as_ptr()) };
    }
}

/// `SWS_BILINEAR`, a macro before FFmpeg 8 and an enum from then on; the
/// value is the same in every version.
const SWS_BILINEAR: c_int = 2;
