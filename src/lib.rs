use ndarray::Array3;
use numpy::{IntoPyArray, PyArray3};
use pyo3::class::iter::IterNextOutput;
use pyo3::prelude::*;
use pyo3::{wrap_pyfunction, PyIterProtocol};
use std::sync::mpsc::Receiver;

mod threaded_decoder;

#[pyclass]
pub struct FrameIterator {
    channel: Receiver<Option<Array3<u8>>>,
}

#[pyproto]
impl PyIterProtocol for FrameIterator {
    fn __iter__(py_self: PyRef<Self>) -> PyRef<Self> {
        py_self
    }

    fn __next__(mut py_self: PyRefMut<Self>) -> IterNextOutput<Py<PyArray3<u8>>, String> {
        match py_self.channel.recv() {
            Ok(Some(tensor)) => IterNextOutput::Yield(tensor.into_pyarray(py_self.py()).to_owned()),
            _ => IterNextOutput::Return(String::from("Ended")),
        }
    }
}

/// Iterates over a video
#[pyfunction]
fn read(path: String) -> PyResult<FrameIterator> {
    let channel = threaded_decoder::start(path);
    let iterator = FrameIterator { channel };
    Ok(iterator)
}

/// Process quickly all videos in the world, one frame at time.
#[pymodule]
fn iterframes(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read, m)?)?;
    Ok(())
}
