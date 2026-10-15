use std::fmt;

use ember_core::Ty;

use crate::op;

pub const MAGIC: [u8; 4] = *b"EMBC";
pub const FORMAT_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub source_file: String,
    pub strings: Vec<String>,
    pub globals: Vec<GlobalMeta>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlobalMeta {
    pub name: String,
    pub ty: Ty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: u16,
    pub locals: Vec<Ty>,
    pub code: Vec<u8>,
    pub spans: Vec<SpanEntry>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SpanEntry {
    pub offset: u32,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bytecode decode error: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

fn ty_to_u8(ty: Ty) -> u8 {
    match ty {
        Ty::I32 => 0,
        Ty::F32 => 1,
        Ty::Bool => 2,
        Ty::Str => 3,
        Ty::Void => 4,
    }
}

fn ty_from_u8(v: u8) -> Option<Ty> {
    match v {
        0 => Some(Ty::I32),
        1 => Some(Ty::F32),
        2 => Some(Ty::Bool),
        3 => Some(Ty::Str),
        4 => Some(Ty::Void),
        _ => None,
    }
}

pub fn encode(chunk: &Chunk) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    push_str(&mut out, &chunk.source_file);
    out.extend_from_slice(&(chunk.strings.len() as u32).to_le_bytes());
    for s in &chunk.strings {
        push_str(&mut out, s);
    }
    out.extend_from_slice(&(chunk.globals.len() as u32).to_le_bytes());
    for g in &chunk.globals {
        out.push(ty_to_u8(g.ty));
        push_str(&mut out, &g.name);
    }
    out.extend_from_slice(&(chunk.functions.len() as u32).to_le_bytes());
    for f in &chunk.functions {
        push_str(&mut out, &f.name);
        out.extend_from_slice(&f.params.to_le_bytes());
        out.extend_from_slice(&(f.locals.len() as u32).to_le_bytes());
        for t in &f.locals {
            out.push(ty_to_u8(*t));
        }
        out.extend_from_slice(&(f.code.len() as u32).to_le_bytes());
        out.extend_from_slice(&f.code);
        out.extend_from_slice(&(f.spans.len() as u32).to_le_bytes());
        for s in &f.spans {
            out.extend_from_slice(&s.offset.to_le_bytes());
            out.extend_from_slice(&s.line.to_le_bytes());
            out.extend_from_slice(&s.col.to_le_bytes());
        }
    }
    out
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| DecodeError("length overflow".to_string()))?;
        if end > self.b.len() {
            return Err(DecodeError(format!("truncated at byte {}", self.pos)));
        }
        let out = &self.b[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn string(&mut self) -> Result<String, DecodeError> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| DecodeError(format!("invalid utf-8 at byte {}", self.pos)))
    }
}

pub fn decode(bytes: &[u8]) -> Result<Chunk, DecodeError> {
    let mut c = Cur { b: bytes, pos: 0 };
    if c.take(4)? != MAGIC {
        return Err(DecodeError("bad magic".to_string()));
    }
    let version = c.u16()?;
    if version != FORMAT_VERSION {
        return Err(DecodeError(format!("unsupported version {version}")));
    }
    let source_file = c.string()?;
    let mut strings = Vec::new();
    for _ in 0..c.u32()? {
        strings.push(c.string()?);
    }
    let mut globals = Vec::new();
    for _ in 0..c.u32()? {
        let ty = ty_from_u8(c.u8()?)
            .ok_or_else(|| DecodeError(format!("bad global type at byte {}", c.pos)))?;
        let name = c.string()?;
        globals.push(GlobalMeta { name, ty });
    }
    let mut functions = Vec::new();
    for _ in 0..c.u32()? {
        let name = c.string()?;
        let params = c.u16()?;
        let mut locals = Vec::new();
        for _ in 0..c.u32()? {
            locals.push(
                ty_from_u8(c.u8()?)
                    .ok_or_else(|| DecodeError(format!("bad local type at byte {}", c.pos)))?,
            );
        }
        let code_len = c.u32()? as usize;
        let code = c.take(code_len)?.to_vec();
        let mut spans = Vec::new();
        for _ in 0..c.u32()? {
            spans.push(SpanEntry {
                offset: c.u32()?,
                line: c.u32()?,
                col: c.u32()?,
            });
        }
        functions.push(Function {
            name,
            params,
            locals,
            code,
            spans,
        });
    }
    if c.pos != bytes.len() {
        return Err(DecodeError(format!(
            "{} trailing byte(s)",
            bytes.len() - c.pos
        )));
    }
    Ok(Chunk {
        source_file,
        strings,
        globals,
        functions,
    })
}

