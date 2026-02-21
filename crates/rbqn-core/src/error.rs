use std::fmt;

#[derive(Debug)]
pub enum BqnError {
    Assertion(String),
    Type(String),
    Rank(String),
    Length(String),
    Domain(String),
    Nyi(String),
    Internal(String),
}

impl fmt::Display for BqnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BqnError::Assertion(s) => write!(f, "{s}"),
            BqnError::Type(s) => write!(f, "type: {s}"),
            BqnError::Rank(s) => write!(f, "rank: {s}"),
            BqnError::Length(s) => write!(f, "length: {s}"),
            BqnError::Domain(s) => write!(f, "domain: {s}"),
            BqnError::Nyi(s) => write!(f, "nyi: {s}"),
            BqnError::Internal(s) => write!(f, "internal: {s}"),
        }
    }
}

impl std::error::Error for BqnError {}

pub type Result<T> = std::result::Result<T, BqnError>;
