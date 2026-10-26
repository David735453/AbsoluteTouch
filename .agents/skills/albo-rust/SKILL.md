---
name: albo:rust
description: Applies Microsoft Pragmatic Rust Guidelines when writing or modifying Rust code. Use BEFORE writing ANY Rust code (.rs files) including simple programs. Enforces naming conventions, error handling, documentation standards, safety rules, and performance patterns.
---

# Microsoft Pragmatic Rust Guidelines

Apply these guidelines when writing or reviewing Rust code (.rs files).

## Trigger Conditions

Invoke this skill when:
- Creating new `.rs` files
- Modifying existing Rust code
- Reviewing Rust pull requests
- Designing Rust APIs or libraries

## Language

Use **American English** for all comments, documentation, and string literals.

## Quick Reference - Universal Rules

### Naming (M-CONCISE-NAMES)
- Avoid weasel words: `Service`, `Manager`, `Factory`, `Handler`, `Helper`
- `FooService` → `Foo`
- `BookingManager` → `Bookings`
- `UserFactory` → use Builder pattern instead

### Panics (M-PANIC-IS-STOP)
- Panics = immediate program termination, NOT error handling
- Valid: programming errors, const contexts, lock poisoning
- Invalid: communicating errors upstream, handling recoverable errors
- Use `Result<T, E>` for recoverable errors

### Documentation (M-CANONICAL-DOCS)
```rust
/// Summary sentence under 15 words.
///
/// Extended description with more details.
///
/// # Examples
///
/// ```
/// let result = my_function(42);
/// ```
///
/// # Errors
///
/// Returns `MyError` when...
///
/// # Panics
///
/// Panics if...
pub fn my_function(value: i32) -> Result<Output, MyError> { }
```

### Debug Trait (M-PUBLIC-DEBUG)
All public types must implement `Debug`:
```rust
#[derive(Debug)]
pub struct MyType { }

// For sensitive data:
impl Debug for Secret {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Secret(...)")
    }
}
```

### Lint Overrides (M-LINT-OVERRIDE-EXPECT)
```rust
// Good - warns if lint no longer triggers
#[expect(clippy::unused_async, reason = "will add I/O later")]
pub async fn ping() { }

// Bad - accumulates stale overrides
#[allow(clippy::unused_async)]
pub async fn ping() { }
```

### Error Handling
- **Applications**: Use `anyhow` or `eyre` (M-APP-ERROR)
- **Libraries**: Use situation-specific error structs (M-ERRORS-CANONICAL-STRUCTS)

```rust
// Library error struct
pub struct ConfigError {
    backtrace: Backtrace,
    path: PathBuf,
}

impl ConfigError {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { backtrace: Backtrace::capture(), path }
    }
}

impl Display for ConfigError { /* ... */ }
impl std::error::Error for ConfigError { }
```

### Safety (M-UNSAFE, M-UNSOUND)
- `unsafe` only for: novel abstractions, performance, FFI
- All code must be **sound** - no exceptions
- Pass Miri for unsafe code

## Detailed Reference Files

For comprehensive guidelines, consult these reference files:

| Topic | File | Key Rules |
|-------|------|-----------|
| AI-Friendly APIs | `references/ai-guidelines.md` | M-DESIGN-FOR-AI |
| Applications | `references/application-guidelines.md` | M-APP-ERROR, M-MIMALLOC-APPS |
| Documentation | `references/documentation-guidelines.md` | M-CANONICAL-DOCS, M-MODULE-DOCS |
| FFI | `references/ffi-guidelines.md` | M-ISOLATE-DLL-STATE |
| Library Design | `references/library-guidelines.md` | M-FEATURES-ADDITIVE, M-OOBE |
| Library UX | `references/library-ux-guidelines.md` | M-INIT-BUILDER, M-ERRORS-CANONICAL-STRUCTS |
| Performance | `references/performance-guidelines.md` | M-HOTPATH, M-THROUGHPUT |
| Safety | `references/safety-guidelines.md` | M-UNSAFE, M-UNSOUND |
| Universal | `references/universal-guidelines.md` | M-CONCISE-NAMES, M-PANIC-IS-STOP |

## Static Verification Checklist

Enable in `Cargo.toml`:
```toml
[lints.rust]
missing_debug_implementations = "warn"
unsafe_op_in_unsafe_fn = "warn"

[lints.clippy]
all = "warn"
pedantic = "warn"
```

Use these tools:
- `rustfmt` - formatting
- `clippy` - linting
- `cargo-audit` - vulnerability scanning
- `miri` - unsafe code validation

## Code Generation Notes

When generating Rust code:

1. **Do NOT add compliance markers** - No `// M-XXX compliant` comments
2. **Apply rules silently** - Follow guidelines without annotation
3. **Prioritize soundness** - Never generate unsound code
4. **Use proper types** - `PathBuf` for paths, strong types over primitives
5. **Document public items** - Summary < 15 words, examples section

## Source

Based on [Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/).