pub fn disassemble(chunk: &Chunk) -> String {
    let mut out = String::new();
    for f in &chunk.functions {
        out.push_str(&format!(
            "fn {} (params {}, slots {}):\n",
            f.name,
            f.params,
            f.locals.len()
        ));
        let mut off = 0usize;
        while off < f.code.len() {
            let opcode = f.code[off];
            match op::name(opcode).zip(op::len(opcode)) {
                Some((name, len)) if off + len <= f.code.len() => {
                    out.push_str(&format!("  {off:04x}: {name}"));
                    match len {
                        3 => out.push_str(&format!(
                            " {}",
                            u16::from_le_bytes([f.code[off + 1], f.code[off + 2]])
                        )),
                        5 if opcode == op::CONST_I32 => out.push_str(&format!(
                            " {}",
                            i32::from_le_bytes([
                                f.code[off + 1],
                                f.code[off + 2],
                                f.code[off + 3],
                                f.code[off + 4]
                            ])
                        )),
                        5 if opcode == op::CONST_F32 => out.push_str(&format!(
                            " {}",
                            f32::from_le_bytes([
                                f.code[off + 1],
                                f.code[off + 2],
                                f.code[off + 3],
                                f.code[off + 4]
                            ])
                        )),
                        5 => out.push_str(&format!(
                            " {}",
                            u32::from_le_bytes([
                                f.code[off + 1],
                                f.code[off + 2],
                                f.code[off + 3],
                                f.code[off + 4]
                            ])
                        )),
                        _ => {}
                    }
                    out.push('\n');
                    off += len;
                }
                _ => {
                    out.push_str(&format!("  {off:04x}: db 0x{opcode:02x}\n"));
                    off += 1;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Chunk {
        Chunk {
            source_file: "t.em".to_string(),
            strings: vec!["hi".to_string()],
            globals: vec![GlobalMeta {
                name: "g".to_string(),
                ty: Ty::I32,
            }],
            functions: vec![Function {
                name: "f".to_string(),
                params: 0,
                locals: vec![],
                code: vec![op::CONST_I32, 7, 0, 0, 0, op::RETURN],
                spans: vec![SpanEntry {
                    offset: 0,
                    line: 1,
                    col: 1,
                }],
            }],
        }
    }

    #[test]
    fn round_trip() {
        let chunk = sample();
        assert_eq!(decode(&encode(&chunk)).unwrap(), chunk);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = encode(&sample());
        bytes[0] = 0;
        assert!(decode(&bytes).unwrap_err().to_string().contains("magic"));
    }

    #[test]
    fn rejects_bad_version() {
        let mut bytes = encode(&sample());
        bytes[4] = 9;
        assert!(decode(&bytes).unwrap_err().to_string().contains("version"));
    }

    #[test]
    fn rejects_truncated() {
        let bytes = encode(&sample());
        let err = decode(&bytes[..bytes.len() - 3]).unwrap_err();
        assert!(err.to_string().contains("truncated"));
    }

    #[test]
    fn rejects_trailing() {
        let mut bytes = encode(&sample());
        bytes.push(0);
        assert!(decode(&bytes).unwrap_err().to_string().contains("trailing"));
    }

    #[test]
    fn disassembly_names_ops() {
        let text = disassemble(&sample());
        assert!(text.contains("const.i32 7"));
        assert!(text.contains("return"));
    }
}
