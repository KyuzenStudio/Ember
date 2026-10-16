use std::collections::{BTreeSet, HashMap};
use std::fmt;

use ember_core::{Ty, host};

use crate::format::{Chunk, Function};
use crate::op;

pub const MAX_STACK_DEPTH: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyError {
    pub function: String,
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "verify error in '{}' at {:04x}: {}",
            self.function, self.offset, self.message
        )
    }
}

impl std::error::Error for VerifyError {}

pub fn verify(chunk: &Chunk) -> Result<(), VerifyError> {
    for f in &chunk.functions {
        verify_fn(chunk, f)?;
    }
    Ok(())
}

fn err(f: &Function, offset: usize, message: String) -> VerifyError {
    VerifyError {
        function: f.name.clone(),
        offset,
        message,
    }
}

fn verify_fn(chunk: &Chunk, f: &Function) -> Result<(), VerifyError> {
    check_structure(f)?;
    check_types(chunk, f)
}

fn check_structure(f: &Function) -> Result<BTreeSet<usize>, VerifyError> {
    if f.params as usize > f.locals.len() {
        return Err(err(
            f,
            0,
            format!("{} params but {} slots", f.params, f.locals.len()),
        ));
    }
    if f.code.is_empty() {
        return Err(err(f, 0, "empty function".to_string()));
    }
    let mut bounds = BTreeSet::new();
    let mut off = 0usize;
    while off < f.code.len() {
        let opcode = f.code[off];
        let len =
            op::len(opcode).ok_or_else(|| err(f, off, format!("invalid opcode 0x{opcode:02x}")))?;
        if off + len > f.code.len() {
            return Err(err(f, off, "truncated instruction".to_string()));
        }
        bounds.insert(off);
        off += len;
    }
    let last_start = *bounds.last().unwrap_or(&0);
    if f.code[last_start] != op::RETURN {
        return Err(err(
            f,
            last_start,
            "function does not end with return".to_string(),
        ));
    }
    for s in &f.spans {
        if !bounds.contains(&(s.offset as usize)) {
            return Err(err(
                f,
                s.offset as usize,
                "source map points outside an instruction".to_string(),
            ));
        }
    }
    for &start in &bounds {
        match f.code[start] {
            op::JUMP | op::JUMP_IF_FALSE => {
                let target = u32::from_le_bytes([
                    f.code[start + 1],
                    f.code[start + 2],
                    f.code[start + 3],
                    f.code[start + 4],
                ]) as usize;
                if !bounds.contains(&target) {
                    return Err(err(
                        f,
                        start,
                        format!("jump target {target} is not an instruction"),
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(bounds)
}

fn check_types(chunk: &Chunk, f: &Function) -> Result<(), VerifyError> {
    let u16_at = |at: usize| u16::from_le_bytes([f.code[at], f.code[at + 1]]) as usize;
    let u32_at = |at: usize| {
        u32::from_le_bytes([f.code[at], f.code[at + 1], f.code[at + 2], f.code[at + 3]]) as usize
    };
    let mut entry: HashMap<usize, Vec<Ty>> = HashMap::new();
    let mut work = vec![(0usize, Vec::new())];
    while let Some((at, stack)) = work.pop() {
        if let Some(seen) = entry.get(&at) {
            if *seen != stack {
                return Err(err(f, at, "inconsistent stack at join".to_string()));
            }
            continue;
        }
        entry.insert(at, stack.clone());
        let mut stack = stack;
        let opcode = f.code[at];
        let len = op::len(opcode).unwrap_or(1);
        let pop = |stack: &mut Vec<Ty>, want: Option<Ty>| -> Result<Ty, VerifyError> {
            match stack.pop() {
                None => Err(err(f, at, "stack underflow".to_string())),
                Some(got) => match want {
                    Some(w) if w != got => Err(err(f, at, format!("expected {w}, found {got}"))),
                    _ => Ok(got),
                },
            }
        };
        match opcode {
            op::CONST_I32 => stack.push(Ty::I32),
            op::CONST_F32 => stack.push(Ty::F32),
            op::CONST_TRUE | op::CONST_FALSE => stack.push(Ty::Bool),
            op::CONST_STR => {
                let idx = u16_at(at + 1);
                if idx >= chunk.strings.len() {
                    return Err(err(f, at, format!("string {idx} out of range")));
                }
                stack.push(Ty::Str);
            }
            op::LOAD_LOCAL => {
                let idx = u16_at(at + 1);
                match f.locals.get(idx) {
                    Some(t) => stack.push(*t),
                    None => return Err(err(f, at, format!("local {idx} out of range"))),
                }
            }
            op::STORE_LOCAL => {
                let idx = u16_at(at + 1);
                match f.locals.get(idx) {
                    Some(t) => {
                        pop(&mut stack, Some(*t))?;
                    }
                    None => return Err(err(f, at, format!("local {idx} out of range"))),
                }
            }
            op::LOAD_GLOBAL => {
                let idx = u16_at(at + 1);
                match chunk.globals.get(idx) {
                    Some(g) => stack.push(g.ty),
                    None => return Err(err(f, at, format!("global {idx} out of range"))),
                }
            }
            op::STORE_GLOBAL => {
                let idx = u16_at(at + 1);
                match chunk.globals.get(idx) {
                    Some(g) => {
                        pop(&mut stack, Some(g.ty))?;
                    }
                    None => return Err(err(f, at, format!("global {idx} out of range"))),
                }
            }
            op::ADD_I | op::SUB_I | op::MUL_I | op::DIV_I | op::MOD_I => {
                pop(&mut stack, Some(Ty::I32))?;
                pop(&mut stack, Some(Ty::I32))?;
                stack.push(Ty::I32);
            }
            op::NEG_I => {
                pop(&mut stack, Some(Ty::I32))?;
                stack.push(Ty::I32);
            }
            op::ADD_F | op::SUB_F | op::MUL_F | op::DIV_F | op::MOD_F => {
                pop(&mut stack, Some(Ty::F32))?;
                pop(&mut stack, Some(Ty::F32))?;
                stack.push(Ty::F32);
            }
            op::NEG_F => {
                pop(&mut stack, Some(Ty::F32))?;
                stack.push(Ty::F32);
            }
            op::EQ_I | op::NE_I | op::LT_I | op::LE_I | op::GT_I | op::GE_I => {
                pop(&mut stack, Some(Ty::I32))?;
                pop(&mut stack, Some(Ty::I32))?;
                stack.push(Ty::Bool);
            }
            op::EQ_F | op::NE_F | op::LT_F | op::LE_F | op::GT_F | op::GE_F => {
                pop(&mut stack, Some(Ty::F32))?;
                pop(&mut stack, Some(Ty::F32))?;
                stack.push(Ty::Bool);
            }
            op::EQ_B | op::NE_B => {
                pop(&mut stack, Some(Ty::Bool))?;
                pop(&mut stack, Some(Ty::Bool))?;
                stack.push(Ty::Bool);
            }
            op::EQ_S | op::NE_S => {
                pop(&mut stack, Some(Ty::Str))?;
                pop(&mut stack, Some(Ty::Str))?;
                stack.push(Ty::Bool);
            }
            op::AND | op::OR => {
                pop(&mut stack, Some(Ty::Bool))?;
                pop(&mut stack, Some(Ty::Bool))?;
                stack.push(Ty::Bool);
            }
            op::NOT => {
                pop(&mut stack, Some(Ty::Bool))?;
                stack.push(Ty::Bool);
            }
            op::JUMP => {
                work.push((u32_at(at + 1), stack));
                continue;
            }
            op::JUMP_IF_FALSE => {
                pop(&mut stack, Some(Ty::Bool))?;
                work.push((u32_at(at + 1), stack.clone()));
            }
            op::CALL_FUNC => {
                let idx = u16_at(at + 1);
                let callee = chunk
                    .functions
                    .get(idx)
                    .ok_or_else(|| err(f, at, format!("function {idx} out of range")))?;
                let mut args = Vec::new();
                for _ in 0..callee.params {
                    args.push(pop(&mut stack, None)?);
                }
                args.reverse();
                for (i, (got, want)) in args.iter().zip(callee.locals.iter()).enumerate() {
                    if got != want {
                        return Err(err(
                            f,
                            at,
                            format!("call argument {} must be {want}, found {got}", i + 1),
                        ));
                    }
                }
            }
            op::CALL_HOST => {
                let idx = u16_at(at + 1);
                let host = host::HOST_FUNCTIONS
                    .get(idx)
                    .ok_or_else(|| err(f, at, format!("host function {idx} out of range")))?;
                let mut args = Vec::new();
                for _ in 0..host.params.len() {
                    args.push(pop(&mut stack, None)?);
                }
                args.reverse();
                for (i, (got, want)) in args.iter().zip(host.params.iter()).enumerate() {
                    if got != want {
                        return Err(err(
                            f,
                            at,
                            format!("host argument {} must be {want}, found {got}", i + 1),
                        ));
                    }
                }
                if host.ret != Ty::Void {
                    stack.push(host.ret);
                }
            }
            op::RETURN => {
                if !stack.is_empty() {
                    return Err(err(
                        f,
                        at,
                        format!("return with {} value(s) on stack", stack.len()),
                    ));
                }
                continue;
            }
            _ => return Err(err(f, at, format!("invalid opcode 0x{opcode:02x}"))),
        }
        if stack.len() > MAX_STACK_DEPTH {
            return Err(err(f, at, "stack depth exceeds limit".to_string()));
        }
        let next = at + len;
        if next < f.code.len() {
            work.push((next, stack));
        } else {
            return Err(err(f, at, "missing return".to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{Function, GlobalMeta, SpanEntry};

    fn func(code: Vec<u8>) -> Function {
        Function {
            name: "f".to_string(),
            params: 0,
            locals: vec![],
            code,
            spans: vec![],
        }
    }

    fn chunk_with(code: Vec<u8>) -> Chunk {
        Chunk {
            source_file: "t.em".to_string(),
            strings: vec!["s".to_string()],
            globals: vec![GlobalMeta {
                name: "g".to_string(),
                ty: Ty::I32,
            }],
            functions: vec![func(code)],
        }
    }

    fn ok(code: Vec<u8>) {
        verify(&chunk_with(code)).unwrap();
    }

    fn bad(code: Vec<u8>) -> String {
        verify(&chunk_with(code)).unwrap_err().to_string()
    }

    fn i32_bytes(v: i32) -> Vec<u8> {
        let mut out = vec![op::CONST_I32];
        out.extend_from_slice(&v.to_le_bytes());
        out
    }

    #[test]
    fn accepts_const_return() {
        let mut code = i32_bytes(7);
        code.extend_from_slice(&[op::STORE_GLOBAL, 0, 0, op::RETURN]);
        ok(code);
    }

    #[test]
    fn rejects_bad_opcode() {
        assert!(bad(vec![0xFF, op::RETURN]).contains("invalid opcode"));
    }

    #[test]
    fn rejects_truncated() {
        assert!(bad(vec![op::CONST_I32, 1, 2]).contains("truncated"));
    }

    #[test]
    fn rejects_oob_jump() {
        let code = vec![op::JUMP, 99, 0, 0, 0, op::RETURN];
        assert!(bad(code).contains("not an instruction"));
    }

    #[test]
    fn rejects_mid_instruction_jump() {
        let mut code = i32_bytes(7);
        code.push(op::RETURN);
        let mut bad_code = vec![op::JUMP, 2, 0, 0, 0];
        bad_code.extend_from_slice(&code);
        assert!(bad(bad_code).contains("not an instruction"));
    }

    #[test]
    fn rejects_str_oob() {
        let code = vec![op::CONST_STR, 9, 0, op::RETURN];
        assert!(bad(code).contains("out of range"));
    }

    #[test]
    fn rejects_local_oob() {
        let code = vec![op::LOAD_LOCAL, 4, 0, op::RETURN];
        assert!(bad(code).contains("out of range"));
    }

    #[test]
    fn rejects_underflow() {
        assert!(bad(vec![op::ADD_I, op::RETURN]).contains("underflow"));
    }

    #[test]
    fn rejects_type_mismatch() {
        let mut code = i32_bytes(1);
        code.extend_from_slice(&[op::CONST_F32, 0, 0, 0, 0, op::ADD_I, op::RETURN]);
        assert!(bad(code).contains("expected i32, found f32"));
    }

    #[test]
    fn rejects_return_with_values() {
        let mut code = i32_bytes(1);
        code.push(op::RETURN);
        assert!(bad(code).contains("on stack"));
    }

    #[test]
    fn rejects_missing_return() {
        let code = i32_bytes(1);
        assert!(bad(code).contains("return"));
    }

    #[test]
    fn rejects_call_bad_idx() {
        let code = vec![op::CALL_FUNC, 7, 0, op::RETURN];
        assert!(bad(code).contains("out of range"));
    }

    #[test]
    fn rejects_inconsistent_join() {
        let mut code = i32_bytes(1);
        code.extend_from_slice(&[op::JUMP, 0, 0, 0, 0, op::RETURN]);
        assert!(bad(code).contains("inconsistent stack"));
    }

    #[test]
    fn rejects_bad_span() {
        let mut chunk = chunk_with(vec![op::RETURN]);
        chunk.functions[0].spans.push(SpanEntry {
            offset: 9,
            line: 1,
            col: 1,
        });
        assert!(
            verify(&chunk)
                .unwrap_err()
                .to_string()
                .contains("source map")
        );
    }

    #[test]
    fn error_display() {
        let e = bad(vec![0xFF, op::RETURN]);
        assert!(e.contains("'f'"));
    }
}
