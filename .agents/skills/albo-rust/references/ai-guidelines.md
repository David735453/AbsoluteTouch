# AI Guidelines

## M-DESIGN-FOR-AI

**Category:** Design with AI Use in Mind

Maximize utility from AI agents working in your codebase by following guidelines that make APIs easier for both humans and AI.

### Key Points

- Create idiomatic Rust API patterns following official guidelines
- Provide thorough documentation for all modules and public items
- Include directly usable examples in documentation
- Use strong types to avoid primitive obsession
- Design APIs that are testable and mockable
- Ensure good test coverage for observable behavior

### Rationale

Rust's strong type system is a boon for agents, as their lack of genuine understanding can often be counterbalanced by comprehensive compiler checks.

### Checklist

- [ ] Follow Rust API Guidelines
- [ ] Document all public items with examples
- [ ] Use newtypes instead of primitives where meaningful
- [ ] Design for testability and mockability
- [ ] Maintain comprehensive test coverage

### Example

```rust
// Good: Strong types, documented, testable
/// A user identifier in the system.
///
/// # Examples
///
/// ```
/// let id = UserId::new(42);
/// assert_eq!(id.as_u64(), 42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

impl UserId {
    /// Creates a new user identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the underlying identifier value.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

// Bad: Primitive obsession, undocumented
pub fn get_user(id: u64) -> User { }
```
