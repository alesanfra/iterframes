//! The DLPack protocol, through which PyTorch, CuPy, JAX, and others take
//! a GPU array without copying it: `__dlpack__` returns a capsule named
//! `dltensor` around a `DLManagedTensor`, whose deleter the consumer calls
//! once it no longer needs the memory.
//!
//! See <https://dmlc.github.io/dlpack/latest/python_spec.html>.

use std::ffi::{c_int, c_void};
use std::ptr;
use std::sync::Arc;

use pyo3::ffi;
use pyo3::prelude::*;

use crate::ffmpeg::Frame;

/// `kDLCUDA`
pub const CUDA: c_int = 2;
/// `kDLUInt`
const UINT: u8 = 1;

#[repr(C)]
struct DLDevice {
    device_type: c_int,
    device_id: c_int,
}

#[repr(C)]
struct DLDataType {
    code: u8,
    bits: u8,
    lanes: u16,
}

#[repr(C)]
struct DLTensor {
    data: *mut c_void,
    device: DLDevice,
    ndim: c_int,
    dtype: DLDataType,
    shape: *mut i64,
    /// In elements, not bytes.
    strides: *mut i64,
    byte_offset: u64,
}

#[repr(C)]
struct DLManagedTensor {
    dl_tensor: DLTensor,
    manager_ctx: *mut c_void,
    deleter: Option<unsafe extern "C" fn(*mut DLManagedTensor)>,
}

/// What a tensor owns: its shape and strides, and a reference to the frame
/// holding its memory, so that it outlives the Python objects it came
/// from. `managed` comes first, so a pointer to it is one to the whole.
#[repr(C)]
struct Holder {
    managed: DLManagedTensor,
    shape: [i64; 3],
    strides: [i64; 3],
    _frame: Arc<Frame>,
}

/// A view of unsigned integers of `bits` bits on CUDA GPU `gpu`, starting
/// at `data` inside `frame`.
pub struct Tensor<'a> {
    pub frame: Arc<Frame>,
    pub data: *mut u8,
    pub gpu: c_int,
    pub bits: u8,
    pub shape: &'a [i64],
    pub strides: &'a [i64],
}

/// Wrap `tensor` in a `dltensor` capsule.
pub fn capsule<'py>(py: Python<'py>, tensor: Tensor<'_>) -> PyResult<Bound<'py, PyAny>> {
    let ndim = tensor.shape.len();
    assert!(ndim <= 3 && tensor.strides.len() == ndim);
    let mut holder = Box::new(Holder {
        managed: DLManagedTensor {
            dl_tensor: DLTensor {
                data: tensor.data.cast(),
                device: DLDevice {
                    device_type: CUDA,
                    device_id: tensor.gpu,
                },
                ndim: ndim as c_int,
                dtype: DLDataType {
                    code: UINT,
                    bits: tensor.bits,
                    lanes: 1,
                },
                shape: ptr::null_mut(),
                strides: ptr::null_mut(),
                byte_offset: 0,
            },
            manager_ctx: ptr::null_mut(),
            deleter: Some(delete),
        },
        shape: [0; 3],
        strides: [0; 3],
        _frame: tensor.frame,
    });
    holder.shape[..ndim].copy_from_slice(tensor.shape);
    holder.strides[..ndim].copy_from_slice(tensor.strides);
    // The box does not move, so pointers into it stay valid.
    holder.managed.dl_tensor.shape = holder.shape.as_mut_ptr();
    holder.managed.dl_tensor.strides = holder.strides.as_mut_ptr();
    let raw = Box::into_raw(holder);
    // SAFETY: `raw` is a valid Holder, owned by the capsule from here on,
    // or freed right away if the capsule cannot be created.
    unsafe {
        (*raw).managed.manager_ctx = raw.cast();
        let capsule = ffi::PyCapsule_New(raw.cast(), c"dltensor".as_ptr(), Some(destroy));
        if capsule.is_null() {
            drop(Box::from_raw(raw));
            return Err(PyErr::fetch(py));
        }
        Ok(Bound::from_owned_ptr(py, capsule))
    }
}

/// The deleter the consumer calls when done with the tensor.
unsafe extern "C" fn delete(managed: *mut DLManagedTensor) {
    // SAFETY: `managed` is the first field of a boxed Holder.
    drop(unsafe { Box::from_raw(managed.cast::<Holder>()) });
}

/// The capsule's destructor. A consumer renames the capsule to
/// `used_dltensor` and owns the tensor from then on; only a capsule that
/// nobody consumed still owns it.
unsafe extern "C" fn destroy(capsule: *mut ffi::PyObject) {
    // SAFETY: called by Python on a capsule created by `capsule`; neither
    // call raises when the name does not match.
    unsafe {
        if ffi::PyCapsule_IsValid(capsule, c"dltensor".as_ptr()) == 1 {
            let managed = ffi::PyCapsule_GetPointer(capsule, c"dltensor".as_ptr());
            delete(managed.cast());
        }
    }
}
