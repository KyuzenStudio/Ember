use std::fmt;

use ember_core::{Ty, host};

use crate::format::Chunk;
use crate::op;
use crate::verifier;

pub const DEFAULT_BUDGET: u64 = 10_000_000;
pub const MAX_CALL_DEPTH: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I32(i32),
    F32(f32),
    Bool(bool),
    Str(String),
}

impl Value {
    fn default_for(ty: Ty) -> Self {
        match ty {
            Ty::I32 => Value::I32(0),
            Ty::F32 => Value::F32(0.0),
            Ty::Bool => Value::Bool(false),
            Ty::Str => Value::Str(String::new()),
            Ty::Void => Value::Bool(false),
        }
    }

    fn ty(&self) -> Ty {
        match self {
            Value::I32(_) => Ty::I32,
            Value::F32(_) => Ty::F32,
            Value::Bool(_) => Ty::Bool,
            Value::Str(_) => Ty::Str,
        }
    }
}

pub trait Host {
    fn input_down(&mut self, key: i32) -> bool;
    fn transform_move(&mut self, dx: f32, dy: f32);
    fn log(&mut self, msg: &str);
}

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

#[derive(Debug)]
struct Frame {
    func_idx: usize,
    ip: usize,
    locals: Vec<Value>,
    base: usize,
}

#[derive(Debug)]
pub struct Vm {
    chunk: Chunk,
    globals: Vec<Value>,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    budget: u64,
}

impl Vm {
    pub fn new(chunk: Chunk) -> Result<Self, crate::VerifyError> {
        verifier::verify(&chunk)?;
        let globals = chunk
            .globals
            .iter()
            .map(|g| Value::default_for(g.ty))
            .collect();
        Ok(Self {
            chunk,
            globals,
            stack: Vec::new(),
            frames: Vec::new(),
            budget: DEFAULT_BUDGET,
        })
    }

    pub fn set_budget(&mut self, budget: u64) {
        self.budget = budget;
    }

    pub fn globals(&self) -> &[Value] {
        &self.globals
    }

    pub fn global_index(&self, name: &str) -> Option<usize> {
        self.chunk.globals.iter().position(|g| g.name == name)
    }

    pub fn set_global(&mut self, idx: usize, value: Value) -> Result<(), RuntimeError> {
        let want = self
            .globals
            .get(idx)
            .map(|g| g.ty())
            .ok_or_else(|| self.failure(ErrorKind::BadIndex("global".to_string())))?;
        if value.ty() != want {
            return Err(self.failure(ErrorKind::TypeMismatch {
                want,
                got: value.ty(),
            }));
        }
        self.globals[idx] = value;
        Ok(())
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.chunk.functions.iter().any(|f| f.name == name)
    }

    pub fn call(
        &mut self,
        host: &mut dyn Host,
        name: &str,
        args: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let idx = self
            .chunk
            .functions
            .iter()
            .position(|f| f.name == name)
            .ok_or_else(|| self.failure(ErrorKind::UnknownFunction(name.to_string())))?;
        let want = self.chunk.functions[idx].params as usize;
        if want != args.len() {
            return Err(self.failure(ErrorKind::ArityMismatch {
                want,
                got: args.len(),
            }));
        }
        for (got, meta) in args.iter().zip(self.chunk.functions[idx].locals.iter()) {
            if got.ty() != *meta {
                return Err(self.failure(ErrorKind::TypeMismatch {
                    want: *meta,
                    got: got.ty(),
                }));
            }
        }
        if self.frames.len() + 1 > MAX_CALL_DEPTH {
            return Err(self.failure(ErrorKind::CallDepthExceeded));
        }
        let mut locals: Vec<Value> = args;
        for ty in self.chunk.functions[idx].locals.iter().skip(locals.len()) {
            locals.push(Value::default_for(*ty));
        }
        self.frames.push(Frame {
            func_idx: idx,
            ip: 0,
            locals,
            base: self.stack.len(),
        });
        self.run(host)
    }

    fn failure(&self, kind: ErrorKind) -> RuntimeError {
        RuntimeError {
            kind,
            file: self.chunk.source_file.clone(),
            line: 1,
            col: 1,
            stack: self.trace(),
        }
    }

    fn failure_at(&self, ip: usize, kind: ErrorKind) -> RuntimeError {
        let (line, col) = self.locate(ip);
        RuntimeError {
            kind,
            file: self.chunk.source_file.clone(),
            line,
            col,
            stack: self.trace(),
        }
    }

