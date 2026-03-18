//! NumPy ↔ Rust conversion and batch-processing utilities.
//!
//! Provides:
//! - DVec3 / DQuat / DMat3 ↔ numpy conversions
//! - GIL-free batch operation templates for near-native throughput
//! - Array validation helpers

use glam::{DMat3, DMat4, DQuat, DVec3};
use numpy::ndarray::{Array1, Array2, ArrayView2};
use numpy::{
    IntoPyArray, PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray2, PyUntypedArrayMethods,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Create `(N, 3)` float32 numpy array from `&[[f32; 3]]`.
pub fn f32x3_to_pyarray2<'py>(py: Python<'py>, data: &[[f32; 3]]) -> Bound<'py, PyArray2<f32>> {
    let n = data.len();
    // SAFETY: [f32; 3] has identical layout to 3 contiguous f32 values.
    let flat: &[f32] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const f32, n * 3) };
    Array2::from_shape_vec((n, 3), flat.to_vec())
        .expect("contiguous f32x3 layout")
        .into_pyarray(py)
}

/// Create `(N, 2)` float32 numpy array from `&[[f32; 2]]`.
pub fn f32x2_to_pyarray2<'py>(py: Python<'py>, data: &[[f32; 2]]) -> Bound<'py, PyArray2<f32>> {
    let n = data.len();
    let flat: &[f32] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const f32, n * 2) };
    Array2::from_shape_vec((n, 2), flat.to_vec())
        .expect("contiguous f32x2 layout")
        .into_pyarray(py)
}

/// Create `(N, 4)` uint8 numpy array from `&[[u8; 4]]`.
pub fn u8x4_to_pyarray2<'py>(py: Python<'py>, data: &[[u8; 4]]) -> Bound<'py, PyArray2<u8>> {
    let n = data.len();
    let flat: &[u8] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, n * 4) };
    Array2::from_shape_vec((n, 4), flat.to_vec())
        .expect("contiguous u8x4 layout")
        .into_pyarray(py)
}

/// Create `(N, 2)` uint32 numpy array from `&[[u32; 2]]`.
pub fn u32x2_to_pyarray2<'py>(py: Python<'py>, data: &[[u32; 2]]) -> Bound<'py, PyArray2<u32>> {
    let n = data.len();
    let flat: &[u32] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u32, n * 2) };
    Array2::from_shape_vec((n, 2), flat.to_vec())
        .expect("contiguous u32x2 layout")
        .into_pyarray(py)
}

/// Create `(N, 4)` uint16 numpy array from `&[[u16; 4]]`.
pub fn u16x4_to_pyarray2<'py>(py: Python<'py>, data: &[[u16; 4]]) -> Bound<'py, PyArray2<u16>> {
    let n = data.len();
    let flat: &[u16] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u16, n * 4) };
    Array2::from_shape_vec((n, 4), flat.to_vec())
        .expect("contiguous u16x4 layout")
        .into_pyarray(py)
}

/// Create `(N, 3)` float64 numpy array from `&[DVec3]` via a single memcpy.
pub fn dvec3_slice_to_pyarray2<'py>(py: Python<'py>, data: &[DVec3]) -> Bound<'py, PyArray2<f64>> {
    let n = data.len();
    let flat: &[f64] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const f64, n * 3) };
    Array2::from_shape_vec((n, 3), flat.to_vec())
        .expect("DVec3 repr(C) layout")
        .into_pyarray(py)
}

/// Convert a `(3,)` float64 numpy array to `DVec3`.
pub fn to_dvec3(obj: &Bound<'_, PyAny>) -> PyResult<DVec3> {
    let arr = obj
        .cast::<PyArray1<f64>>()
        .map_err(|_| PyValueError::new_err("Expected a (3,) float64 numpy array"))?;
    let ro = arr.readonly();
    let slice = ro.as_slice()?;
    if slice.len() != 3 {
        return Err(PyValueError::new_err(format!(
            "Expected numpy shape (3,), got ({},)",
            slice.len()
        )));
    }
    Ok(DVec3::new(slice[0], slice[1], slice[2]))
}

