# FFI Guidelines

## M-ISOLATE-DLL-STATE

**Category:** Isolate DLL State Between FFI Libraries

Multiple Rust-based DLLs can only share "portable" state to prevent corruption.

### Portable Data Requirements

Data crossing DLL boundaries must be:
- `#[repr(C)]` or similarly well-defined layout
- No interaction with statics or thread-locals
- No interaction with `TypeId`
- No pointers to non-portable data

### Critical Issues

Each DLL has its own:
- Statics and thread-locals
- Type layout (can differ between compilations)
- Unique type IDs

### Problematic Types (DO NOT Share Across DLLs)

- **Allocated instances**: `String`, `Vec`, `Box`
- **Libraries relying on statics**: tokio, log, tracing
- **Non-`#[repr(C)]` structs**
- **Data structures relying on consistent TypeId**

### Safe FFI Pattern

```rust
// Define portable types
#[repr(C)]
pub struct PortableData {
    pub id: u64,
    pub flags: u32,
}

// Export with C ABI
#[no_mangle]
pub extern "C" fn process_data(data: *const PortableData) -> i32 {
    if data.is_null() {
        return -1;
    }

    // SAFETY: Caller guarantees valid pointer
    let data = unsafe { &*data };

    // Process and return result
    0
}
```

### String Handling Across FFI

```rust
use std::ffi::{CStr, CString};

#[no_mangle]
pub extern "C" fn get_name() -> *mut std::ffi::c_char {
    let name = CString::new("example").unwrap();
    name.into_raw()
}

#[no_mangle]
pub extern "C" fn free_name(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        // SAFETY: ptr was allocated by get_name
        unsafe { drop(CString::from_raw(ptr)); }
    }
}
```

### Checklist

- [ ] Use `#[repr(C)]` for all shared types
- [ ] Avoid passing Rust-specific types (String, Vec, Box)
- [ ] Document ownership and lifetime requirements
- [ ] Provide explicit free functions for allocated data
- [ ] Test with multiple DLL configurations