    fn trace(&self) -> Vec<String> {
        self.frames
            .iter()
            .map(|fr| self.chunk.functions[fr.func_idx].name.clone())
            .collect()
    }

    fn locate(&self, ip: usize) -> (u32, u32) {
        let frame = match self.frames.last() {
            Some(fr) => fr,
            None => return (1, 1),
        };
        let spans = &self.chunk.functions[frame.func_idx].spans;
        let mut line = 1;
        let mut col = 1;
        for s in spans {
            if (s.offset as usize) <= ip {
                line = s.line;
                col = s.col;
            } else {
                break;
            }
        }
        (line, col)
    }

    fn run(&mut self, host: &mut dyn Host) -> Result<(), RuntimeError> {
        while let Some(ip) = self.frames.last().map(|fr| fr.ip) {
            if self.budget == 0 {
                return Err(self.failure_at(ip, ErrorKind::BudgetExhausted));
            }
            self.budget -= 1;
            let opcode = self.code_byte(ip)?;
            match opcode {
                op::CONST_I32 => {
                    let v = self.i32_at(ip + 1)?;
                    self.push(Value::I32(v), ip)?;
                    self.advance(5)?;
                }
                op::CONST_F32 => {
                    let v = self.f32_at(ip + 1)?;
                    self.push(Value::F32(v), ip)?;
                    self.advance(5)?;
                }
                op::CONST_TRUE => {
                    self.push(Value::Bool(true), ip)?;
                    self.advance(1)?;
                }
                op::CONST_FALSE => {
                    self.push(Value::Bool(false), ip)?;
                    self.advance(1)?;
                }
                op::CONST_STR => {
                    let idx = self.u16_at(ip + 1)? as usize;
                    let s = self.chunk.strings.get(idx).ok_or_else(|| {
                        self.failure_at(ip, ErrorKind::BadIndex("string".to_string()))
                    })?;
                    self.push(Value::Str(s.clone()), ip)?;
                    self.advance(3)?;
                }
                op::LOAD_LOCAL => {
                    let v = self.local_get(self.u16_at(ip + 1)? as usize, ip)?;
                    self.push(v, ip)?;
                    self.advance(3)?;
                }
                op::STORE_LOCAL => {
                    let v = self.pop(ip)?;
                    self.local_set(self.u16_at(ip + 1)? as usize, v, ip)?;
                    self.advance(3)?;
                }
                op::LOAD_GLOBAL => {
                    let idx = self.u16_at(ip + 1)? as usize;
                    let v = self
                        .globals
                        .get(idx)
                        .ok_or_else(|| {
                            self.failure_at(ip, ErrorKind::BadIndex("global".to_string()))
                        })?
                        .clone();
                    self.push(v, ip)?;
                    self.advance(3)?;
                }
                op::STORE_GLOBAL => {
                    let v = self.pop(ip)?;
                    let idx = self.u16_at(ip + 1)? as usize;
                    let want = self.globals.get(idx).map(|g| g.ty()).ok_or_else(|| {
                        self.failure_at(ip, ErrorKind::BadIndex("global".to_string()))
                    })?;
                    if v.ty() != want {
                        return Err(
                            self.failure_at(ip, ErrorKind::TypeMismatch { want, got: v.ty() })
                        );
                    }
                    self.globals[idx] = v;
                    self.advance(3)?;
                }
                op::ADD_I => self.binary_i(ip, |a, b| a.wrapping_add(b))?,
                op::SUB_I => self.binary_i(ip, |a, b| a.wrapping_sub(b))?,
                op::MUL_I => self.binary_i(ip, |a, b| a.wrapping_mul(b))?,
                op::DIV_I => {
                    let (a, b) = self.pop2_i32(ip)?;
                    if b == 0 {
                        return Err(self.failure_at(ip, ErrorKind::DivisionByZero));
                    }
                    self.push(Value::I32(a.wrapping_div(b)), ip)?;
                    self.advance(1)?;
                }
                op::MOD_I => {
                    let (a, b) = self.pop2_i32(ip)?;
                    if b == 0 {
                        return Err(self.failure_at(ip, ErrorKind::DivisionByZero));
                    }
                    self.push(Value::I32(a.wrapping_rem(b)), ip)?;
                    self.advance(1)?;
                }
                op::NEG_I => {
                    let a = self.pop_i32(ip)?;
                    self.push(Value::I32(a.wrapping_neg()), ip)?;
                    self.advance(1)?;
                }
                op::ADD_F => self.binary_f(ip, |a, b| a + b)?,
                op::SUB_F => self.binary_f(ip, |a, b| a - b)?,
                op::MUL_F => self.binary_f(ip, |a, b| a * b)?,
                op::DIV_F => self.binary_f(ip, |a, b| a / b)?,
                op::MOD_F => self.binary_f(ip, |a, b| a % b)?,
                op::NEG_F => {
                    let a = self.pop_f32(ip)?;
                    self.push(Value::F32(-a), ip)?;
                    self.advance(1)?;
                }
                op::EQ_I => self.cmp_i(ip, |a, b| a == b)?,
                op::NE_I => self.cmp_i(ip, |a, b| a != b)?,
                op::LT_I => self.cmp_i(ip, |a, b| a < b)?,
                op::LE_I => self.cmp_i(ip, |a, b| a <= b)?,
                op::GT_I => self.cmp_i(ip, |a, b| a > b)?,
                op::GE_I => self.cmp_i(ip, |a, b| a >= b)?,
                op::EQ_F => self.cmp_f(ip, |a, b| a == b)?,
                op::NE_F => self.cmp_f(ip, |a, b| a != b)?,
                op::LT_F => self.cmp_f(ip, |a, b| a < b)?,
                op::LE_F => self.cmp_f(ip, |a, b| a <= b)?,
                op::GT_F => self.cmp_f(ip, |a, b| a > b)?,
                op::GE_F => self.cmp_f(ip, |a, b| a >= b)?,
                op::EQ_B => self.cmp_b(ip, |a, b| a == b)?,
                op::NE_B => self.cmp_b(ip, |a, b| a != b)?,
                op::EQ_S => {
                    let (a, b) = self.pop2_str(ip)?;
                    self.push(Value::Bool(a == b), ip)?;
                    self.advance(1)?;
                }
                op::NE_S => {
                    let (a, b) = self.pop2_str(ip)?;
                    self.push(Value::Bool(a != b), ip)?;
                    self.advance(1)?;
                }
                op::AND => {
                    let (a, b) = self.pop2_bool(ip)?;
                    self.push(Value::Bool(a && b), ip)?;
                    self.advance(1)?;
                }
                op::OR => {
                    let (a, b) = self.pop2_bool(ip)?;
                    self.push(Value::Bool(a || b), ip)?;
                    self.advance(1)?;
                }
                op::NOT => {
                    let a = self.pop_bool(ip)?;
                    self.push(Value::Bool(!a), ip)?;
                    self.advance(1)?;
                }
                op::JUMP => {
                    let target = self.u32_at(ip + 1)? as usize;
                    self.jump(target, ip)?;
                }
                op::JUMP_IF_FALSE => {
                    let target = self.u32_at(ip + 1)? as usize;
                    let cond = self.pop_bool(ip)?;
                    if !cond {
                        self.jump(target, ip)?;
                    } else {
                        self.advance(5)?;
                    }
                }
                op::CALL_FUNC => {
                    let idx = self.u16_at(ip + 1)? as usize;
                    if idx >= self.chunk.functions.len() {
                        return Err(
                            self.failure_at(ip, ErrorKind::BadIndex("function".to_string()))
                        );
                    }
                    let callee_params = self.chunk.functions[idx].params as usize;
                    let mut args = Vec::with_capacity(callee_params);
                    for _ in 0..callee_params {
                        args.push(self.pop(ip)?);
                    }
                    args.reverse();
                    for (got, want) in args.iter().zip(self.chunk.functions[idx].locals.iter()) {
                        if got.ty() != *want {
                            return Err(self.failure_at(
                                ip,
                                ErrorKind::TypeMismatch {
                                    want: *want,
                                    got: got.ty(),
                                },
                            ));
                        }
                    }
                    if self.frames.len() + 1 > MAX_CALL_DEPTH {
                        return Err(self.failure_at(ip, ErrorKind::CallDepthExceeded));
                    }
                    let mut locals = args;
                    for ty in self.chunk.functions[idx].locals.iter().skip(locals.len()) {
                        locals.push(Value::default_for(*ty));
                    }
                    self.advance(3)?;
                    self.frames.push(Frame {
                        func_idx: idx,
                        ip: 0,
                        locals,
                        base: self.stack.len(),
                    });
                }
                op::CALL_HOST => {
                    let idx = self.u16_at(ip + 1)? as usize;
                    let sig = host::HOST_FUNCTIONS.get(idx).ok_or_else(|| {
                        self.failure_at(ip, ErrorKind::BadIndex("host function".to_string()))
                    })?;
                    let mut args = Vec::with_capacity(sig.params.len());
                    for _ in 0..sig.params.len() {
                        args.push(self.pop(ip)?);
                    }
                    args.reverse();
                    for (got, want) in args.iter().zip(sig.params.iter()) {
                        if got.ty() != *want {
                            return Err(self.failure_at(
                                ip,
                                ErrorKind::TypeMismatch {
                                    want: *want,
                                    got: got.ty(),
                                },
                            ));
                        }
                    }
                    let ret = self.host_call(host, sig.path, &args, ip)?;
                    if let Some(v) = ret {
                        self.push(v, ip)?;
                    }
                    self.advance(3)?;
                }
                op::RETURN => {
                    let frame = self
                        .frames
                        .pop()
                        .ok_or_else(|| self.failure_at(ip, ErrorKind::StackUnderflow))?;
                    self.stack.truncate(frame.base);
                }
                _ => return Err(self.failure_at(ip, ErrorKind::BadOpcode(opcode))),
            }
        }
        Ok(())
    }