/// Convert a `(4,)` float64 numpy array to `DQuat` (input order: x,y,z,w).
pub fn to_dquat(obj: &Bound<'_, PyAny>) -> PyResult<DQuat> {
    let arr = obj
        .cast::<PyArray1<f64>>()
        .map_err(|_| PyValueError::new_err("Expected a (4,) float64 numpy array"))?;
    let ro = arr.readonly();
    let slice = ro.as_slice()?;
    if slice.len() != 4 {
        return Err(PyValueError::new_err(format!(
            "Expected numpy shape (4,), got ({},)",
            slice.len()
        )));
    }
    Ok(DQuat::from_xyzw(slice[0], slice[1], slice[2], slice[3]))
}

/// Copy a DVec3 to a new (3,) float64 numpy array.
pub fn dvec3_to_numpy<'py>(py: Python<'py>, v: DVec3) -> Bound<'py, PyArray1<f64>> {
    Array1::from_vec(vec![v.x, v.y, v.z]).into_pyarray(py)
}

/// Copy a DQuat to a new (4,) float64 numpy array in [x,y,z,w] order.
pub fn dquat_to_numpy<'py>(py: Python<'py>, q: DQuat) -> Bound<'py, PyArray1<f64>> {
    Array1::from_vec(vec![q.x, q.y, q.z, q.w]).into_pyarray(py)
}

/// Copy a DMat3 to a new (3,3) float64 numpy array (row-major, i.e. arr[row, col]).
pub fn dmat3_to_numpy<'py>(py: Python<'py>, m: DMat3) -> Bound<'py, PyArray2<f64>> {
    // glam is column-major; transpose → to_cols_array gives row-major for numpy.
    let flat = m.transpose().to_cols_array();
    Array2::from_shape_vec((3, 3), flat.to_vec())
        .expect("DMat3 3×3 layout")
        .into_pyarray(py)
}

/// Copy a DMat4 to a new (4,4) float64 numpy array (row-major).
pub fn dmat4_to_numpy<'py>(py: Python<'py>, m: &DMat4) -> Bound<'py, PyArray2<f64>> {
    let flat = m.transpose().to_cols_array();
    Array2::from_shape_vec((4, 4), flat.to_vec())
        .expect("DMat4 4×4 layout")
        .into_pyarray(py)
}

/// Extract a `DMat4` from a `(4, 4)` float64 numpy array (row-major input).
pub fn to_dmat4(obj: &Bound<'_, PyAny>) -> PyResult<DMat4> {
    let arr = obj
        .cast::<PyArray2<f64>>()
        .map_err(|_| PyValueError::new_err("Expected (4, 4) float64 numpy array"))?;
    let ro = arr.readonly();
    let v = ro.as_array();
    if v.shape() != [4, 4] {
        return Err(PyValueError::new_err("Expected (4, 4) matrix"));
    }
    Ok(DMat4::from_cols_array(&[
        v[[0, 0]],
        v[[1, 0]],
        v[[2, 0]],
        v[[3, 0]],
        v[[0, 1]],
        v[[1, 1]],
        v[[2, 1]],
        v[[3, 1]],
        v[[0, 2]],
        v[[1, 2]],
        v[[2, 2]],
        v[[3, 2]],
        v[[0, 3]],
        v[[1, 3]],
        v[[2, 3]],
        v[[3, 3]],
    ]))
}

// Batch helpers — GIL-free transforms for near-native throughput

/// Check if a PyAny is a (N, cols) float64 numpy array.
pub fn is_points_array(obj: &Bound<'_, PyAny>, cols: usize) -> bool {
    if let Ok(arr) = obj.cast::<PyArray2<f64>>() {
        let shape = arr.shape();
        shape.len() == 2 && shape[1] == cols
    } else {
        false
    }
}

/// Require a (N, cols) float64 numpy array; error otherwise.
pub fn require_points_array<'py>(
    obj: &Bound<'py, PyAny>,
    cols: usize,
) -> PyResult<PyReadonlyArray2<'py, f64>> {
    let arr = obj
        .cast::<PyArray2<f64>>()
        .map_err(|_| PyValueError::new_err(format!("Expected (N, {cols}) float64 numpy array")))?;
    let ro = arr.readonly();
    let shape = ro.shape();
    if shape.len() != 2 || shape[1] != cols {
        return Err(PyValueError::new_err(format!(
            "Expected (N, {cols}) float64 numpy array, got shape ({}, {})",
            shape[0], shape[1]
        )));
    }
    Ok(ro)
}

