//! Type-erased value for FFI boundary crossing.
//!
//! [`ErasedValue`] wraps a `*mut c_void` value pointer together with a
//! [`orkester_value_vtable_t`] that provides lifecycle operations (drop,
//! clone). This replaces the old `Payload` pattern of passing individual
//! function pointers at each call site.

use std::ffi::c_void;

/// Vtable for type-erased values crossing the FFI boundary.
///
/// The host allocates one static vtable per type and passes a pointer to
/// it when resolving or creating values. Rust stores the vtable reference
/// alongside the value pointer.
#[repr(C)]
pub struct orkester_value_vtable_t {
    /// Destroy the value. Called when orkester drops the value.
    /// Must handle null gracefully.
    pub drop: Option<unsafe extern "C" fn(*mut c_void)>,

    /// Clone the value. Required for shared tasks. May be null if the
    /// value type is not cloneable (sharing will panic in debug mode).
    pub clone: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
}

/// A type-erased value that travels through the scheduling pipeline.
///
/// Owns a `*mut c_void` data pointer and a vtable for lifecycle management.
/// When dropped, calls the vtable's `drop` function. When cloned (for
/// shared tasks), calls the vtable's `clone` function.
pub(crate) struct ErasedValue {
    data: *mut c_void,
    vtable: &'static orkester_value_vtable_t,
}

// SAFETY: The host guarantees that the value is safe to send across threads.
// All values flowing through orkester's scheduling pipeline must be thread-safe.
unsafe impl Send for ErasedValue {}

/// Static vtable for empty (null) values — no-op drop, no clone.
static EMPTY_VTABLE: orkester_value_vtable_t = orkester_value_vtable_t {
    drop: None,
    clone: None,
};

impl ErasedValue {
    /// Create an empty value (null pointer, no-op vtable).
    pub(crate) fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            vtable: &EMPTY_VTABLE,
        }
    }

    /// Create from a raw pointer and vtable.
    pub(crate) fn new(data: *mut c_void, vtable: &'static orkester_value_vtable_t) -> Self {
        Self { data, vtable }
    }

    /// Create from a raw pointer and individual function pointers (legacy compat).
    ///
    /// Leaks a vtable allocation. Use `new()` with a static vtable for
    /// production paths.
    pub(crate) fn from_raw(
        data: *mut c_void,
        drop_fn: Option<unsafe extern "C" fn(*mut c_void)>,
        clone_fn: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    ) -> Self {
        let vtable = Box::leak(Box::new(orkester_value_vtable_t {
            drop: drop_fn,
            clone: clone_fn,
        }));
        Self { data, vtable }
    }

    /// Extract the data pointer, preventing the destructor from running.
    pub(crate) fn take(&mut self) -> *mut c_void {
        let val = self.data;
        self.data = std::ptr::null_mut();
        val
    }
}

impl Clone for ErasedValue {
    fn clone(&self) -> Self {
        if self.data.is_null() {
            return Self {
                data: std::ptr::null_mut(),
                vtable: self.vtable,
            };
        }
        match self.vtable.clone {
            Some(clone_fn) => Self {
                data: unsafe { clone_fn(self.data) },
                vtable: self.vtable,
            },
            None => {
                debug_assert!(
                    false,
                    "ErasedValue::clone() called without clone function in vtable; \
                     pass a vtable with clone to orkester_task_share"
                );
                Self {
                    data: std::ptr::null_mut(),
                    vtable: self.vtable,
                }
            }
        }
    }
}

impl Drop for ErasedValue {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.vtable.drop {
            if !self.data.is_null() {
                unsafe { drop_fn(self.data) };
            }
        }
    }
}
