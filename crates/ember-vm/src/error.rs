use std::fmt;

use ember_core::Ty;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    DivisionByZero,
    BudgetExhausted,
    CallDepthExceeded,
    UnknownFunction(String),
    ArityMismatch { want: usize, got: usize },
    TypeMismatch { want: Ty, got: Ty },
    StackUnderflow,
    BadIndex(String),
    BadOpcode(u8),
    HostFailed(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::DivisionByZero => write!(f, "division by zero"),
            ErrorKind::BudgetExhausted => write!(f, "instruction budget exhausted"),
            ErrorKind::CallDepthExceeded => write!(f, "call depth exceeded"),
            ErrorKind::UnknownFunction(name) => write!(f, "unknown function '{name}'"),
            ErrorKind::ArityMismatch { want, got } => {
                write!(f, "takes {want} argument(s), got {got}")
            }
            ErrorKind::TypeMismatch { want, got } => write!(f, "expected {want}, found {got}"),
            ErrorKind::StackUnderflow => write!(f, "stack underflow"),
            ErrorKind::BadIndex(what) => write!(f, "bad {what}"),
            ErrorKind::BadOpcode(op) => write!(f, "invalid opcode 0x{op:02x}"),
            ErrorKind::HostFailed(msg) => write!(f, "host error: {msg}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub kind: ErrorKind,
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub stack: Vec<String>,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:{}:{}: {}", self.file, self.line, self.col, self.kind)?;
        for name in &self.stack {
            writeln!(f, "  in {name}")?;
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeError {}
