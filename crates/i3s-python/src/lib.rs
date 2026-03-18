//! Python bindings for i3s-native.

mod async_support;
mod geometry;
mod geospatial;
mod numpy_conv;
mod selection;
mod spec;

use pyo3::prelude::*;

/// Root Python module: `i3s._native`
#[pymodule]
fn _native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.setattr(
        "__doc__",
        "Compiled Rust bindings for the i3s-native engine",
    )?;

    // Submodules
    let async_mod = PyModule::new(py, "async_")?;
    async_support::register(&async_mod)?;
    m.add_submodule(&async_mod)?;

    let geom = PyModule::new(py, "geometry")?;
    geometry::register(&geom)?;
    m.add_submodule(&geom)?;

    let geospatial_mod = PyModule::new(py, "geospatial")?;
    geospatial::register(&geospatial_mod)?;
    m.add_submodule(&geospatial_mod)?;

    let selection_mod = PyModule::new(py, "selection")?;
    selection::register(&selection_mod)?;
    m.add_submodule(&selection_mod)?;

    let spec_mod = PyModule::new(py, "spec")?;
    spec::register(&spec_mod)?;
    m.add_submodule(&spec_mod)?;

    // Register submodules in sys.modules so `from i3s.X import Y` works.
    // Keys must match the Python package paths, not the native module paths.
    let sys_modules = py.import("sys")?.getattr("modules")?;
    sys_modules.set_item("i3s.async_", &async_mod)?;
    sys_modules.set_item("i3s.geometry", &geom)?;
    sys_modules.set_item("i3s.geospatial", &geospatial_mod)?;
    sys_modules.set_item("i3s.selection", &selection_mod)?;
    sys_modules.set_item("i3s.spec", &spec_mod)?;

    Ok(())
}
