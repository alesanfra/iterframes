use std::sync::mpsc::Receiver;

use ffmpeg::frame::Video;
use numpy::{PyArray, PyArray3};
use pyo3::prelude::*;

mod decoder;

#[pyclass(module = "iterframes")]
pub struct FrameReader {
    channel: Receiver<Option<Video>>,
}

#[pymethods]
impl FrameReader {
    #[new]
    fn new(
        path: String,
        height: Option<u32>,
        width: Option<u32>,
        prefetch_frames: Option<usize>,
    ) -> Self {
        Self {
            channel: decoder::start(path, height, width, prefetch_frames),
        }
    }

    fn next(self_: PyRefMut<Self>) -> PyResult<Option<(Py<PyArray3<u8>>, usize, usize, usize)>> {
        if let Ok(Some(frame)) = self_.channel.recv() {
            let height = frame.height() as usize;
            let width = frame.width() as usize;
            let stride = frame.stride(0) as usize;

            let tensor = PyArray::from_slice(self_.py(), frame.data(0))
                .reshape((height, stride / 3, 3))
                .unwrap()
                .into();
            Ok(Some((tensor, height, width, stride)))
        } else {
            Ok(None)
        }
    }
}

/// Process quickly all videos in the world, one frame at time.
#[pymodule]
fn iterframes(_py: Python, module: &PyModule) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_class::<FrameReader>()?;
    Ok(())
}
