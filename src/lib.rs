use std::ffi::c_int;
use std::path::PathBuf;

use crossbeam_channel::Receiver;
use ffmpeg::util::frame::video::Video;
use pyo3::exceptions::{PyBufferError, PyOSError, PyRuntimeError, PyValueError};
use pyo3::ffi;
use pyo3::prelude::*;

mod decoder;

use decoder::{Error, Message};

impl From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        match err {
            // OSError picks the subclass from errno, e.g. FileNotFoundError.
            Error::Open(path, ffmpeg::Error::Other { errno }) => {
                PyOSError::new_err((errno, ffmpeg::Error::Other { errno }.to_string(), path))
            }
            Error::Open(path, err) => {
                PyValueError::new_err(format!("cannot read video {path:?}: {err}"))
            }
            Error::Decode(err) => PyRuntimeError::new_err(format!("decoding failed: {err}")),
        }
    }
}

/// A decoded frame of packed RGB24 pixels, exposed through the buffer
/// protocol as a writable `(height, width, 3)` array of bytes. The pixels
/// stay in the FFmpeg frame, so `numpy.asarray(frame)` copies nothing.
#[pyclass(module = "iterframes", frozen)]
struct Frame {
    frame: Video,
    shape: [ffi::Py_ssize_t; 3],
    strides: [ffi::Py_ssize_t; 3],
}

impl Frame {
    /// Wrap `frame`, whose rows the decoder has already packed.
    fn new(frame: Video) -> Self {
        let (height, width) = (frame.height() as isize, frame.width() as isize);
        Self {
            frame,
            shape: [height, width, 3],
            strides: [width * 3, 3, 1],
        }
    }
}

#[pymethods]
impl Frame {
    unsafe fn __getbuffer__(
        slf: Bound<'_, Self>,
        view: *mut ffi::Py_buffer,
        flags: c_int,
    ) -> PyResult<()> {
        if view.is_null() {
            return Err(PyBufferError::new_err("view is null"));
        }
        let frame = slf.get();
        let [height, width, channels] = frame.shape;
        // SAFETY: `view` is non-null and owned by the caller. The pixels
        // live as long as `frame`, which `view.obj` keeps alive, and nothing
        // on the Rust side reads or writes them once the frame is wrapped.
        unsafe {
            (*view).buf = (*frame.frame.as_ptr()).data[0].cast();
            (*view).len = height * width * channels;
            (*view).readonly = 0;
            (*view).itemsize = 1;
            (*view).format = if flags & ffi::PyBUF_FORMAT != 0 {
                c"B".as_ptr().cast_mut()
            } else {
                std::ptr::null_mut()
            };
            // Without PyBUF_ND the consumer asked for a flat run of bytes,
            // which the packed rows already are.
            if flags & ffi::PyBUF_ND != 0 {
                (*view).ndim = 3;
                (*view).shape = frame.shape.as_ptr().cast_mut();
            } else {
                (*view).ndim = 1;
                (*view).shape = std::ptr::null_mut();
            }
            (*view).strides = if flags & ffi::PyBUF_STRIDES == ffi::PyBUF_STRIDES {
                frame.strides.as_ptr().cast_mut()
            } else {
                std::ptr::null_mut()
            };
            (*view).suboffsets = std::ptr::null_mut();
            (*view).internal = std::ptr::null_mut();
            (*view).obj = slf.into_any().into_ptr();
        }
        Ok(())
    }
}

/// Iterator over the frames of a video, decoded on a background thread.
///
/// Each item is a `Frame`, which supports the buffer protocol. Use
/// `iterframes.read`, which wraps the frames in NumPy arrays.
#[pyclass(module = "iterframes")]
struct FrameReader {
    frames: Receiver<Message>,
}

#[pymethods]
impl FrameReader {
    #[new]
    #[pyo3(signature = (path, height=None, width=None, prefetch_frames=1))]
    fn new(
        path: PathBuf,
        height: Option<u32>,
        width: Option<u32>,
        prefetch_frames: usize,
    ) -> PyResult<Self> {
        let path = path
            .into_os_string()
            .into_string()
            .map_err(|path| PyValueError::new_err(format!("path is not valid UTF-8: {path:?}")))?;
        Ok(Self {
            frames: decoder::start(path, height, width, prefetch_frames),
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Frame>> {
        // A closed channel means the video is over.
        let Ok(message) = py.detach(|| self.frames.recv()) else {
            return Ok(None);
        };
        Ok(Some(Frame::new(message?)))
    }
}

/// Decode videos with FFmpeg, one frame at a time.
#[pymodule]
mod iterframes {
    use super::*;

    #[pymodule_export]
    use super::{Frame, FrameReader};

    #[pymodule_init]
    fn init(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add("__version__", env!("CARGO_PKG_VERSION"))?;
        module.add("FFMPEG_VERSION", unsafe {
            std::ffi::CStr::from_ptr(ffmpeg::ffi::av_version_info())
                .to_string_lossy()
                .into_owned()
        })?;
        ffmpeg::init()
            .map_err(|err| PyRuntimeError::new_err(format!("cannot initialize FFmpeg: {err}")))?;
        // Errors reach Python as exceptions; keep FFmpeg's warnings off stderr.
        ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Error);
        Ok(())
    }
}
