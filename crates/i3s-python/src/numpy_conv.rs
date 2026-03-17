//! NumPy ↔ Rust conversion and batch-processing utilities.
//!
//! Equivalent to the cesium-native `NumpyConversions.h` header. Provides:
//! - DVec3 / DQuat / DMat3 ↔ numpy conversions
//! - GIL-free batch operation templates for near-native throughput
//! - Array validation helpers

use glam::{DMat3, DQuat, DVec3};
use numpy::ndarray::{Array1, Array2, ArrayView2};
use numpy::{IntoPyArray, PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

// ============================================================================
// numpy -> glam  (always copies)
// ============================================================================

/// Convert a (3,) numpy array or Python sequence to DVec3.
pub fn to_dvec3(obj: &Bound<'_, PyAny>) -> PyResult<DVec3> {
    // Try numpy array first
    if let Ok(arr) = obj.cast::<PyArray1<f64>>() {
        let ro = arr.readonly();
        let slice = ro.as_slice()?;
        if slice.len() != 3 {
            return Err(PyValueError::new_err("Expected numpy shape (3,)"));
        }
        return Ok(DVec3::new(slice[0], slice[1], slice[2]));
    }
    // Fall back to sequence
    let seq: Vec<f64> = obj.extract()?;
    if seq.len() != 3 {
        return Err(PyValueError::new_err("Expected sequence length 3"));
    }
    Ok(DVec3::new(seq[0], seq[1], seq[2]))
}

/// Convert a (4,) numpy array or sequence to DQuat (input order: x,y,z,w).
pub fn to_dquat(obj: &Bound<'_, PyAny>) -> PyResult<DQuat> {
    if let Ok(arr) = obj.cast::<PyArray1<f64>>() {
        let ro = arr.readonly();
        let slice = ro.as_slice()?;
        if slice.len() != 4 {
            return Err(PyValueError::new_err("Expected numpy shape (4,)"));
        }
        return Ok(DQuat::from_xyzw(slice[0], slice[1], slice[2], slice[3]));
    }
    let seq: Vec<f64> = obj.extract()?;
    if seq.len() != 4 {
        return Err(PyValueError::new_err("Expected sequence length 4"));
    }
    Ok(DQuat::from_xyzw(seq[0], seq[1], seq[2], seq[3]))
}

// ============================================================================
// glam -> numpy  (copy)
// ============================================================================

/// Copy a DVec3 to a new (3,) float64 numpy array.
pub fn dvec3_to_numpy<'py>(py: Python<'py>, v: DVec3) -> Bound<'py, PyArray1<f64>> {
    Array1::from_vec(vec![v.x, v.y, v.z]).into_pyarray(py)
}

/// Copy a DQuat to a new (4,) float64 numpy array in [x,y,z,w] order.
pub fn dquat_to_numpy<'py>(py: Python<'py>, q: DQuat) -> Bound<'py, PyArray1<f64>> {
    Array1::from_vec(vec![q.x, q.y, q.z, q.w]).into_pyarray(py)
}

/// Copy a DMat3 to a new (3,3) float64 numpy array (column-major).
pub fn dmat3_to_numpy<'py>(py: Python<'py>, m: DMat3) -> Bound<'py, PyArray2<f64>> {
    let mut arr = Array2::<f64>::zeros((3, 3));
    for col in 0..3 {
        for row in 0..3 {
            arr[[col, row]] = m.col(col)[row];
        }
    }
    arr.into_pyarray(py)
}

// ============================================================================
// Batch helpers — GIL-free transforms for near-native throughput
// ============================================================================

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

    // Release the GIL while processing
    py.detach(|| {
        let out_data = output.as_slice_mut().unwrap();
        for i in 0..n {
            let in_row = view.row(i);
            let in_slice = in_row.as_slice().unwrap();
            let out_slice = &mut out_data[i * out_cols..(i + 1) * out_cols];
            func(in_slice, out_slice);
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
        for i in 0..n {
            let in_row = view.row(i);
            let in_slice = in_row.as_slice().unwrap();
            output[i] = func(in_slice);
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
        for i in 0..n {
            let in_row = view.row(i);
            let in_slice = in_row.as_slice().unwrap();
            output[i] = func(in_slice);
        }
    });

    Ok(output.into_pyarray(py))
}
