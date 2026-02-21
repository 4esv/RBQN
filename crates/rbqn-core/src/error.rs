use std::fmt;

#[derive(Debug, Clone)]
pub struct BqnError {
    pub message: String,
    pub stack: Vec<StackFrame>,
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub src: Option<String>,
    pub pos: Option<(usize, usize)>,
}

impl fmt::Display for BqnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BqnError {}

pub type Result<T> = std::result::Result<T, BqnError>;

pub fn throw(msg: impl Into<String>) -> ! {
    panic!("{}", msg.into())
}
