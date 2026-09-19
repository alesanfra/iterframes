use std::path::PathBuf;

use crossbeam_channel::Receiver;
use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyByteArray;

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

/// Iterator over the frames of a video, decoded on a background thread.
///
/// Each item is a `(buffer, height, width)` tuple, where `buffer` is a
/// `bytearray` of packed RGB24 pixels. Use `iterframes.read`, which wraps
/// the buffers in NumPy arrays.
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

    fn __next__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<(Bound<'py, PyByteArray>, u32, u32)>> {
        // A closed channel means the video is over.
        let Ok(message) = py.detach(|| self.frames.recv()) else {
            return Ok(None);
        };
        let frame = message?;
        let (height, width) = (frame.height(), frame.width());
        let row = width as usize * 3;
        let stride = frame.stride(0);
        let data = frame.data(0);
        // Copy the rows without the padding FFmpeg adds after each of them,
        // so that the array is contiguous.
        let buffer = PyByteArray::new_with(py, row * height as usize, |buffer| {
            for (dst, src) in buffer.chunks_exact_mut(row).zip(data.chunks(stride)) {
                dst.copy_from_slice(&src[..row]);
            }
            Ok(())
        })?;
        Ok(Some((buffer, height, width)))
    }
}

/// Decode videos with FFmpeg, one frame at a time.
#[pymodule]
mod iterframes {
    use super::*;

    #[pymodule_export]
    use super::FrameReader;

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
