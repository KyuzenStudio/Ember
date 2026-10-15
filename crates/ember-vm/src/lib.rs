pub mod format;
pub mod op;
pub mod verifier;

pub use format::{
    Chunk, DecodeError, Function, GlobalMeta, SpanEntry, decode, disassemble, encode,
};
pub use verifier::{VerifyError, verify};
