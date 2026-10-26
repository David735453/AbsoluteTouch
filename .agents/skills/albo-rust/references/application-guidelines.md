# Application Guidelines

## M-APP-ERROR

**Category:** Applications May Use Anyhow or Derivatives

Applications can use application-level error crates like `anyhow` or `eyre` instead of custom error types.

### Key Points

- Applications and internal-only crates may re-export common Result types
- Select one application error crate and use consistently
- Libraries must follow canonical struct patterns instead (see M-ERRORS-CANONICAL-STRUCTS)

### Example

```rust
use eyre::Result;

fn start_application() -> Result<()> {
    initialize_logging()?;
    load_configuration()?;
    start_server()?;
    Ok(())
}

// Or with anyhow:
use anyhow::{Context, Result};

fn load_config() -> Result<Config> {
    let path = find_config_file()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;
    toml::from_str(&content)
        .context("failed to parse config")
}
```

### When to Use

- Binary crates (applications)
- Internal-only crates not exposed as libraries
- Prototyping and quick scripts

### When NOT to Use

- Public libraries (use M-ERRORS-CANONICAL-STRUCTS)
- APIs where callers need to match on specific error variants

---

## M-MIMALLOC-APPS

**Category:** Use Mimalloc for Applications

Applications should set mimalloc as their global allocator for performance gains.

### Benefits

Up to 25% benchmark improvements on allocation hot paths.

### Implementation

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // Application code...
}
```

### Cargo.toml

```toml
[dependencies]
mimalloc = "0.1"
```

### When to Use

- Applications with significant heap allocation
- Performance-critical binaries
- Server applications

### Considerations

- Increases binary size slightly
- May not benefit all workloads equally
- Profile before and after to verify gains
