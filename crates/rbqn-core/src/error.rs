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

impl std::error::Error for BqnError {}

pub type Result<T> = std::result::Result<T, BqnError>;

pub fn throw(msg: impl Into<String>) -> BqnError {
    BqnError::Domain(msg.into())
}

pub fn throw_type(msg: impl Into<String>) -> BqnError {
    BqnError::Type(msg.into())
}

pub fn throw_rank(msg: impl Into<String>) -> BqnError {
    BqnError::Rank(msg.into())
}

pub fn throw_shape(msg: impl Into<String>) -> BqnError {
    BqnError::Shape(msg.into())
}

pub fn throw_nyi(msg: impl Into<String>) -> BqnError {
    BqnError::Nyi(msg.into())
}
