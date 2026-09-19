use std::ffi::c_int;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use pyo3::exceptions::{PyBufferError, PyOSError, PyRuntimeError, PyValueError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

mod decoder;
mod dlpack;
mod ffmpeg;

use decoder::{CUDA_FORMATS, Decoded, Device, Error, Message};

impl From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        match err {
            // OSError picks the subclass from errno, e.g. FileNotFoundError.
            Error::Open(path, err) if err.errno().is_some() => {
                PyOSError::new_err((err.errno(), err.to_string(), path))
            }
            Error::Open(path, err) => {
                PyValueError::new_err(format!("cannot read video {path:?}: {err}"))
            }
            Error::Decode(err) => PyRuntimeError::new_err(format!("decoding failed: {err}")),
            Error::Device(name, err) => {
                // FFmpeg's error when libcuda.so.1 is missing is a bare EPERM.
                let hint = match name {
                    "cuda" => " (is an NVIDIA GPU with its driver installed?)",
                    _ => "",
                };
                PyRuntimeError::new_err(format!("cannot open device {name:?}: {err}{hint}"))
            }
            Error::NotOnDevice(reason) => {
                PyRuntimeError::new_err(format!("cannot keep frames on the GPU: {reason}"))
            }
        }
    }
}

/// A decoded frame of packed RGB24 pixels, exposed through the buffer
/// protocol as a writable `(height, width, 3)` array of bytes. The pixels
/// stay in the FFmpeg frame, so `numpy.asarray(frame)` copies nothing.
#[pyclass(module = "iterframes", frozen)]
struct Frame {
    frame: ffmpeg::Frame,
    shape: [ffi::Py_ssize_t; 3],
    strides: [ffi::Py_ssize_t; 3],
}

impl Frame {
    /// Wrap `frame`, an RGB24 frame whose rows follow each other.
    fn new(frame: ffmpeg::Frame) -> Self {
        let (height, width) = (frame.height() as isize, frame.width() as isize);
        debug_assert_eq!(frame.stride(0), width as usize * 3);
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
            (*view).buf = frame.frame.data(0).cast();
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

/// A frame left on the NVIDIA GPU that decoded it, as NV12: a plane of
/// luma, `y`, and a plane of interleaved chroma at half the resolution,
/// `uv`. P010 and P016, for videos of more than 8 bits, have the same
/// layout with 16-bit samples. The planes support DLPack.
#[pyclass(module = "iterframes", frozen)]
struct CudaFrame {
    frame: Arc<ffmpeg::Frame>,
    gpu: c_int,
    format: &'static str,
    bits: u8,
}

impl CudaFrame {
    fn new(frame: ffmpeg::Frame, gpu: c_int) -> Self {
        let format = frame.hardware_format();
        let (_, name, bits) = CUDA_FORMATS
            .into_iter()
            .find(|(known, _, _)| Some(*known) == format)
            .expect("the decoder only sends CUDA frames in these formats");
        Self {
            frame: Arc::new(frame),
            gpu,
            format: name,
            bits,
        }
    }

    fn plane(&self, index: usize, shape: Vec<i64>, mut strides: Vec<i64>) -> Plane {
        // DLPack counts strides in elements; the first one is FFmpeg's
        // padded row size.
        strides.insert(
            0,
            (self.frame.stride(index) / (self.bits as usize / 8)) as i64,
        );
        Plane {
            frame: Arc::clone(&self.frame),
            index,
            gpu: self.gpu,
            bits: self.bits,
            shape,
            strides,
        }
    }
}

#[pymethods]
impl CudaFrame {
    #[getter]
    fn height(&self) -> u32 {
        self.frame.height()
    }

    #[getter]
    fn width(&self) -> u32 {
        self.frame.width()
    }

    /// `"nv12"`, `"p010"`, or `"p016"`.
    #[getter]
    fn format(&self) -> &'static str {
        self.format
    }

    /// The GPU holding the frame, as PyTorch names it, such as `"cuda:0"`.
    #[getter]
    fn device(&self) -> String {
        format!("cuda:{}", self.gpu)
    }

    /// Luma, of shape `(height, width)`.
    #[getter]
    fn y(&self) -> Plane {
        let (height, width) = (self.height() as i64, self.width() as i64);
        self.plane(0, vec![height, width], vec![1])
    }

    /// Chroma, of shape `(height / 2, width / 2, 2)` rounded up, with U
    /// then V in the last axis.
    #[getter]
    fn uv(&self) -> Plane {
        let (height, width) = (self.height() as i64, self.width() as i64);
        self.plane(1, vec![(height + 1) / 2, (width + 1) / 2, 2], vec![2, 1])
    }

    fn __repr__(&self) -> String {
        format!(
            "<CudaFrame {}x{} {} on {}>",
            self.width(),
            self.height(),
            self.format,
            self.device()
        )
    }
}

/// One plane of a `CudaFrame`, for `torch.from_dlpack` and the like. The
/// memory stays valid for as long as the plane, or any tensor made from
/// it, lives.
#[pyclass(module = "iterframes", frozen)]
struct Plane {
    frame: Arc<ffmpeg::Frame>,
    index: usize,
    gpu: c_int,
    bits: u8,
    shape: Vec<i64>,
    strides: Vec<i64>,
}

#[pymethods]
impl Plane {
    #[getter]
    fn shape(&self) -> Vec<i64> {
        self.shape.clone()
    }

