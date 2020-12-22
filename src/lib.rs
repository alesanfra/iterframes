use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

mod video_reader;


/// Iterates over a video
#[pyfunction]
fn read_video(length: usize) -> PyResult<video_reader::VideoReader> {
    Ok(video_reader::VideoReader { length: length, count: 0 })
}


/// A Python module implemented in Rust.
#[pymodule]
fn iterframes(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read_video, m)?)?;
    Ok(())
}