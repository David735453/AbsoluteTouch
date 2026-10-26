# Universal Guidelines

## M-CONCISE-NAMES

**Category:** Names are Free of Weasel Words

Symbol names should be free of meaningless weasel words, especially types/traits.

### Common Offenders to Avoid

| Bad | Better |
|-----|--------|
| `FooService` | `Foo` |
| `FooManager` | `Foo` or specific action |
| `FooFactory` | `FooBuilder` |
| `FooHandler` | `Foo` or specific action |
| `FooHelper` | Function or method |
| `FooUtils` | Module or functions |

### Examples

```rust
// Bad
struct BookingService { }
struct ConnectionManager { }
struct UserFactory { }

// Good
struct Bookings { }
struct ConnectionPool { }
struct UserBuilder { }
```

If something manages bookings, call it `Bookings`. If it's specific, append that quality (e.g., `BookingDispatcher`).

---

## M-DOCUMENTED-MAGIC

**Category:** Magic Values are Documented

Hardcoded magic values must include comments explaining rationale and side effects.

### Comment Should Cover

- Why this value was chosen
- Non-obvious side effects if changed
- External systems interacting with it

### Progression

```rust
// Bad: No explanation
wait_timeout(60 * 60 * 24).await;

// Better: Some explanation
wait_timeout(60 * 60 * 24).await; // Large enough for server
                                  // to finish; too low aborts
                                  // valid requests

// Best: Named constant with full documentation
/// Duration to wait for upstream server to complete.
///
/// Large enough to ensure the server finishes processing.
/// Setting this too low aborts valid long-running requests.
/// Based on api.foo.com timeout policies.
const UPSTREAM_SERVER_TIMEOUT: Duration = Duration::from_secs(86400);

wait_timeout(UPSTREAM_SERVER_TIMEOUT).await;
```

---

## M-LINT-OVERRIDE-EXPECT

**Category:** Lint Overrides Should Use `#[expect]`

Use `#[expect]` instead of `#[allow]` for lint overrides to prevent stale annotations.

### Benefit

`#[expect]` warns if the marked lint wasn't encountered, preventing accumulation of outdated overrides.

### Pattern

```rust
// Good: Will warn if lint no longer triggers
#[expect(clippy::unused_async, reason = "API fixed; will add I/O later")]
pub async fn ping_server() { }

// Bad: May accumulate stale overrides
#[allow(clippy::unused_async)]
pub async fn ping_server() { }
```

### Exception

`#[allow]` remains appropriate for generated code and macros.

---

## M-LOG-STRUCTURED

**Category:** Use Structured Logging with Message Templates

Logging should use structured events with named properties following message templates spec.

### Key Principles

- Avoid string formatting (allocates at runtime)
- Use message templates with named properties
- Name events hierarchically: `<component>.<operation>.<state>`
- Follow OpenTelemetry semantic conventions
- Redact sensitive data

### Examples

```rust
// Bad: String formatting
tracing::info!("file opened: {}", path);

// Good: Structured event
tracing::info!(
    name: "file.open.success",
    file.path = %path.display(),
    "file opened"
);
```

### OpenTelemetry Conventions

| Domain | Attributes |
|--------|------------|
| HTTP | `http.request.method`, `http.response.status_code` |
| File | `file.path`, `file.directory`, `file.name` |
| Database | `db.system.name`, `db.namespace` |
| Errors | `error.type`, `error.message` |

---

## M-PANIC-IS-STOP

**Category:** Panic Means 'Stop the Program'

Panics signal immediate program termination, not error handling.

### Valid Panic Reasons

- Programming errors: `expect("must never happen")`
- Const contexts
- User-requested (providing `unwrap()`)
- Lock poisoning indicating prior panic

### Invalid Uses

- Communicating errors upstream
- Handling self-inflicted errors
- Assuming panics will be caught

### Implication

With `panic = "abort"`, any panic unnecessarily terminates otherwise functional programs.

---

## M-PANIC-ON-BUG

**Category:** Detected Programming Bugs are Panics, Not Errors

Unrecoverable programming errors must panic; don't return Error types.

### Context Matters

```rust
// Panic appropriate: Programming error
fn divide_by(x: i32, y: i32) -> i32 {
    assert!(y != 0, "division by zero is a bug");
    x / y
}

// Result appropriate: Inherent fallibility
fn parse_uri(s: &str) -> Result<Uri, ParseError> {
    s.parse()
}
```

### Strategy

"Make it Correct by Construction" - use types to prevent panic-inducing code paths.

```rust
// Instead of runtime checks:
fn process(value: i32) {
    assert!(value > 0);
    // ...
}

// Use types:
struct PositiveInt(i32);

impl PositiveInt {
    fn new(value: i32) -> Option<Self> {
        (value > 0).then_some(Self(value))
    }
}

fn process(value: PositiveInt) {
    // Can't be called with invalid value
}
```

---

## M-PUBLIC-DEBUG

**Category:** Public Types are Debug

All public types must implement Debug, either derived or custom.

```rust
// Simple case: Derive
#[derive(Debug)]
pub struct Config {
    pub endpoint: String,
    pub timeout: Duration,
}

// Sensitive data: Custom implementation
pub struct UserCredentials {
    username: String,
    password: String,
}

impl std::fmt::Debug for UserCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[test]
fn test_debug_redacts_password() {
    let creds = UserCredentials {
        username: "user".into(),
        password: "secret123".into(),
    };
    let debug = format!("{:?}", creds);
    assert!(!debug.contains("secret123"));
}
```

---

## M-PUBLIC-DISPLAY

**Category:** Public Types Meant to be Read are Display

Types expected to be read by users should implement Display.

### Applies To

- Error types (required by `std::error::Error`)
- String-like wrappers
- Types users will print

---

## M-REGULAR-FN

**Category:** Prefer Regular over Associated Functions

Associated functions primarily create instances; general computation belongs in regular functions.

```rust
// Bad: Unrelated to Database
impl Database {
    fn check_parameters(p: &str) -> bool { }
}

// Good: Top-level function
fn check_parameters(p: &str) -> bool { }
```

**Exception:** Trait associated functions (`Default::default()`) remain idiomatic.

---

## M-SMALLER-CRATES

**Category:** If in Doubt, Split the Crate

Err toward too many crates rather than too few for compile time and modularity.

### Principle

If a submodule is independently usable, move it to a separate crate.

### Features vs. Crates

- **Crates**: Independently usable items
- **Features**: Optional extensions to existing functionality

---

## M-STATIC-VERIFICATION

**Category:** Use Static Verification

### Cargo.toml Lints

```toml
[lints.rust]
ambiguous_negative_literals = "warn"
missing_debug_implementations = "warn"
redundant_imports = "warn"
unsafe_op_in_unsafe_fn = "warn"

[lints.clippy]
all = "warn"
cargo = "warn"
pedantic = "warn"
```

### Tools Checklist

- [ ] `rustfmt` - formatting
- [ ] `clippy` - linting
- [ ] `cargo-audit` - vulnerability scanning
- [ ] `cargo-hack` - feature combinations
- [ ] `cargo-udeps` - unused dependencies
- [ ] `miri` - unsafe code validation

---

## M-UPSTREAM-GUIDELINES

**Category:** Follow the Upstream Guidelines

### Key Rust API Guidelines

| Rule | Description |
|------|-------------|
| C-CONV | Conversion: `as_`/`to_`/`into_` conventions |
| C-GETTER | Getters follow Rust conventions |
| C-COMMON-TRAITS | Implement standard traits |
| C-CTOR | Static inherent constructors |

### Resources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Style Guide](https://doc.rust-lang.org/style-guide/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
