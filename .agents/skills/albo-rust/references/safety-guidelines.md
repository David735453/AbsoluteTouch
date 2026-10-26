# Safety Guidelines

## M-UNSAFE-IMPLIES-UB

**Category:** Unsafe Implies Undefined Behavior

The `unsafe` marker may only apply to functions/traits where misuse risks undefined behavior.

### Valid Use

```rust
/// Prints a string from a raw pointer.
///
/// # Safety
///
/// `x` must point to a valid, initialized `String`.
unsafe fn print_string(x: *const String) {
    println!("{}", unsafe { &*x });
}
```

### Invalid Use

```rust
// Bad: Dangerous but not UB-related
unsafe fn delete_database() {
    std::fs::remove_dir_all("/var/db").unwrap();
}

// Should be:
fn delete_database() {
    std::fs::remove_dir_all("/var/db").unwrap();
}
```

---

## M-UNSAFE

**Category:** Unsafe Needs Reason, Should be Avoided

Use `unsafe` only for novel abstractions, performance, or FFI/platform calls.

### Valid Reasons

1. **Novel abstractions** (smart pointers, allocators)
2. **Performance** (e.g., `.get_unchecked()`)
3. **FFI and platform calls**

### Invalid Uses (Ad-Hoc Unsafe)

- Shortening safe programs via transmute
- Bypassing Send bounds
- Bypassing lifetime requirements

### For Novel Abstractions

- [ ] Verify no established alternative exists
- [ ] Minimize and test thoroughly
- [ ] Harden against adversarial code and panicking closures
- [ ] Accompany with plain-text safety reasoning
- [ ] Pass Miri including adversarial tests
- [ ] Follow unsafe code guidelines

### For Performance

```rust
// Only use when benchmarks prove necessity
fn sum_unchecked(data: &[i32], indices: &[usize]) -> i32 {
    let mut sum = 0;
    for &i in indices {
        // SAFETY: Caller guarantees all indices are in bounds.
        // Benchmarks show 15% improvement over bounds-checked version.
        sum += unsafe { *data.get_unchecked(i) };
    }
    sum
}
```

Requirements:
- [ ] Benchmark before implementation
- [ ] Document safety reasoning
- [ ] Pass Miri
- [ ] Follow unsafe code guidelines

### For FFI

```rust
// Use established interop libraries when possible
extern "C" {
    fn external_function(data: *const u8, len: usize) -> i32;
}

/// Calls the external function with the provided data.
///
/// # Safety
///
/// The external library must be properly initialized before calling.
pub unsafe fn call_external(data: &[u8]) -> i32 {
    // SAFETY: data.as_ptr() is valid for data.len() bytes
    external_function(data.as_ptr(), data.len())
}
```

---

## M-UNSOUND

**Category:** All Code Must be Sound

Seemingly safe code must never produce undefined behavior when called.

### Definition

A function is **unsound** if it appears safe (unmarked `unsafe`) but any calling mode causes UB.

### Anti-Pattern

```rust
// UNSOUND! Appears safe but causes UB
fn unsound_ref<T>(x: &T) -> &u128 {
    unsafe { std::mem::transmute(x) }
}

// This compiles but is UB:
let small: u8 = 42;
let big: &u128 = unsound_ref(&small); // Reading uninitialized memory!
```

### Correct Pattern

```rust
// Mark as unsafe since caller must ensure T has correct size/alignment
unsafe fn cast_ref<T>(x: &T) -> &u128 {
    assert!(std::mem::size_of::<T>() >= std::mem::size_of::<u128>());
    assert!(std::mem::align_of::<T>() >= std::mem::align_of::<u128>());
    &*(x as *const T as *const u128)
}
```

### Important

**No Exceptions.** There are no exceptions in this case: unsound code is never acceptable.

### Module Boundaries

Soundness boundaries equal module boundaries; safe functions can rely on guarantees elsewhere in the same module.

```rust
mod internal {
    pub struct SafeWrapper {
        // Invariant: data is always valid
        data: *mut u8,
    }

    impl SafeWrapper {
        pub fn new() -> Self {
            Self { data: Box::into_raw(Box::new(0u8)) }
        }

        // Safe because module maintains invariant
        pub fn get(&self) -> u8 {
            // SAFETY: data is always valid per module invariant
            unsafe { *self.data }
        }
    }

    impl Drop for SafeWrapper {
        fn drop(&mut self) {
            // SAFETY: data was allocated with Box::new
            unsafe { drop(Box::from_raw(self.data)); }
        }
    }
}
```

### Testing Unsafe Code

```bash
# Run with Miri to detect UB
cargo +nightly miri test
```
