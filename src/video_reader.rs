use pyo3::class::iter::IterNextOutput;
use pyo3::prelude::*;
use pyo3::PyIterProtocol;

#[pyclass]
pub struct VideoReader {
    pub length: usize,
    pub count: usize,
}

#[pyproto]
impl PyIterProtocol for VideoReader {
    fn __iter__(this: PyRef<Self>) -> PyRef<Self> {
        this
    }

    fn __next__(mut this: PyRefMut<Self>) -> IterNextOutput<usize, &'static str> {
        if this.count < this.length {
            this.count += 1;
            IterNextOutput::Yield(this.count)
        } else {
            IterNextOutput::Return("Ended")
        }
    }
}