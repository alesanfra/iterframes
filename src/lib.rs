use std::sync::mpsc::Receiver;

use ffmpeg::frame::Video;
use numpy::{PyArray, PyArray3};
use pyo3::prelude::*;
use pyo3::{wrap_pyfunction, PyIterProtocol};

mod decoder;

#[pyclass(module = "iterframes")]
pub struct FrameIterator {
    channel: Receiver<Option<Video>>,
}

#[pyproto]
impl PyIterProtocol for FrameIterator {
    fn __iter__(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(slf: PyRefMut<Self>) -> Option<Py<PyArray3<u8>>> {
        if let Ok(Some(frame)) = slf.channel.recv() {
            let tensor = PyArray::from_slice(slf.py(), frame.data(0))
                .reshape((frame.height() as usize, frame.stride(0) / 3 as usize, 3))
                .unwrap()
                .into();
            Some(tensor)
        } else {
            None
        }
    }
}

/// Iterates over a video
///
/// Returns:
///     Frame with shape HxWx3.
#[pyfunction]
fn read(
    path: String,
    height: Option<u32>,
    width: Option<u32>,
    prefetch_frames: Option<usize>,
) -> FrameIterator {
    FrameIterator {
        channel: decoder::start(path, height, width, prefetch_frames),
    }
}

/// Process quickly all videos in the world, one frame at time.
#[pymodule]
fn iterframes(_py: Python, module: &PyModule) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_function(wrap_pyfunction!(read, module)?)?;
    module.add_class::<FrameIterator>()?;
    Ok(())
}
