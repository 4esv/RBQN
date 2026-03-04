use std::fmt;

#[derive(Debug, Clone)]
pub enum BqnError {
    Type(String),
    Rank(String),
    Shape(String),
    Domain(String),
    Assert(String),
    Nyi(String),
}

impl fmt::Display for BqnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BqnError::Type(s) => write!(f, "Type error: {s}"),
            BqnError::Rank(s) => write!(f, "Rank error: {s}"),
            BqnError::Shape(s) => write!(f, "Shape error: {s}"),
            BqnError::Domain(s) => write!(f, "Domain error: {s}"),
            BqnError::Assert(s) => write!(f, "Assertion error: {s}"),
            BqnError::Nyi(s) => write!(f, "Not yet implemented: {s}"),
        }
    }
}

impl BqnError {
    /// Raw message for •CurrentError: Assert returns just the user string,
    /// other variants include their prefix (matching CBQN behavior).
    pub fn current_error_msg(&self) -> String {
        match self {
            BqnError::Assert(s) => s.clone(),
            other => other.to_string(),
        }
    }
}

impl std::error::Error for BqnError {}

pub type Result<T> = std::result::Result<T, BqnError>;

/// Panic with a typed BqnError value. Use this to propagate errors through
/// catch_unwind boundaries without losing the error variant.
pub fn throw_bqn(e: BqnError) -> ! {
    std::panic::panic_any(e)
}

pub fn throw(msg: impl Into<String>) -> ! {
    throw_bqn(BqnError::Domain(msg.into()))
}

pub fn throw_type(msg: impl Into<String>) -> ! {
    throw_bqn(BqnError::Type(msg.into()))
}

pub fn throw_rank(msg: impl Into<String>) -> ! {
    throw_bqn(BqnError::Rank(msg.into()))
}

pub fn throw_shape(msg: impl Into<String>) -> ! {
    throw_bqn(BqnError::Shape(msg.into()))
}

pub fn throw_nyi(msg: impl Into<String>) -> ! {
    throw_bqn(BqnError::Nyi(msg.into()))
}
