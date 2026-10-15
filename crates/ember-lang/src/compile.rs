use std::collections::HashMap;

use ember_core::{Ty, host};
use ember_vm::format::{Chunk, Function as VmFunction, GlobalMeta, SpanEntry};
use ember_vm::op;

use crate::ast::*;
use crate::{Error, Span, line_col, semantic};

pub const INIT_FN: &str = "$init";

pub fn compile(file: &str, src: &str) -> Result<Chunk, Vec<Error>> {
    let checked = semantic::check(file, src)?;
    let mut c = Cx::new(file, src);
    c.program(&checked.program)?;
    Ok(c.finish())
}

struct Cx<'a> {
    file: &'a str,
    src: &'a str,
    strings: Vec<String>,
    string_ids: HashMap<String, u16>,
    globals: Vec<(String, Ty)>,
    func_ids: HashMap<String, usize>,
    functions: Vec<FnBuild>,
}

struct FnBuild {
    name: String,
    params: u16,
    locals: Vec<Ty>,
    code: Vec<u8>,
    spans: Vec<SpanEntry>,
}

struct FnCx {
    build: FnBuild,
    scopes: Vec<HashMap<String, u16>>,
}

impl<'a> Cx<'a> {
    fn new(file: &'a str, src: &'a str) -> Self {
        Self {
            file,
            src,
            strings: Vec::new(),
            string_ids: HashMap::new(),
            globals: Vec::new(),
            func_ids: HashMap::new(),
            functions: Vec::new(),
        }
    }

    fn internal(&self, span: Span, what: &str) -> Error {
        Error::new(
            self.file,
            self.src,
            span.start as usize,
            span,
            format!("internal compiler error: {what}"),
        )
    }

    fn ty_of(&self, name: &str) -> Result<Ty, Error> {
        Ty::from_name(name).ok_or_else(|| {
            Error::new(
                self.file,
                self.src,
                0,
                Span::empty(0),
                format!("unknown type '{name}'"),
            )
        })
    }

