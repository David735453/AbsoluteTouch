# Documentation Guidelines

## M-CANONICAL-DOCS

**Category:** Documentation Has Canonical Sections

Public library items must contain canonical doc sections with consistent structure.

### Required Sections

1. **Summary sentence** (< 15 words, always present)
2. **Extended documentation** (when applicable)
3. **Examples** (required for public items)
4. **Errors** (for functions returning Result)
5. **Panics** (if function may panic)
6. **Safety** (for unsafe functions)
7. **Abort** (if function may abort)

### Template

```rust
/// Brief summary under 15 words describing the function.
///
/// Extended description providing more context, use cases,
/// and implementation details when relevant.
///
/// # Examples
///
/// ```
/// use my_crate::my_function;
///
/// let result = my_function(42)?;
/// assert_eq!(result, 84);
/// # Ok::<(), my_crate::Error>(())
/// ```
///
/// # Errors
///
/// Returns [`MyError::InvalidInput`] if the value is negative.
///
/// # Panics
///
/// Panics if called from within an async context.
pub fn my_function(value: i32) -> Result<i32, MyError> { }
```

### Anti-Patterns

```rust
// Bad: Parameter table format
/// # Parameters
/// * `value` - The input value
/// * `config` - Configuration options

// Good: Explain parameters in prose
/// Processes the input `value` according to `config` settings.
```

---

## M-DOC-INLINE

**Category:** Mark `pub use` Items with `#[doc(inline)]`

Re-exported items should use `#[doc(inline)]` to appear naturally in documentation.

### Usage

```rust
// Good: Item appears in this module's docs
#[doc(inline)]
pub use crate::internal::ImportantType;

// Bad: Users must navigate to internal module
pub use crate::internal::ImportantType;
```

### Exception

External types from std or third-party crates should remain non-inlined:

```rust
// Keep external types non-inlined
pub use std::collections::HashMap;
pub use serde::Serialize;
```

---

## M-FIRST-DOC-SENTENCE

**Category:** First Sentence is One Line; Approximately 15 Words

Documentation summary sentences should fit on one line and not exceed ~15 words.

### Rationale

- Improves skimmability of module summaries
- Prevents widow lines in rendered docs
- Forces concise, focused descriptions

### Examples

```rust
// Good: Concise summary
/// Creates a new database connection with default settings.
pub fn connect() -> Connection { }

// Bad: Too long
/// Creates a new database connection to the specified server using the
/// provided credentials and configuration options.
pub fn connect() -> Connection { }

// Good: Split into summary + extended
/// Creates a new database connection with default settings.
///
/// Connects to the specified server using the provided credentials
/// and configuration options.
pub fn connect() -> Connection { }
```

---

## M-MODULE-DOCS

**Category:** Has Comprehensive Module Documentation

Every public library module requires `//!` documentation with comprehensive coverage.

### Should Cover

- Module contents overview
- When/when not to use
- Examples
- Technical specifications
- Observable side effects and guarantees
- Relevant implementation details

### Template

```rust
//! Brief module summary under 15 words.
//!
//! This module provides functionality for handling X. Use this when
//! you need to Y. Do not use this for Z.
//!
//! # Overview
//!
//! The main types in this module are:
//! - [`Foo`] - Primary interface for...
//! - [`Bar`] - Helper for...
//!
//! # Examples
//!
//! Basic usage:
//!
//! ```
//! use my_crate::my_module::Foo;
//!
//! let foo = Foo::new();
//! foo.process()?;
//! # Ok::<(), my_crate::Error>(())
//! ```
//!
//! # Implementation Notes
//!
//! This module uses X internally for performance reasons.

use crate::internal;

/// The main type for...
pub struct Foo { }
```

### Reference Examples

Study these std modules for excellent documentation:
- `std::fmt`
- `std::pin`
- `std::option`
