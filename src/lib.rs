extern crate ffmpeg_next as ffmpeg;

use std::sync::mpsc::Receiver;

use ffmpeg::frame::Video;
use numpy::{PyArray, PyArray3};
use pyo3::class::iter::IterNextOutput;
use pyo3::prelude::*;
use pyo3::{wrap_pyfunction, PyIterProtocol};

mod decoder;

#[pyclass(module = "iterframes")]
pub struct FrameIterator {
    channel: Receiver<Option<Video>>,
}

#[pyproto]
impl PyIterProtocol for FrameIterator {
    fn __iter__(py_self: PyRef<Self>) -> PyRef<Self> {
        py_self
    }

    fn __next__(py_self: PyRefMut<Self>) -> IterNextOutput<Py<PyArray3<u8>>, String> {
        match py_self.channel.recv() {
            Ok(Some(frame)) => {
                let tensor = PyArray::from_slice(py_self.py(), frame.data(0))
                    .reshape((frame.height() as usize, frame.width() as usize, 3))
                    .unwrap()
                    .into();
                IterNextOutput::Yield(tensor)
            }
            _ => IterNextOutput::Return(String::from("Ended")),
        }
    }
}

/// Iterates over a video
///
/// Returns:
///     Frame with shape HxWx3.
#[pyfunction]
fn read(path: String, prefetch_frames: Option<usize>) -> PyResult<FrameIterator> {
    let channel = decoder::start(path, prefetch_frames);
    let iterator = FrameIterator { channel };
    Ok(iterator)
}

/// Process quickly all videos in the world, one frame at time.
#[pymodule]
fn iterframes(_py: Python, module: &PyModule) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_function(wrap_pyfunction!(read, module)?)?;
    module.add_class::<FrameIterator>()?;
    Ok(())
}