/// Apply a unary transform to N input points (N, in_cols) -> (N, out_cols).
/// The closure runs without the GIL held for maximum throughput.
pub fn batch_transform<'py, F>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    in_cols: usize,
    out_cols: usize,
    func: F,
) -> PyResult<Bound<'py, PyArray2<f64>>>
where
    F: Fn(&[f64], &mut [f64]) + Send + Sync,
{
    let ro = require_points_array(input, in_cols)?;
    let view: ArrayView2<f64> = ro.as_array();
    let n = view.nrows();
    let mut output = Array2::<f64>::zeros((n, out_cols));

    py.detach(|| {
        let out_data = output
            .as_slice_mut()
            .expect("freshly allocated Array2 is always C-contiguous");
        if let Some(flat_in) = view.as_slice() {
            // Fast path: C-contiguous input (numpy default) — no per-row copy.
            for i in 0..n {
                let in_slice = &flat_in[i * in_cols..(i + 1) * in_cols];
                let out_slice = &mut out_data[i * out_cols..(i + 1) * out_cols];
                func(in_slice, out_slice);
            }
        } else {
            // Fallback: non-contiguous (Fortran-order, strided slice, etc.).
            let mut row_buf = vec![0.0f64; in_cols];
            for i in 0..n {
                let in_row = view.row(i);
                for (j, &v) in in_row.iter().enumerate() {
                    row_buf[j] = v;
                }
                let out_slice = &mut out_data[i * out_cols..(i + 1) * out_cols];
                func(&row_buf, out_slice);
            }
        }
    });

    Ok(output.into_pyarray(py))
}

/// Apply a unary predicate to N input points (N, cols) -> (N,) bool.
pub fn batch_predicate<'py, F>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    cols: usize,
    func: F,
) -> PyResult<Bound<'py, PyArray1<bool>>>
where
    F: Fn(&[f64]) -> bool + Send + Sync,
{
    let ro = require_points_array(input, cols)?;
    let view: ArrayView2<f64> = ro.as_array();
    let n = view.nrows();
    let mut output = Array1::<bool>::from_elem(n, false);

    py.detach(|| {
        if let Some(flat_in) = view.as_slice() {
            for i in 0..n {
                let in_slice = &flat_in[i * cols..(i + 1) * cols];
                output[i] = func(in_slice);
            }
        } else {
            let mut row_buf = vec![0.0f64; cols];
            for i in 0..n {
                let in_row = view.row(i);
                for (j, &v) in in_row.iter().enumerate() {
                    row_buf[j] = v;
                }
                output[i] = func(&row_buf);
            }
        }
    });

    Ok(output.into_pyarray(py))
}

/// Apply a unary function to N input points (N, cols) -> (N,) f64.
pub fn batch_scalar<'py, F>(
    py: Python<'py>,
    input: &Bound<'py, PyAny>,
    cols: usize,
    func: F,
) -> PyResult<Bound<'py, PyArray1<f64>>>
where
    F: Fn(&[f64]) -> f64 + Send + Sync,
{
    let ro = require_points_array(input, cols)?;
    let view: ArrayView2<f64> = ro.as_array();
    let n = view.nrows();
    let mut output = Array1::<f64>::zeros(n);

    py.detach(|| {
        if let Some(flat_in) = view.as_slice() {
            for i in 0..n {
                let in_slice = &flat_in[i * cols..(i + 1) * cols];
                output[i] = func(in_slice);
            }
        } else {
            let mut row_buf = vec![0.0f64; cols];
            for i in 0..n {
                let in_row = view.row(i);
                for (j, &v) in in_row.iter().enumerate() {
                    row_buf[j] = v;
                }
                output[i] = func(&row_buf);
            }
        }
    });

    Ok(output.into_pyarray(py))
}
