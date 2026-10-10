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
        // NOTE: CBQN prints no error kind, only the message ("Error: Stack overflow").
        match self {
            BqnError::Type(s)
            | BqnError::Rank(s)
            | BqnError::Shape(s)
            | BqnError::Domain(s)
            | BqnError::Assert(s) => f.write_str(s),
            BqnError::Nyi(s) => write!(f, "Not yet implemented: {s}"),
        }
    }
}

impl BqnError {
    /// Raw message for •CurrentError (same as Display, matching CBQN).
    pub fn current_error_msg(&self) -> String {
        self.to_string()
    }
}

impl std::error::Error for BqnError {}

/// Format a shape the way CBQN does in error messages: ⟨3⟩ for rank 1, 2‿3 otherwise.
pub fn fmt_shape(sh: &[usize]) -> String {
    match sh {
        [] => "⟨⟩".to_string(),
        [n] => format!("⟨{n}⟩"),
        _ => sh.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("‿"),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_has_no_kind_prefix() {
        assert_eq!(BqnError::Domain("Stack overflow".into()).to_string(), "Stack overflow");
        assert_eq!(BqnError::Shape("Mapping: x".into()).to_string(), "Mapping: x");
        assert_eq!(BqnError::Assert("msg".into()).current_error_msg(), "msg");
    }

    #[test]
    fn shape_format() {
        assert_eq!(fmt_shape(&[]), "⟨⟩");
        assert_eq!(fmt_shape(&[3]), "⟨3⟩");
        assert_eq!(fmt_shape(&[2, 3]), "2‿3");
    }
}