    fn host_call(
        &mut self,
        host: &mut dyn Host,
        path: &str,
        args: &[Value],
        ip: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        match path {
            "log" => {
                let [Value::Str(s)] = args else {
                    return Err(self
                        .failure_at(ip, ErrorKind::HostFailed("log takes a string".to_string())));
                };
                host.log(s);
                Ok(None)
            }
            "input.down" => {
                let [Value::I32(k)] = args else {
                    return Err(self.failure_at(
                        ip,
                        ErrorKind::HostFailed("input.down takes an i32".to_string()),
                    ));
                };
                Ok(Some(Value::Bool(host.input_down(*k))))
            }
            "transform.move" => {
                let [Value::F32(dx), Value::F32(dy)] = args else {
                    return Err(self.failure_at(
                        ip,
                        ErrorKind::HostFailed("transform.move takes f32s".to_string()),
                    ));
                };
                host.transform_move(*dx, *dy);
                Ok(None)
            }
            _ => Err(self.failure_at(ip, ErrorKind::HostFailed(format!("unknown host '{path}'")))),
        }
    }

    fn code_byte(&self, ip: usize) -> Result<u8, RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| self.failure(ErrorKind::StackUnderflow))?;
        self.chunk.functions[frame.func_idx]
            .code
            .get(ip)
            .copied()
            .ok_or_else(|| self.failure_at(ip, ErrorKind::BadOpcode(0)))
    }

    fn operand(&self, ip: usize, n: usize) -> Result<Vec<u8>, RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| self.failure(ErrorKind::StackUnderflow))?;
        let code = &self.chunk.functions[frame.func_idx].code;
        code.get(ip..ip + n)
            .map(|s| s.to_vec())
            .ok_or_else(|| self.failure_at(ip, ErrorKind::BadOpcode(0)))
    }

    fn u16_at(&self, ip: usize) -> Result<u16, RuntimeError> {
        let b = self.operand(ip, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32_at(&self, ip: usize) -> Result<u32, RuntimeError> {
        let b = self.operand(ip, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32_at(&self, ip: usize) -> Result<i32, RuntimeError> {
        let b = self.operand(ip, 4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32_at(&self, ip: usize) -> Result<f32, RuntimeError> {
        let b = self.operand(ip, 4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn push(&mut self, v: Value, ip: usize) -> Result<(), RuntimeError> {
        if self.stack.len() + 1 > crate::verifier::MAX_STACK_DEPTH {
            return Err(self.failure_at(ip, ErrorKind::StackUnderflow));
        }
        self.stack.push(v);
        Ok(())
    }

    fn pop(&mut self, ip: usize) -> Result<Value, RuntimeError> {
        let base = self.frames.last().map(|fr| fr.base).unwrap_or(0);
        if self.stack.len() <= base {
            return Err(self.failure_at(ip, ErrorKind::StackUnderflow));
        }
        Ok(self.stack.pop().unwrap_or(Value::Bool(false)))
    }

    fn pop_i32(&mut self, ip: usize) -> Result<i32, RuntimeError> {
        match self.pop(ip)? {
            Value::I32(v) => Ok(v),
            got => Err(self.failure_at(
                ip,
                ErrorKind::TypeMismatch {
                    want: Ty::I32,
                    got: got.ty(),
                },
            )),
        }
    }

    fn pop_f32(&mut self, ip: usize) -> Result<f32, RuntimeError> {
        match self.pop(ip)? {
            Value::F32(v) => Ok(v),
            got => Err(self.failure_at(
                ip,
                ErrorKind::TypeMismatch {
                    want: Ty::F32,
                    got: got.ty(),
                },
            )),
        }
    }

    fn pop_bool(&mut self, ip: usize) -> Result<bool, RuntimeError> {
        match self.pop(ip)? {
            Value::Bool(v) => Ok(v),
            got => Err(self.failure_at(
                ip,
                ErrorKind::TypeMismatch {
                    want: Ty::Bool,
                    got: got.ty(),
                },
            )),
        }
    }

    fn pop2_i32(&mut self, ip: usize) -> Result<(i32, i32), RuntimeError> {
        let b = self.pop_i32(ip)?;
        let a = self.pop_i32(ip)?;
        Ok((a, b))
    }

    fn pop2_bool(&mut self, ip: usize) -> Result<(bool, bool), RuntimeError> {
        let b = self.pop_bool(ip)?;
        let a = self.pop_bool(ip)?;
        Ok((a, b))
    }

    fn pop2_str(&mut self, ip: usize) -> Result<(String, String), RuntimeError> {
        let b = match self.pop(ip)? {
            Value::Str(s) => s,
            got => {
                return Err(self.failure_at(
                    ip,
                    ErrorKind::TypeMismatch {
                        want: Ty::Str,
                        got: got.ty(),
                    },
                ));
            }
        };
        let a = match self.pop(ip)? {
            Value::Str(s) => s,
            got => {
                return Err(self.failure_at(
                    ip,
                    ErrorKind::TypeMismatch {
                        want: Ty::Str,
                        got: got.ty(),
                    },
                ));
            }
        };
        Ok((a, b))
    }

    fn binary_i(&mut self, ip: usize, f: fn(i32, i32) -> i32) -> Result<(), RuntimeError> {
        let (a, b) = self.pop2_i32(ip)?;
        self.push(Value::I32(f(a, b)), ip)?;
        self.advance(1)
    }

    fn binary_f(&mut self, ip: usize, f: fn(f32, f32) -> f32) -> Result<(), RuntimeError> {
        let b = self.pop_f32(ip)?;
        let a = self.pop_f32(ip)?;
        self.push(Value::F32(f(a, b)), ip)?;
        self.advance(1)
    }

    fn cmp_i(&mut self, ip: usize, f: fn(i32, i32) -> bool) -> Result<(), RuntimeError> {
        let (a, b) = self.pop2_i32(ip)?;
        self.push(Value::Bool(f(a, b)), ip)?;
        self.advance(1)
    }

    fn cmp_f(&mut self, ip: usize, f: fn(f32, f32) -> bool) -> Result<(), RuntimeError> {
        let b = self.pop_f32(ip)?;
        let a = self.pop_f32(ip)?;
        self.push(Value::Bool(f(a, b)), ip)?;
        self.advance(1)
    }

    fn cmp_b(&mut self, ip: usize, f: fn(bool, bool) -> bool) -> Result<(), RuntimeError> {
        let (a, b) = self.pop2_bool(ip)?;
        self.push(Value::Bool(f(a, b)), ip)?;
        self.advance(1)
    }

    fn local_get(&self, slot: usize, ip: usize) -> Result<Value, RuntimeError> {
        self.frames
            .last()
            .and_then(|fr| fr.locals.get(slot))
            .cloned()
            .ok_or_else(|| self.failure_at(ip, ErrorKind::BadIndex("local".to_string())))
    }

    fn local_set(&mut self, slot: usize, v: Value, ip: usize) -> Result<(), RuntimeError> {
        let fi = self
            .frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| self.failure(ErrorKind::StackUnderflow))?;
        let want = self
            .frames
            .get(fi)
            .and_then(|fr| fr.locals.get(slot))
            .map(|d| d.ty())
            .ok_or_else(|| self.failure_at(ip, ErrorKind::BadIndex("local".to_string())))?;
        if v.ty() != want {
            return Err(self.failure_at(ip, ErrorKind::TypeMismatch { want, got: v.ty() }));
        }
        self.frames[fi].locals[slot] = v;
        Ok(())
    }

    fn advance(&mut self, n: usize) -> Result<(), RuntimeError> {
        let fi = self
            .frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| self.failure(ErrorKind::StackUnderflow))?;
        self.frames[fi].ip += n;
        Ok(())
    }

    fn jump(&mut self, target: usize, ip: usize) -> Result<(), RuntimeError> {
        let len = self
            .frames
            .last()
            .map(|fr| self.chunk.functions[fr.func_idx].code.len())
            .unwrap_or(0);
        if target >= len {
            return Err(self.failure_at(ip, ErrorKind::BadIndex("jump target".to_string())));
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.ip = target;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{Function, GlobalMeta, SpanEntry};

    struct TestHost {
        keys: Vec<i32>,
        moves: Vec<(f32, f32)>,
        logs: Vec<String>,
    }

    impl TestHost {
        fn new() -> Self {
            Self {
                keys: Vec::new(),
                moves: Vec::new(),
                logs: Vec::new(),
            }
        }
    }

    impl Host for TestHost {
        fn input_down(&mut self, key: i32) -> bool {
            self.keys.contains(&key)
        }

        fn transform_move(&mut self, dx: f32, dy: f32) {
            self.moves.push((dx, dy));
        }

        fn log(&mut self, msg: &str) {
            self.logs.push(msg.to_string());
        }
    }

    struct Asm {
        code: Vec<u8>,
    }

    impl Asm {
        fn new() -> Self {
            Self { code: Vec::new() }
        }

        fn op(&mut self, o: u8) -> &mut Self {
            self.code.push(o);
            self
        }

        fn u16(&mut self, v: u16) -> &mut Self {
            self.code.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn u32(&mut self, v: u32) -> &mut Self {
            self.code.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn i32(&mut self, v: i32) -> &mut Self {
            self.code.extend_from_slice(&v.to_le_bytes());
            self
        }
    }

    fn func(name: &str, params: u16, locals: Vec<Ty>, code: Vec<u8>) -> Function {
        Function {
            name: name.to_string(),
            params,
            locals,
            code,
            spans: vec![],
        }
    }

    fn chunk_of(functions: Vec<Function>, globals: Vec<(&str, Ty)>, strings: Vec<&str>) -> Chunk {
        Chunk {
            source_file: "t.em".to_string(),
            strings: strings.into_iter().map(|s| s.to_string()).collect(),
            globals: globals
                .into_iter()
                .map(|(name, ty)| GlobalMeta {
                    name: name.to_string(),
                    ty,
                })
                .collect(),
            functions,
        }
    }

    fn run(chunk: Chunk, name: &str, args: Vec<Value>) -> (TestHost, Vm) {
        let mut vm = Vm::new(chunk).unwrap();
        let mut host = TestHost::new();
        vm.call(&mut host, name, args).unwrap();
        (host, vm)
    }

    #[test]
    fn arith_store() {
        let mut a = Asm::new();
        a.op(op::CONST_I32).i32(6);
        a.op(op::CONST_I32).i32(7);
        a.op(op::MUL_I);
        a.op(op::STORE_GLOBAL).u16(0);
        a.op(op::RETURN);
        let (_, vm) = run(
            chunk_of(
                vec![func("main", 0, vec![], a.code)],
                vec![("n", Ty::I32)],
                vec![],
            ),
            "main",
            vec![],
        );
        assert_eq!(vm.globals(), &[Value::I32(42)]);
    }

    #[test]
    fn branch_true_path() {
        let code = vec![
            op::CONST_TRUE,
            op::JUMP_IF_FALSE,
            16,
            0,
            0,
            0,
            op::CONST_I32,
            1,
            0,
            0,
            0,
            op::JUMP,
            21,
            0,
            0,
            0,
            op::CONST_I32,
            2,
            0,
            0,
            0,
            op::STORE_GLOBAL,
            0,
            0,
            op::RETURN,
        ];
        let (_, vm) = run(
            chunk_of(
                vec![func("main", 0, vec![], code)],
                vec![("g", Ty::I32)],
                vec![],
            ),
            "main",
            vec![],
        );
        assert_eq!(vm.globals(), &[Value::I32(1)]);
    }

    #[test]
    fn branch_false_path() {
        let code = vec![
            op::CONST_FALSE,
            op::JUMP_IF_FALSE,
            16,
            0,
            0,
            0,
            op::CONST_I32,
            1,
            0,
            0,
            0,
            op::JUMP,
            21,
            0,
            0,
            0,
            op::CONST_I32,
            2,
            0,
            0,
            0,
            op::STORE_GLOBAL,
            0,
            0,
            op::RETURN,
        ];
        let (_, vm) = run(
            chunk_of(
                vec![func("main", 0, vec![], code)],
                vec![("g", Ty::I32)],
                vec![],
            ),
            "main",
            vec![],
        );
        assert_eq!(vm.globals(), &[Value::I32(2)]);
    }

    #[test]
    fn while_sum() {
        let mut a = Asm::new();
        a.op(op::CONST_I32).i32(0);
        a.op(op::STORE_LOCAL).u16(0);
        a.op(op::LOAD_LOCAL).u16(0);
        a.op(op::CONST_I32).i32(5);
        a.op(op::LT_I);
        a.op(op::JUMP_IF_FALSE).u32(55);
        a.op(op::LOAD_GLOBAL).u16(0);
        a.op(op::LOAD_LOCAL).u16(0);
        a.op(op::CONST_I32).i32(2);
        a.op(op::MUL_I);
        a.op(op::ADD_I);
        a.op(op::STORE_GLOBAL).u16(0);
        a.op(op::LOAD_LOCAL).u16(0);
        a.op(op::CONST_I32).i32(1);
        a.op(op::ADD_I);
        a.op(op::STORE_LOCAL).u16(0);
        a.op(op::JUMP).u32(8);
        a.op(op::RETURN);
        let (_, vm) = run(
            chunk_of(
                vec![func("main", 0, vec![Ty::I32], a.code)],
                vec![("n", Ty::I32)],
                vec![],
            ),
            "main",
            vec![],
        );
        assert_eq!(vm.globals(), &[Value::I32(20)]);
    }

    #[test]
    fn void_calls_twice() {
        let mut inc = Asm::new();
        inc.op(op::LOAD_GLOBAL).u16(0);
        inc.op(op::CONST_I32).i32(1);
        inc.op(op::ADD_I);
        inc.op(op::STORE_GLOBAL).u16(0);
        inc.op(op::RETURN);
        let mut main = Asm::new();
        main.op(op::CALL_FUNC).u16(1);
        main.op(op::CALL_FUNC).u16(1);
        main.op(op::RETURN);
        let (_, vm) = run(
            chunk_of(
                vec![
                    func("main", 0, vec![], main.code),
                    func("inc", 0, vec![], inc.code),
                ],
                vec![("n", Ty::I32)],
                vec![],
            ),
            "main",
            vec![],
        );
        assert_eq!(vm.globals(), &[Value::I32(2)]);
    }

    #[test]
    fn host_log() {
        let mut a = Asm::new();
        a.op(op::CONST_STR).u16(0);
        a.op(op::CALL_HOST).u16(0);
        a.op(op::RETURN);
        let (host, _) = run(
            chunk_of(vec![func("main", 0, vec![], a.code)], vec![], vec!["hi"]),
            "main",
            vec![],
        );
        assert_eq!(host.logs, vec!["hi".to_string()]);
    }

    #[test]
    fn host_input_down() {
        let mut a = Asm::new();
        a.op(op::CONST_I32).i32(3);
        a.op(op::CALL_HOST).u16(1);
        a.op(op::STORE_GLOBAL).u16(0);
        a.op(op::RETURN);
        let chunk = || {
            chunk_of(
                vec![func("main", 0, vec![], a.code.clone())],
                vec![("f", Ty::Bool)],
                vec![],
            )
        };
        let mut vm = Vm::new(chunk()).unwrap();
        let mut host = TestHost::new();
        host.keys.push(3);
        vm.call(&mut host, "main", vec![]).unwrap();
        assert_eq!(vm.globals(), &[Value::Bool(true)]);
        let mut vm = Vm::new(chunk()).unwrap();
        let mut host = TestHost::new();
        vm.call(&mut host, "main", vec![]).unwrap();
        assert_eq!(vm.globals(), &[Value::Bool(false)]);
    }

    #[test]
    fn string_eq() {
        let mut a = Asm::new();
        a.op(op::CONST_STR).u16(0);
        a.op(op::CONST_STR).u16(0);
        a.op(op::EQ_S);
        a.op(op::STORE_GLOBAL).u16(0);
        a.op(op::RETURN);
        let (_, vm) = run(
            chunk_of(
                vec![func("main", 0, vec![], a.code)],
                vec![("ok", Ty::Bool)],
                vec!["x"],
            ),
            "main",
            vec![],
        );
        assert_eq!(vm.globals(), &[Value::Bool(true)]);
    }

    #[test]
    fn div_by_zero_errors_with_location() {
        let mut a = Asm::new();
        a.op(op::CONST_I32).i32(1);
        a.op(op::CONST_I32).i32(0);
        a.op(op::DIV_I);
        a.op(op::STORE_LOCAL).u16(0);
        a.op(op::RETURN);
        let mut chunk = chunk_of(vec![func("f", 0, vec![Ty::I32], a.code)], vec![], vec![]);
        chunk.functions[0].spans.push(SpanEntry {
            offset: 10,
            line: 2,
            col: 1,
        });
        let mut vm = Vm::new(chunk).unwrap();
        let mut host = TestHost::new();
        let err = vm.call(&mut host, "f", vec![]).unwrap_err();
        assert_eq!(err.kind, ErrorKind::DivisionByZero);
        assert_eq!((err.line, err.col), (2, 1));
        assert_eq!(err.stack, vec!["f".to_string()]);
        assert!(err.to_string().contains("t.em:2:1:"));
    }

    #[test]
    fn budget_stops_infinite_loop() {
        let mut a = Asm::new();
        a.op(op::JUMP).u32(0);
        a.op(op::RETURN);
        let mut vm = Vm::new(chunk_of(vec![func("f", 0, vec![], a.code)], vec![], vec![])).unwrap();
        vm.set_budget(50);
        let mut host = TestHost::new();
        let err = vm.call(&mut host, "f", vec![]).unwrap_err();
        assert_eq!(err.kind, ErrorKind::BudgetExhausted);
    }

    #[test]
    fn tampered_chunk_rejected() {
        let mut a = Asm::new();
        a.op(op::RETURN);
        let mut chunk = chunk_of(vec![func("f", 0, vec![], a.code)], vec![], vec![]);
        chunk.functions[0].code[0] = 0xFF;
        assert!(Vm::new(chunk).is_err());
    }

    #[test]
    fn unknown_fn_and_arity_error() {
        let mut a = Asm::new();
        a.op(op::RETURN);
        let chunk = chunk_of(vec![func("f", 0, vec![], a.code)], vec![], vec![]);
        let mut vm = Vm::new(chunk).unwrap();
        let mut host = TestHost::new();
        assert!(matches!(
            vm.call(&mut host, "nope", vec![]).unwrap_err().kind,
            ErrorKind::UnknownFunction(_)
        ));
        assert!(matches!(
            vm.call(&mut host, "f", vec![Value::I32(1)])
                .unwrap_err()
                .kind,
            ErrorKind::ArityMismatch { .. }
        ));
    }

    #[test]
    fn recursion_hits_depth_limit() {
        let mut a = Asm::new();
        a.op(op::CALL_FUNC).u16(0);
        a.op(op::RETURN);
        let mut vm = Vm::new(chunk_of(vec![func("f", 0, vec![], a.code)], vec![], vec![])).unwrap();
        let mut host = TestHost::new();
        let err = vm.call(&mut host, "f", vec![]).unwrap_err();
        assert_eq!(err.kind, ErrorKind::CallDepthExceeded);
    }

    #[test]
    fn nested_error_stack() {
        let mut inner = Asm::new();
        inner.op(op::CONST_I32).i32(1);
        inner.op(op::CONST_I32).i32(0);
        inner.op(op::DIV_I);
        inner.op(op::STORE_LOCAL).u16(0);
        inner.op(op::RETURN);
        let mut outer = Asm::new();
        outer.op(op::CALL_FUNC).u16(1);
        outer.op(op::RETURN);
        let mut chunk = chunk_of(
            vec![
                func("outer", 0, vec![], outer.code),
                func("inner", 0, vec![Ty::I32], inner.code),
            ],
            vec![],
            vec![],
        );
        chunk.functions[1].spans.push(SpanEntry {
            offset: 10,
            line: 3,
            col: 1,
        });
        let mut vm = Vm::new(chunk).unwrap();
        let mut host = TestHost::new();
        let err = vm.call(&mut host, "outer", vec![]).unwrap_err();
        assert_eq!(err.kind, ErrorKind::DivisionByZero);
        assert_eq!(err.stack, vec!["outer".to_string(), "inner".to_string()]);
    }

    #[test]
    fn globals_default() {
        let mut a = Asm::new();
        a.op(op::RETURN);
        let vm = Vm::new(chunk_of(
            vec![func("f", 0, vec![], a.code)],
            vec![
                ("a", Ty::I32),
                ("b", Ty::F32),
                ("c", Ty::Bool),
                ("d", Ty::Str),
            ],
            vec![],
        ))
        .unwrap();
        assert_eq!(
            vm.globals(),
            &[
                Value::I32(0),
                Value::F32(0.0),
                Value::Bool(false),
                Value::Str(String::new())
            ]
        );
    }
}
