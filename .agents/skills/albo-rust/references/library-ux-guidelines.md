# Library UX Guidelines

## M-AVOID-WRAPPERS

**Category:** Avoid Smart Pointers and Wrappers in APIs

Hide smart pointers (Rc, Arc, Box, RefCell) behind clean APIs using simple types.

### Anti-Pattern

```rust
pub fn process_shared(data: Arc<Mutex<Shared>>) -> Box<Processed>
```

### Pattern

```rust
pub fn process_data(data: &Data) -> State
```

### Exceptions

- Smart pointer fundamental to purpose (container library)
- Benchmarks show significant performance gains

---

## M-DI-HIERARCHY

**Category:** Prefer Types over Generics, Generics over Dyn Traits

### Escalation Ladder

1. **Concrete types** (preferred)
2. **Generics** (acceptable unless nesting problem)
3. **Dyn traits** (last resort, hidden in wrappers)

### Generic Nesting Solution

```rust
// When nesting gets too deep, wrap dyn trait
struct DynamicDataAccess(Arc<dyn DataAccess>);

impl DynamicDataAccess {
    fn new<T: DataAccess + 'static>(db: T) -> Self {
        Self(Arc::new(db))
    }
}

impl DataAccess for DynamicDataAccess {
    fn load(&self) -> Data {
        self.0.load()
    }
}
```

---

## M-ERRORS-CANONICAL-STRUCTS

**Category:** Errors are Canonical Structs

Errors should be situation-specific structs with Backtrace and upstream causes.

### Structure

```rust
use std::backtrace::Backtrace;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ConfigurationError {
    backtrace: Backtrace,
    path: PathBuf,
    kind: ConfigErrorKind,
}

#[derive(Debug)]
enum ConfigErrorKind {
    NotFound,
    ParseError(String),
    InvalidValue { key: String, reason: String },
}

impl ConfigurationError {
    pub(crate) fn not_found(path: impl AsRef<Path>) -> Self {
        Self {
            backtrace: Backtrace::capture(),
            path: path.as_ref().to_owned(),
            kind: ConfigErrorKind::NotFound,
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.path
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self.kind, ConfigErrorKind::NotFound)
    }
}

impl Display for ConfigurationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ConfigErrorKind::NotFound => {
                write!(f, "configuration file not found: {}", self.path.display())
            }
            ConfigErrorKind::ParseError(msg) => {
                write!(f, "failed to parse {}: {}", self.path.display(), msg)
            }
            ConfigErrorKind::InvalidValue { key, reason } => {
                write!(f, "invalid value for '{}' in {}: {}",
                    key, self.path.display(), reason)
            }
        }
    }
}

impl std::error::Error for ConfigurationError {}
```

### Guidelines

- Prefer specific error types over catch-all enum
- Store private `ErrorKind` with `is_xxx()` accessors
- Include backtrace (minimal overhead when not captured)
- Display should summarize what happened

---

## M-ESSENTIAL-FN-INHERENT

**Category:** Essential Functionality Should be Inherent

Implement core functionality inherently; traits forward to inherent implementations.

### Pattern

```rust
impl HttpClient {
    /// Downloads a file from the specified URL.
    pub fn download_file(&self, url: impl AsRef<str>) -> Result<Vec<u8>> {
        // Implementation
    }
}

// Trait forwards to inherent method
impl Download for HttpClient {
    fn download_file(&self, url: impl AsRef<str>) -> Result<Vec<u8>> {
        Self::download_file(self, url)
    }
}
```

---

## M-IMPL-ASREF

**Category:** Accept `impl AsRef<>` Where Feasible

```rust
// Good: Accepts &str, String, &String
fn print(x: impl AsRef<str>) {
    println!("{}", x.as_ref());
}

// Good: Accepts &Path, PathBuf, &str
fn read_file(x: impl AsRef<Path>) -> std::io::Result<String> {
    std::fs::read_to_string(x)
}

// Good: Accepts &[u8], Vec<u8>, &Vec<u8>
fn send_network(x: impl AsRef<[u8]>) { }
```