    #[pyo3(signature = (*, stream=None, max_version=None, dl_device=None, copy=None))]
    fn __dlpack__<'py>(
        &self,
        py: Python<'py>,
        stream: Option<Bound<'py, PyAny>>,
        max_version: Option<Bound<'py, PyAny>>,
        dl_device: Option<(c_int, c_int)>,
        copy: Option<bool>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // The decoder waited for the copy into the frame, so the pixels are
        // ready for any stream; the capsule is the unversioned kind, which
        // every consumer accepts.
        let _ = (stream, max_version);
        if copy == Some(true) {
            return Err(PyBufferError::new_err("the plane cannot be copied"));
        }
        if dl_device.is_some_and(|device| device != self.__dlpack_device__()) {
            return Err(PyBufferError::new_err(format!(
                "the plane is on cuda:{} only",
                self.gpu
            )));
        }
        dlpack::capsule(
            py,
            dlpack::Tensor {
                frame: Arc::clone(&self.frame),
                data: self.frame.data(self.index),
                gpu: self.gpu,
                bits: self.bits,
                shape: &self.shape,
                strides: &self.strides,
            },
        )
    }

    fn __dlpack_device__(&self) -> (c_int, c_int) {
        (dlpack::CUDA, self.gpu)
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
    #[pyo3(signature = (
        path, height=None, width=None, prefetch_frames=1, device="cpu", on_device=false
    ))]
    fn new(
        path: PathBuf,
        height: Option<u32>,
        width: Option<u32>,
        prefetch_frames: usize,
        device: &str,
        on_device: bool,
    ) -> PyResult<Self> {
        let device = match device {
            "cpu" => Device::Cpu,
            "auto" => Device::Auto,
            name => hardware_devices()
                .find(|(ours, _)| *ours == name)
                .map(|(ours, kind)| Device::Hardware(kind, ours))
                .ok_or_else(|| {
                    let problem = match name {
                        "mps" | "cuda" => "is not available on this platform",
                        _ => "is unknown",
                    };
                    PyValueError::new_err(format!(
                        "device {name:?} {problem}; use \"auto\" or one of {}",
                        devices().join(", ")
                    ))
                })?,
        };
        if on_device && !matches!(device, Device::Hardware(_, "cuda")) {
            return Err(PyValueError::new_err(
                "on_device=True needs device=\"cuda\": only NVIDIA GPUs keep frames",
            ));
        }
        let path = path
            .into_os_string()
            .into_string()
            .map_err(|path| PyValueError::new_err(format!("path is not valid UTF-8: {path:?}")))?;
        Ok(Self {
            frames: decoder::start(path, height, width, device, on_device, prefetch_frames),
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        // A closed channel means the video is over.
        let Ok(message) = py.detach(|| self.frames.recv()) else {
            return Ok(None);
        };
        let frame = match message? {
            Decoded::Rgb(frame) => Py::new(py, Frame::new(frame))?.into_any(),
            Decoded::Cuda { frame, gpu } => Py::new(py, CudaFrame::new(frame, gpu))?.into_any(),
        };
        Ok(Some(frame))
    }
}

/// The hardware devices of this build, by the names PyTorch gives them,
/// which Python users know better than FFmpeg's.
fn hardware_devices() -> impl Iterator<Item = (&'static str, ffmpeg::DeviceType)> {
    [("mps", "videotoolbox"), ("cuda", "cuda")]
        .into_iter()
        .filter_map(|(ours, ffmpeg)| Some((ours, ffmpeg::DeviceType::by_name(ffmpeg)?)))
}

/// The values `device` accepts besides `"auto"`.
fn devices() -> Vec<&'static str> {
    std::iter::once("cpu")
        .chain(hardware_devices().map(|(name, _)| name))
        .collect()
}

/// Decode videos with FFmpeg, one frame at a time.
#[pymodule]
mod iterframes {
    use super::*;

    #[pymodule_export]
    use super::{CudaFrame, Frame, FrameReader, Plane};

    #[pymodule_init]
    fn init(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add("__version__", env!("CARGO_PKG_VERSION"))?;
        module.add("FFMPEG_VERSION", ffmpeg::version())?;
        module.add("DEVICES", PyTuple::new(module.py(), devices())?)?;
        ffmpeg::init();
        Ok(())
    }
}