    fn program(&mut self, program: &Program) -> Result<(), Vec<Error>> {
        let mut errors = Vec::new();
        for item in &program.items {
            match item {
                Item::GlobalVar(g) => match self.ty_of(&g.ty.name) {
                    Ok(t) => self.globals.push((g.name.clone(), t)),
                    Err(e) => errors.push(e),
                },
                Item::Function(f) => {
                    let mut bad = false;
                    for p in &f.params {
                        if self.ty_of(&p.ty.name).is_err() {
                            bad = true;
                        }
                    }
                    if !bad {
                        self.func_ids.insert(f.name.clone(), self.func_ids.len());
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let init_id = self.func_ids.len();
        self.func_ids.insert(INIT_FN.to_string(), init_id);
        let mut builds: Vec<(usize, FnBuild)> = Vec::new();
        builds.push((init_id, self.init_fn(program)?));
        for item in &program.items {
            if let Item::Function(f) = item {
                let id = self
                    .func_ids
                    .get(&f.name)
                    .copied()
                    .ok_or_else(|| vec![self.internal(f.span, "missing function id")])?;
                builds.push((id, self.function(f)?));
            }
        }
        builds.sort_by_key(|(id, _)| *id);
        self.functions = builds.into_iter().map(|(_, b)| b).collect();
        Ok(())
    }

    fn finish(self) -> Chunk {
        Chunk {
            source_file: self.file.to_string(),
            strings: self.strings,
            globals: self
                .globals
                .into_iter()
                .map(|(name, ty)| GlobalMeta { name, ty })
                .collect(),
            functions: self
                .functions
                .into_iter()
                .map(|b| VmFunction {
                    name: b.name,
                    params: b.params,
                    locals: b.locals,
                    code: b.code,
                    spans: b.spans,
                })
                .collect(),
        }
    }

    fn init_fn(&mut self, program: &Program) -> Result<FnBuild, Vec<Error>> {
        let mut f = FnCx::new(INIT_FN);
        let mut errors = Vec::new();
        for item in &program.items {
            if let Item::GlobalVar(g) = item {
                let idx = self.global_idx(&g.name)?;
                if let Err(e) = self.expr(&g.init, &mut f) {
                    errors.push(e);
                    continue;
                }
                self.emit_u16(&mut f, op::STORE_GLOBAL, idx, g.span);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        self.emit(&mut f, op::RETURN, Span::empty(self.src.len()));
        Ok(f.build)
    }

    fn global_idx(&self, name: &str) -> Result<u16, Vec<Error>> {
        self.globals
            .iter()
            .position(|(n, _)| n == name)
            .and_then(|i| u16::try_from(i).ok())
            .ok_or_else(|| {
                vec![Error::new(
                    self.file,
                    self.src,
                    0,
                    Span::empty(0),
                    format!("unknown global '{name}'"),
                )]
            })
    }

    fn function(&mut self, f: &Function) -> Result<FnBuild, Vec<Error>> {
        let mut fx = FnCx::new(&f.name);
        fx.build.params = u16::try_from(f.params.len())
            .map_err(|_| vec![self.internal(f.span, "too many parameters")])?;
        fx.scopes.push(HashMap::new());
        let mut errors = Vec::new();
        for p in &f.params {
            match self.ty_of(&p.ty.name) {
                Ok(t) => {
                    fx.declare(p.name.clone(), t);
                }
                Err(e) => errors.push(e),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        fx.scopes.push(HashMap::new());
        for s in &f.body.stmts {
            if let Err(e) = self.stmt(s, &mut fx) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let end = Span::empty(self.src.len());
        self.emit(&mut fx, op::RETURN, end);
        Ok(fx.build)
    }

    fn stmt(&mut self, s: &Stmt, f: &mut FnCx) -> Result<(), Error> {
        match s {
            Stmt::Var(v) => {
                let ty = self
                    .ty_of(&v.ty.name)
                    .map_err(|_| self.internal(v.span, "bad type"))?;
                self.expr(&v.init, f)?;
                let slot = f.declare(v.name.clone(), ty);
                self.emit_u16(f, op::STORE_LOCAL, slot, v.span);
                Ok(())
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                self.expr(value, f)?;
                if target.parts.len() > 1 {
                    return Err(self.internal(*span, "host assignment"));
                }
                let name = &target.parts[0];
                if let Some(slot) = f.lookup(name) {
                    self.emit_u16(f, op::STORE_LOCAL, slot, *span);
                    Ok(())
                } else if let Some(idx) = self.global_pos(name) {
                    self.emit_u16(f, op::STORE_GLOBAL, idx, *span);
                    Ok(())
                } else {
                    Err(self.internal(*span, "unknown assign target"))
                }
            }
            Stmt::If {
                cond, then, els, ..
            } => {
                self.expr(cond, f)?;
                let to_else = self.emit_jump(f, op::JUMP_IF_FALSE, expr_span(cond));
                self.block(then, f)?;
                match els {
                    Some(Else::Block(b)) => {
                        let to_end = self.emit_jump(f, op::JUMP, expr_span(cond));
                        self.patch(f, to_else);
                        self.block(b, f)?;
                        self.patch(f, to_end);
                    }
                    Some(Else::If(nested)) => {
                        let to_end = self.emit_jump(f, op::JUMP, expr_span(cond));
                        self.patch(f, to_else);
                        self.stmt(nested, f)?;
                        self.patch(f, to_end);
                    }
                    None => self.patch(f, to_else),
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                let loop_at = f.build.code.len() as u32;
                self.expr(cond, f)?;
                let to_end = self.emit_jump(f, op::JUMP_IF_FALSE, expr_span(cond));
                self.block(body, f)?;
                let pos = self.emit_jump(f, op::JUMP, expr_span(cond));
                self.patch_at(f, pos, loop_at);
                self.patch(f, to_end);
                Ok(())
            }
            Stmt::Return { value: None, span } => {
                self.emit(f, op::RETURN, *span);
                Ok(())
            }
            Stmt::Return {
                value: Some(_),
                span,
            } => Err(self.internal(*span, "valued return")),
            Stmt::Expr(e) => {
                self.expr(e, f)?;
                Ok(())
            }
        }
    }

    fn block(&mut self, b: &Block, f: &mut FnCx) -> Result<(), Error> {
        f.scopes.push(HashMap::new());
        for s in &b.stmts {
            self.stmt(s, f)?;
        }
        f.scopes.pop();
        Ok(())
    }

    fn expr(&mut self, e: &Expr, f: &mut FnCx) -> Result<(), Error> {
        let span = expr_span(e);
        match e {
            Expr::Int(v, _) => self.emit_i32(f, *v, span),
            Expr::Float(v, _) => self.emit_f32(f, *v, span),
            Expr::Str(s, _) => {
                let idx = self.intern(s);
                self.emit_u16(f, op::CONST_STR, idx, span);
            }
            Expr::Bool(true, _) => self.emit(f, op::CONST_TRUE, span),
            Expr::Bool(false, _) => self.emit(f, op::CONST_FALSE, span),
            Expr::Path(p) => self.load_path(p, f)?,
            Expr::Call { func, args, .. } => self.call(func, args, f)?,
            Expr::Unary {
                op: UnOp::Neg, rhs, ..
            } => {
                let t = self.expr_ty(rhs, f)?;
                self.expr(rhs, f)?;
                self.emit(f, if t == Ty::F32 { op::NEG_F } else { op::NEG_I }, span);
            }
            Expr::Unary {
                op: UnOp::Not, rhs, ..
            } => {
                self.expr(rhs, f)?;
                self.emit(f, op::NOT, span);
            }
            Expr::Binary {
                op: bop, lhs, rhs, ..
            } => {
                let t = self.expr_ty(lhs, f)?;
                self.expr(lhs, f)?;
                self.expr(rhs, f)?;
                self.emit(f, bin_op(*bop, t), span);
            }
        }
        Ok(())
    }

    fn load_path(&mut self, p: &Path, f: &mut FnCx) -> Result<(), Error> {
        if p.parts.len() > 1 {
            if host::host_const(&p.parts.join(".")).is_some() {
                let v = host::host_const(&p.parts.join("."))
                    .map(|c| c.value)
                    .unwrap_or(0);
                self.emit_i32(f, v as i64, p.span);
                return Ok(());
            }
            return Err(self.internal(p.span, "bad host path"));
        }
        let name = &p.parts[0];
        if let Some(slot) = f.lookup(name) {
            self.emit_u16(f, op::LOAD_LOCAL, slot, p.span);
            Ok(())
        } else if let Some(idx) = self.global_pos(name) {
            self.emit_u16(f, op::LOAD_GLOBAL, idx, p.span);
            Ok(())
        } else {
            Err(self.internal(p.span, "unknown name"))
        }
    }

    fn call(&mut self, func: &Path, args: &[Expr], f: &mut FnCx) -> Result<(), Error> {
        for a in args {
            self.expr(a, f)?;
        }
        if func.parts.len() > 1 {
            let dotted = func.parts.join(".");
            let idx = host::HOST_FUNCTIONS
                .iter()
                .position(|h| h.path == dotted)
                .and_then(|i| u16::try_from(i).ok())
                .ok_or_else(|| self.internal(func.span, "unknown host fn"))?;
            self.emit_u16(f, op::CALL_HOST, idx, func.span);
            return Ok(());
        }
        let name = &func.parts[0];
        if let Some(id) = self.func_ids.get(name).copied() {
            let idx =
                u16::try_from(id).map_err(|_| self.internal(func.span, "too many functions"))?;
            self.emit_u16(f, op::CALL_FUNC, idx, func.span);
            Ok(())
        } else if host::host_fn(name).is_some() {
            let idx = host::HOST_FUNCTIONS
                .iter()
                .position(|h| h.path == name)
                .and_then(|i| u16::try_from(i).ok())
                .ok_or_else(|| self.internal(func.span, "unknown host fn"))?;
            self.emit_u16(f, op::CALL_HOST, idx, func.span);
            Ok(())
        } else {
            Err(self.internal(func.span, "unknown call target"))
        }
    }

    fn expr_ty(&self, e: &Expr, f: &FnCx) -> Result<Ty, Error> {
        match e {
            Expr::Int(..) => Ok(Ty::I32),
            Expr::Float(..) => Ok(Ty::F32),
            Expr::Str(..) => Ok(Ty::Str),
            Expr::Bool(..) => Ok(Ty::Bool),
            Expr::Path(p) => {
                if p.parts.len() > 1 {
                    return Ok(Ty::I32);
                }
                let name = &p.parts[0];
                if let Some(slot) = f.lookup(name) {
                    return f
                        .build
                        .locals
                        .get(slot as usize)
                        .copied()
                        .ok_or_else(|| self.internal(p.span, "bad slot"));
                }
                self.globals
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, t)| *t)
                    .ok_or_else(|| self.internal(p.span, "unknown name"))
            }
            Expr::Call { func, .. } => {
                if func.parts.len() > 1 {
                    let dotted = func.parts.join(".");
                    host::host_fn(&dotted)
                        .map(|h| h.ret)
                        .ok_or_else(|| self.internal(func.span, "unknown host fn"))
                } else if self.func_ids.contains_key(&func.parts[0]) {
                    Ok(Ty::Void)
                } else {
                    host::host_fn(&func.parts[0])
                        .map(|h| h.ret)
                        .ok_or_else(|| self.internal(func.span, "unknown call"))
                }
            }
            Expr::Unary {
                op: UnOp::Neg, rhs, ..
            } => self.expr_ty(rhs, f),
            Expr::Unary { op: UnOp::Not, .. } => Ok(Ty::Bool),
            Expr::Binary {
                op: BinOp::And | BinOp::Or,
                ..
            } => Ok(Ty::Bool),
            Expr::Binary {
                op: BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge,
                lhs,
                ..
            } => {
                let _ = self.expr_ty(lhs, f)?;
                Ok(Ty::Bool)
            }
            Expr::Binary { lhs, .. } => self.expr_ty(lhs, f),
        }
    }

    fn global_pos(&self, name: &str) -> Option<u16> {
        self.globals
            .iter()
            .position(|(n, _)| n == name)
            .and_then(|i| u16::try_from(i).ok())
    }

    fn intern(&mut self, s: &str) -> u16 {
        if let Some(id) = self.string_ids.get(s) {
            return *id;
        }
        let id = self.strings.len() as u16;
        self.strings.push(s.to_string());
        self.string_ids.insert(s.to_string(), id);
        id
    }

    fn mark(&mut self, f: &mut FnCx, span: Span) {
        let (line, col) = line_col(self.src, span.start as usize);
        f.build.spans.push(SpanEntry {
            offset: f.build.code.len() as u32,
            line: line as u32,
            col: col as u32,
        });
    }

    fn emit(&mut self, f: &mut FnCx, opcode: u8, span: Span) {
        self.mark(f, span);
        f.build.code.push(opcode);
    }

    fn emit_u16(&mut self, f: &mut FnCx, opcode: u8, v: u16, span: Span) {
        self.mark(f, span);
        f.build.code.push(opcode);
        f.build.code.extend_from_slice(&v.to_le_bytes());
    }

    fn emit_i32(&mut self, f: &mut FnCx, v: i64, span: Span) {
        self.mark(f, span);
        f.build.code.push(op::CONST_I32);
        f.build.code.extend_from_slice(&(v as i32).to_le_bytes());
    }

    fn emit_f32(&mut self, f: &mut FnCx, v: f64, span: Span) {
        self.mark(f, span);
        f.build.code.push(op::CONST_F32);
        f.build.code.extend_from_slice(&(v as f32).to_le_bytes());
    }

    fn emit_jump(&mut self, f: &mut FnCx, opcode: u8, span: Span) -> usize {
        self.mark(f, span);
        f.build.code.push(opcode);
        let pos = f.build.code.len();
        f.build.code.extend_from_slice(&0u32.to_le_bytes());
        pos
    }

    fn patch(&mut self, f: &mut FnCx, pos: usize) {
        let target = f.build.code.len() as u32;
        self.patch_at(f, pos, target);
    }

    fn patch_at(&mut self, f: &mut FnCx, pos: usize, target: u32) {
        f.build.code[pos..pos + 4].copy_from_slice(&target.to_le_bytes());
    }
}

fn fx_alloc(build: &mut FnBuild, ty: Ty) -> u16 {
    build.locals.push(ty);
    (build.locals.len() - 1) as u16
}

impl FnCx {
    fn new(name: &str) -> Self {
        Self {
            build: FnBuild {
                name: name.to_string(),
                params: 0,
                locals: Vec::new(),
                code: Vec::new(),
                spans: Vec::new(),
            },
            scopes: Vec::new(),
        }
    }

    fn declare(&mut self, name: String, ty: Ty) -> u16 {
        let slot = fx_alloc(&mut self.build, ty);
        if let Some(frame) = self.scopes.last_mut() {
            frame.insert(name, slot);
        }
        slot
    }

    fn lookup(&self, name: &str) -> Option<u16> {
        for frame in self.scopes.iter().rev() {
            if let Some(slot) = frame.get(name) {
                return Some(*slot);
            }
        }
        None
    }
}

fn bin_op(op: BinOp, t: Ty) -> u8 {
    match (op, t) {
        (BinOp::Add, Ty::I32) => op::ADD_I,
        (BinOp::Sub, Ty::I32) => op::SUB_I,
        (BinOp::Mul, Ty::I32) => op::MUL_I,
        (BinOp::Div, Ty::I32) => op::DIV_I,
        (BinOp::Mod, Ty::I32) => op::MOD_I,
        (BinOp::Add, _) => op::ADD_F,
        (BinOp::Sub, _) => op::SUB_F,
        (BinOp::Mul, _) => op::MUL_F,
        (BinOp::Div, _) => op::DIV_F,
        (BinOp::Mod, _) => op::MOD_F,
        (BinOp::Eq, Ty::I32) => op::EQ_I,
        (BinOp::Ne, Ty::I32) => op::NE_I,
        (BinOp::Lt, Ty::I32) => op::LT_I,
        (BinOp::Le, Ty::I32) => op::LE_I,
        (BinOp::Gt, Ty::I32) => op::GT_I,
        (BinOp::Ge, Ty::I32) => op::GE_I,
        (BinOp::Eq, Ty::F32) => op::EQ_F,
        (BinOp::Ne, Ty::F32) => op::NE_F,
        (BinOp::Lt, Ty::F32) => op::LT_F,
        (BinOp::Le, Ty::F32) => op::LE_F,
        (BinOp::Gt, Ty::F32) => op::GT_F,
        (BinOp::Ge, Ty::F32) => op::GE_F,
        (BinOp::Eq, Ty::Bool) => op::EQ_B,
        (BinOp::Ne, Ty::Bool) => op::NE_B,
        (BinOp::Eq, _) => op::EQ_S,
        (BinOp::Ne, _) => op::NE_S,
        (BinOp::Lt, _) => op::LT_F,
        (BinOp::Le, _) => op::LE_F,
        (BinOp::Gt, _) => op::GT_F,
        (BinOp::Ge, _) => op::GE_F,
        (BinOp::And, _) => op::AND,
        (BinOp::Or, _) => op::OR,
    }
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Int(_, s) | Expr::Float(_, s) | Expr::Str(_, s) | Expr::Bool(_, s) => *s,
        Expr::Path(p) => p.span,
        Expr::Call { span, .. } | Expr::Unary { span, .. } | Expr::Binary { span, .. } => *span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_vm::{decode, disassemble, encode, verify};

    const STARTER: &str = "var speed: f32 = 220.0\nfn on_create() {\nlog(\"hi\")\n}\nfn on_update(dt: f32) {\nif input.down(Key.Right) {\ntransform.move(speed * dt, 0.0)\n}\n}\n";

    fn chunk(src: &str) -> Chunk {
        compile("t.em", src).unwrap()
    }

    fn func<'c>(c: &'c Chunk, name: &str) -> &'c ember_vm::Function {
        c.functions.iter().find(|f| f.name == name).unwrap()
    }

    #[test]
    fn starter_compiles_and_verifies() {
        let c = chunk(STARTER);
        verify(&c).unwrap();
        let names: Vec<&str> = c.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"$init"));
        assert!(names.contains(&"on_create"));
        assert!(names.contains(&"on_update"));
        assert!(disassemble(&c).contains("call.host"));
    }

    #[test]
    fn global_init_stores() {
        let c = chunk(STARTER);
        assert_eq!(c.globals.len(), 1);
        assert!(disassemble(&c).contains("store.global"));
    }

    #[test]
    fn branches_emit_jumps() {
        let c = chunk("fn f(x: bool) {\nif x {\n} else {\n}\nwhile x {\n}\n}\n");
        verify(&c).unwrap();
        let text = disassemble(&c);
        assert!(text.contains("jump.if_false"));
        assert!(text.contains("jump\n") || text.contains("jump "));
    }

    #[test]
    fn type_error_flows() {
        assert!(compile("t.em", "var x: i32 = 1.5\n").is_err());
        assert!(compile("t.em", "fn f() {\ny = 1\n}\n").is_err());
    }

    #[test]
    fn locals_layout() {
        let c = chunk("fn f(a: i32) {\nvar x: i32 = 1\nvar y: f32 = 2.0\n}\n");
        let f = func(&c, "f");
        assert_eq!(f.params, 1);
        assert_eq!(f.locals, vec![Ty::I32, Ty::I32, Ty::F32]);
    }

    #[test]
    fn strings_pooled() {
        let c = chunk("fn f() {\nlog(\"hi\")\nlog(\"hi\")\n}\n");
        verify(&c).unwrap();
        assert_eq!(c.strings, vec!["hi".to_string()]);
    }

    #[test]
    fn shadow_compiles() {
        let c = chunk("fn f() {\nvar x: i32 = 1\nif true {\nvar x: f32 = 2.0\n}\n}\n");
        verify(&c).unwrap();
    }

    #[test]
    fn round_trip_encode_verify() {
        let c = chunk(STARTER);
        let back = decode(&encode(&c)).unwrap();
        verify(&back).unwrap();
        assert_eq!(back, c);
    }
}