---

## M-IMPL-IO

**Category:** Accept `impl 'IO'` Where Feasible (Sans IO)

```rust
use std::io::{Read, Write};

fn parse_data(mut reader: impl Read) -> Result<Data, Error> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    // Parse buf...
}

fn write_output(mut writer: impl Write, data: &Data) -> std::io::Result<()> {
    writer.write_all(data.as_bytes())
}
```

---

## M-IMPL-RANGEBOUNDS

**Category:** Accept `impl RangeBounds<>` Where Feasible

```rust
use std::ops::RangeBounds;

// Good: Accepts 1..3, 1.., ..3, .., 1..=3
fn select_range(data: &[u8], range: impl RangeBounds<usize>) -> &[u8] {
    let start = match range.start_bound() {
        std::ops::Bound::Included(&n) => n,
        std::ops::Bound::Excluded(&n) => n + 1,
        std::ops::Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        std::ops::Bound::Included(&n) => n + 1,
        std::ops::Bound::Excluded(&n) => n,
        std::ops::Bound::Unbounded => data.len(),
    };
    &data[start..end]
}
```

---

## M-INIT-BUILDER

**Category:** Complex Type Construction has Builders

Types with 4+ initialization permutations should provide builders.

### Pattern

```rust
pub struct Client {
    endpoint: String,
    timeout: Duration,
    retries: u32,
    auth: Option<Auth>,
}

impl Client {
    /// Creates a builder for constructing a client.
    pub fn builder(endpoint: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(endpoint)
    }
}

pub struct ClientBuilder {
    endpoint: String,
    timeout: Duration,
    retries: u32,
    auth: Option<Auth>,
}

impl ClientBuilder {
    fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout: Duration::from_secs(30),
            retries: 3,
            auth: None,
        }
    }

    /// Sets the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the number of retries.
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Sets authentication credentials.
    pub fn auth(mut self, auth: Auth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Builds the client.
    pub fn build(self) -> Client {
        Client {
            endpoint: self.endpoint,
            timeout: self.timeout,
            retries: self.retries,
            auth: self.auth,
        }
    }
}
```

### Builder Conventions

- Name: `FooBuilder` for building `Foo`
- No public `new()` - use `Foo::builder()`
- Setter methods named `x()` not `set_x()`
- Required params passed to builder constructor

---

## M-INIT-CASCADED

**Category:** Complex Type Initialization Hierarchies are Cascaded

Types with 4+ parameters should cascade initialization via helper types.

### Anti-Pattern

```rust
fn new(bank: &str, customer: &str, currency: &str, amount: u64) -> Self { }
```

### Pattern

```rust
pub struct Transaction {
    account: Account,
    amount: Currency,
}

impl Transaction {
    pub fn new(account: Account, amount: Currency) -> Self {
        Self { account, amount }
    }
}

pub struct Account {
    bank: Bank,
    customer: Customer,
}

pub struct Currency {
    code: CurrencyCode,
    amount: u64,
}
```

---

## M-SERVICES-CLONE

**Category:** Services are Clone

Heavyweight service types should implement Clone via Arc<Inner>.

```rust
#[derive(Clone)]
pub struct DatabaseService {
    inner: Arc<DatabaseServiceInner>,
}

struct DatabaseServiceInner {
    pool: Pool,
    config: Config,
}

impl DatabaseService {
    pub fn new(config: Config) -> Self {
        Self {
            inner: Arc::new(DatabaseServiceInner {
                pool: Pool::new(&config),
                config,
            }),
        }
    }

    pub fn query(&self, sql: &str) -> Result<Rows> {
        self.inner.pool.query(sql)
    }
}
```

---

## M-SIMPLE-ABSTRACTIONS

**Category:** Abstractions Don't Visibly Nest

### Preference Hierarchy

- `Service` (Great)
- `Service<Backend>` (Acceptable)
- `Service<Backend<Store>>` (Bad)

### Rule of Thumb

Primary service APIs shouldn't nest on their own; maximum 1 level of type parameters.
