use crossbeam::channel::Receiver;
use ffmpeg::frame::Video;
use pyo3::exceptions::PyRuntimeError;
use pyo3::iter::IterNextOutput;
use pyo3::prelude::*;
use pyo3::types::PyByteArray;
use pyo3::PyIterProtocol;

mod decoder;

#[pyclass(module = "iterframes")]
pub struct Frame {
    #[pyo3(get)]
    buffer: Py<PyByteArray>,

    #[pyo3(get)]
    height: usize,

    #[pyo3(get)]
    width: usize,

    #[pyo3(get)]
    stride: usize,
}

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
}

#[pyproto]
impl PyIterProtocol for FrameReader {
    fn __iter__(self_: PyRef<Self>) -> PyRef<Self> {
        self_
    }

    fn __next__(self_: PyRefMut<Self>) -> IterNextOutput<Frame, ()> {
        if let Ok(Some(frame)) = self_.channel.recv() {
            let frame = Frame {
                buffer: PyByteArray::new(self_.py(), &frame.data(0)).into(),
                height: frame.height() as usize,
                width: frame.width() as usize,
                stride: frame.stride(0),
            };
            IterNextOutput::Yield(frame)
        } else {
            IterNextOutput::Return(())
        }
    }
}

#[pyfunction]
fn read_batch(
    py: Python,
    path: &str,
    height: Option<u32>,
    width: Option<u32>,
) -> PyResult<Vec<Frame>> {
    match decoder::decode_all_video(path, height, width) {
        Ok(frames) => Ok(frames
            .into_iter()
            .map(|frame| Frame {
                buffer: PyByteArray::new(py, &frame.data(0)).into(),
                height: frame.height() as usize,
                width: frame.width() as usize,
                stride: frame.stride(0) as usize,
            })
            .rev()
            .collect::<Vec<Frame>>()),
        Err(_) => Err(PyRuntimeError::new_err("Error on decoding")),
    }
}

/// Process quickly all videos in the world, one frame at time.
#[pymodule]
fn iterframes(_py: Python, module: &PyModule) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add_class::<FrameReader>()?;
    module.add_class::<Frame>()?;
    module.add_function(wrap_pyfunction!(read_batch, module)?)?;
    Ok(())
}
