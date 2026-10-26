# Library Guidelines

## Building

### M-FEATURES-ADDITIVE

**Category:** Features are Additive

All library features must be additive; any combination must work.

#### Requirements

- No `no-std` feature (use `std` instead)
- Adding feature `foo` must not disable/modify public items
- Adding enum variants is acceptable if enum is `#[non_exhaustive]`
- Features must not rely on manual enabling of other features
- Features must not skip-enable child features

#### Example

```toml
# Cargo.toml
[features]
default = ["std"]
std = []
serde = ["dep:serde"]
async = ["dep:tokio"]
```

```rust
// Good: Features only add functionality
#[cfg(feature = "serde")]
impl Serialize for MyType { }

#[cfg(feature = "async")]
impl MyType {
    pub async fn process_async(&self) { }
}

// Bad: Feature removes functionality
#[cfg(not(feature = "minimal"))]
pub fn full_feature() { }
```

---

### M-OOBE

**Category:** Libraries Work Out of the Box

Libraries must build on all Tier 1 platforms without prerequisites beyond cargo/rust.

#### Checklist

- [ ] Build on all Tier 1 platforms (Windows, Linux, macOS)
- [ ] Require no additional tools or environment variables
- [ ] Use conditional compilation for platform-specific dependencies

#### Example

```rust
#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(target_os = "linux")]
mod linux_impl;

#[cfg(target_os = "macos")]
mod macos_impl;

pub fn platform_specific() {
    #[cfg(target_os = "windows")]
    windows_impl::run();

    #[cfg(target_os = "linux")]
    linux_impl::run();

    #[cfg(target_os = "macos")]
    macos_impl::run();
}
```

---

### M-SYS-CRATES

**Category:** Native `-sys` Crates Compile Without Dependencies

Native wrapper crates must fully control native library builds from build.rs.

#### Requirements

- Fully govern native library build from build.rs
- Make external tools optional
- Embed upstream source code
- Make sources verifiable (Git URL + hash)
- Pre-generate bindgen glue when possible
- Support static and dynamic linking via libloading

---

## Interoperability

### M-DONT-LEAK-TYPES

**Category:** Don't Leak External Types

Prefer std types in public APIs; carefully consider external type exposure.

#### Heuristic

- [ ] Avoid leaking third-party types when possible
- [ ] Umbrella crates may freely leak sibling types
- [ ] Behind feature flags, types may leak (e.g., serde)
- [ ] Without features only for substantial benefit

```rust
// Good: Use std types
pub fn process(data: &[u8]) -> Vec<u8> { }

// Acceptable: Behind feature flag
#[cfg(feature = "bytes")]
pub fn process_bytes(data: bytes::Bytes) -> bytes::Bytes { }
```

---

### M-ESCAPE-HATCHES

**Category:** Native Escape Hatches

Types wrapping native handles should provide unsafe conversion methods.

```rust
pub struct Handle(HNATIVE);

impl Handle {
    pub fn new() -> Self { }

    /// Creates a handle from a native value.
    ///
    /// # Safety
    ///
    /// Caller must ensure `native` is a valid handle.
    pub unsafe fn from_native(native: HNATIVE) -> Self {
        Self(native)
    }

    /// Consumes the handle, returning the native value.
    pub fn into_native(self) -> HNATIVE {
        let native = self.0;
        std::mem::forget(self);
        native
    }

    /// Returns the native handle value.
    pub fn as_native(&self) -> HNATIVE {
        self.0
    }
}
```

---

### M-TYPES-SEND

**Category:** Types are Send

Public types should be `Send` for Tokio and async abstraction compatibility.

#### Assertion Pattern

```rust
const fn assert_send<T: Send>() {}
const _: () = assert_send::<MyFuture>();
const _: () = assert_send::<MyService>();
```

---

## Resilience

### M-AVOID-STATICS

**Category:** Avoid Statics

Libraries should avoid statics/thread-locals where consistent views matter.

```rust
// Bad: Hidden global state
static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn increment() -> u64 {
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

// Good: Explicit state
pub struct Counter(AtomicU64);

impl Counter {
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn increment(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}
```

---

### M-MOCKABLE-SYSCALLS

**Category:** I/O and System Calls Are Mockable

User-facing types performing I/O or syscalls should be mockable.

```rust
// Good: Mockable I/O
pub struct Library<R: Runtime> {
    runtime: R,
}

impl<R: Runtime> Library<R> {
    pub fn new(runtime: R) -> Self {
        Self { runtime }
    }
}

// For testing
impl Library<MockRuntime> {
    pub fn new_mocked() -> (Self, MockController) {
        let (runtime, controller) = MockRuntime::new();
        (Self { runtime }, controller)
    }
}
```

---

### M-NO-GLOB-REEXPORTS

**Category:** Don't Glob Re-Export Items

```rust
// Bad: Opaque, hard to review
pub use internal::*;

// Good: Explicit, reviewable
pub use internal::{TypeA, TypeB, function_c};
```

---

### M-TEST-UTIL

**Category:** Test Utilities are Feature Gated

```rust
#[cfg(feature = "test-util")]
pub mod test_util {
    pub fn create_mock_data() -> MockData { }
    pub fn bypass_validation() { }
}
```

```toml
[features]
test-util = []
```
