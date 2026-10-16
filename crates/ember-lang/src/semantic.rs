use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::host::{self, Ty};
use crate::{Error, Span};

#[derive(Debug)]
pub struct Checked {
    pub program: Program,
}

enum Sym {
    Var(Ty),
    Func(Vec<Ty>),
}

pub fn check(file: &str, src: &str) -> Result<Checked, Vec<Error>> {
    let program = match crate::parse(file, src) {
        Ok(p) => p,
        Err(e) => return Err(vec![e]),
    };
    let mut c = Ck {
        file,
        src,
        errors: Vec::new(),
        globals: HashMap::new(),
        poisoned: HashSet::new(),
    };
    c.collect_globals(&program);
    for item in &program.items {
        match item {
            Item::GlobalVar(g) => c.check_global(g),
            Item::Function(f) => c.check_function(f),
        }
    }
    if c.errors.is_empty() {
        Ok(Checked { program })
    } else {
        Err(c.errors)
    }
}

struct Ck<'a> {
    file: &'a str,
    src: &'a str,
    errors: Vec<Error>,
    globals: HashMap<String, Sym>,
    poisoned: HashSet<String>,
}

impl<'a> Ck<'a> {
    fn err(&mut self, span: Span, message: String) {
        self.errors.push(Error::new(
            self.file,
            self.src,
            span.start as usize,
            span,
            message,
        ));
    }

    fn value_ty(&mut self, span: Span, name: &str) -> Option<Ty> {
        match Ty::from_name(name) {
            Some(Ty::Void) => {
                self.err(span, "type 'void' cannot hold a value".to_string());
                None
            }
            Some(t) => Some(t),
            None => {
                self.err(span, format!("unknown type '{name}'"));
                None
            }
        }
    }

    fn collect_globals(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::GlobalVar(g) => {
                    if host::HOST_ROOTS.contains(&g.name.as_str()) {
                        self.err(g.span, format!("'{}' redefines a host name", g.name));
                        continue;
                    }
                    if self.globals.contains_key(&g.name) {
                        self.err(g.span, format!("'{}' is already defined", g.name));
                        continue;
                    }
                    match self.value_ty(g.ty.span, &g.ty.name) {
                        Some(t) => {
                            self.globals.insert(g.name.clone(), Sym::Var(t));
                        }
                        None => {
                            self.poisoned.insert(g.name.clone());
                        }
                    }
                }
                Item::Function(f) => {
                    if host::HOST_ROOTS.contains(&f.name.as_str()) {
                        self.err(f.span, format!("'{}' redefines a host name", f.name));
                        continue;
                    }
                    if self.globals.contains_key(&f.name) {
                        self.err(f.span, format!("'{}' is already defined", f.name));
                        continue;
                    }
                    let mut params = Vec::new();
                    let mut bad = false;
                    for p in &f.params {
                        match self.value_ty(p.ty.span, &p.ty.name) {
                            Some(t) => params.push(t),
                            None => {
                                bad = true;
                            }
                        }
                    }
                    if !bad {
                        self.globals.insert(f.name.clone(), Sym::Func(params));
                    }
                }
            }
        }
    }

    fn check_global(&mut self, g: &GlobalVar) {
        let Some(declared) = self.global_var_ty(&g.name) else {
            return;
        };
        let mut scopes = vec![HashMap::new()];
        let got = self.expr_ty(&g.init, &mut scopes);
        if got != declared {
            self.err(
                expr_span(&g.init),
                format!("expected {}, found {}", declared, got),
            );
        }
    }

    fn global_var_ty(&self, name: &str) -> Option<Ty> {
        match self.globals.get(name) {
            Some(Sym::Var(t)) => Some(*t),
            _ => None,
        }
    }

    fn check_function(&mut self, f: &Function) {
        if f.name == "on_create" && !f.params.is_empty() {
            self.err(f.span, "on_create must take no parameters".to_string());
        }
        if f.name == "on_update"
            && (f.params.len() != 1 || Ty::from_name(&f.params[0].ty.name) != Some(Ty::F32))
        {
            self.err(
                f.span,
                "on_update must take exactly one f32 parameter".to_string(),
            );
        }
        let mut scopes = vec![HashMap::new()];
        for p in &f.params {
            if let Some(t) = self.value_ty(p.ty.span, &p.ty.name) {
                if scopes[0].contains_key(&p.name) {
                    self.err(p.span, format!("'{}' is already defined", p.name));
                } else {
                    scopes[0].insert(p.name.clone(), Sym::Var(t));
                }
            }
        }
        self.block(&f.body, &mut scopes);
    }

    fn block(&mut self, b: &Block, scopes: &mut Vec<HashMap<String, Sym>>) {
        scopes.push(HashMap::new());
        for s in &b.stmts {
            self.stmt(s, scopes);
        }
        scopes.pop();
    }

    fn stmt(&mut self, s: &Stmt, scopes: &mut Vec<HashMap<String, Sym>>) {
        match s {
            Stmt::Var(v) => {
                let declared = match self.value_ty(v.ty.span, &v.ty.name) {
                    Some(t) => t,
                    None => {
                        self.expr_ty(&v.init, scopes);
                        return;
                    }
                };
                let got = self.expr_ty(&v.init, scopes);
                if got != declared {
                    self.err(
                        expr_span(&v.init),
                        format!("expected {declared}, found {got}"),
                    );
                }
                let clash = scopes.last().is_some_and(|m| m.contains_key(&v.name));
                if clash {
                    self.err(v.span, format!("'{}' is already defined", v.name));
                } else if let Some(top) = scopes.last_mut() {
                    top.insert(v.name.clone(), Sym::Var(declared));
                }
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                let vt = self.assign_target(target, scopes);
                let got = self.expr_ty(value, scopes);
                if let Some(vt) = vt
                    && vt != got
                {
                    self.err(*span, format!("expected {vt}, found {got}"));
                }
            }
            Stmt::If {
                cond,
                then,
                els,
                span,
            } => {
                let got = self.expr_ty(cond, scopes);
                if got != Ty::Bool {
                    self.err(*span, format!("condition must be bool, found {got}"));
                }
                self.block(then, scopes);
                match els {
                    Some(Else::Block(b)) => self.block(b, scopes),
                    Some(Else::If(nested)) => self.stmt(nested, scopes),
                    None => {}
                }
            }
            Stmt::While { cond, body, span } => {
                let got = self.expr_ty(cond, scopes);
                if got != Ty::Bool {
                    self.err(*span, format!("condition must be bool, found {got}"));
                }
                self.block(body, scopes);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::Return { value: Some(v), .. } => {
                self.err(
                    expr_span(v),
                    "v0.1 functions return void; 'return <expr>' is not supported".to_string(),
                );
                self.expr_ty(v, scopes);
            }
            Stmt::Expr(e) => {
                if !matches!(e, Expr::Call { .. }) {
                    self.err(
                        expr_span(e),
                        "only calls and assignments allowed as statements".to_string(),
                    );
                }
                self.expr_ty(e, scopes);
            }
        }
    }

    fn assign_target(&mut self, target: &Path, scopes: &[HashMap<String, Sym>]) -> Option<Ty> {
        if target.parts.len() > 1 {
            self.err(target.span, "cannot assign to a host path".to_string());
            return None;
        }
        let name = &target.parts[0];
        for scope in scopes.iter().rev() {
            if let Some(Sym::Var(t)) = scope.get(name) {
                return Some(*t);
            }
            if let Some(Sym::Func(_)) = scope.get(name) {
                self.err(target.span, format!("cannot assign to function '{name}'"));
                return None;
            }
        }
        match self.globals.get(name) {
            Some(Sym::Var(t)) => Some(*t),
            Some(Sym::Func(_)) => {
                self.err(target.span, format!("cannot assign to function '{name}'"));
                None
            }
            None => {
                self.undefined(name, target.span);
                None
            }
        }
    }

    fn undefined(&mut self, name: &str, span: Span) {
        if self.poisoned.insert(name.to_string()) {
            self.err(span, format!("undefined name '{name}'"));
        }
    }

    fn expr_ty(&mut self, e: &Expr, scopes: &mut Vec<HashMap<String, Sym>>) -> Ty {
        match e {
            Expr::Int(..) => Ty::I32,
            Expr::Float(..) => Ty::F32,
            Expr::Str(..) => Ty::Str,
            Expr::Bool(..) => Ty::Bool,
            Expr::Path(p) => self.path_ty(p, scopes),
            Expr::Call { func, args, span } => self.call_ty(func, args, *span, scopes),
            Expr::Unary { op, rhs, span } => {
                let t = self.expr_ty(rhs, scopes);
                let (ok, ret) = match (op, t) {
                    (UnOp::Neg, Ty::I32) => (true, Ty::I32),
                    (UnOp::Neg, Ty::F32) => (true, Ty::F32),
                    (UnOp::Not, Ty::Bool) => (true, Ty::Bool),
                    _ => (false, t),
                };
                if !ok {
                    let sym = if *op == UnOp::Neg { "-x" } else { "!x" };
                    self.err(*span, format!("operator '{sym}' not defined for {t}"));
                }
                ret
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let l = self.expr_ty(lhs, scopes);
                let r = self.expr_ty(rhs, scopes);
                self.binary_ty(*op, l, r, *span)
            }
        }
    }

    fn binary_ty(&mut self, op: BinOp, l: Ty, r: Ty, span: Span) -> Ty {
        let num = matches!(l, Ty::I32 | Ty::F32) && l == r;
        let ret = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod if num => Some(l),
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if num => Some(Ty::Bool),
            BinOp::Eq | BinOp::Ne
                if l == r && matches!(l, Ty::I32 | Ty::F32 | Ty::Bool | Ty::Str) =>
            {
                Some(Ty::Bool)
            }
            BinOp::And | BinOp::Or if l == Ty::Bool && r == Ty::Bool => Some(Ty::Bool),
            _ => None,
        };
        match ret {
            Some(t) => t,
            None => {
                self.err(span, format!("operator not defined for {l} and {r}"));
                Ty::Bool
            }
        }
    }

    fn path_ty(&mut self, p: &Path, scopes: &[HashMap<String, Sym>]) -> Ty {
        if p.parts.len() > 1 {
            let dotted = p.parts.join(".");
            if host::host_const(&dotted).is_some() {
                return Ty::I32;
            }
            self.err(p.span, format!("unknown host path '{dotted}'"));
            return Ty::I32;
        }
        let name = &p.parts[0];
        for scope in scopes.iter().rev() {
            match scope.get(name) {
                Some(Sym::Var(t)) => return *t,
                Some(Sym::Func(_)) => {
                    self.err(p.span, format!("function '{name}' used as a value"));
                    return Ty::Void;
                }
                None => {}
            }
        }
        match self.globals.get(name) {
            Some(Sym::Var(t)) => *t,
            Some(Sym::Func(_)) => {
                self.err(p.span, format!("function '{name}' used as a value"));
                Ty::Void
            }
            None => {
                self.undefined(name, p.span);
                Ty::I32
            }
        }
    }

    fn call_ty(
        &mut self,
        func: &Path,
        args: &[Expr],
        span: Span,
        scopes: &mut Vec<HashMap<String, Sym>>,
    ) -> Ty {
        let got: Vec<Ty> = args.iter().map(|a| self.expr_ty(a, scopes)).collect();
        if func.parts.len() > 1 {
            let dotted = func.parts.join(".");
            match host::host_fn(&dotted) {
                Some(h) => {
                    self.check_arity(&dotted, h.params, &got, span);
                    return h.ret;
                }
                None => {
                    self.err(func.span, format!("unknown host function '{dotted}'"));
                    return Ty::Void;
                }
            }
        }
        let name = &func.parts[0];
        for scope in scopes.iter().rev() {
            if scope.contains_key(name) {
                self.err(func.span, format!("'{name}' is not a function"));
                return Ty::Void;
            }
        }
        match self.globals.get(name) {
            Some(Sym::Func(params)) => {
                let params = params.clone();
                self.check_arity(name, &params, &got, span);
                Ty::Void
            }
            Some(Sym::Var(_)) => {
                self.err(func.span, format!("'{name}' is not a function"));
                Ty::Void
            }
            None => match host::host_fn(name) {
                Some(h) => {
                    self.check_arity(name, h.params, &got, span);
                    h.ret
                }
                None => {
                    self.undefined(name, func.span);
                    Ty::Void
                }
            },
        }
    }

    fn check_arity(&mut self, name: &str, params: &[Ty], got: &[Ty], span: Span) {
        if params.len() != got.len() {
            self.err(
                span,
                format!(
                    "'{}' takes {} argument(s), got {}",
                    name,
                    params.len(),
                    got.len()
                ),
            );
            return;
        }
        for (i, (want, have)) in params.iter().zip(got.iter()).enumerate() {
            if want != have {
                self.err(
                    span,
                    format!(
                        "argument {} of '{name}' must be {want}, found {have}",
                        i + 1
                    ),
                );
            }
        }
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

    const STARTER: &str = "var speed: f32 = 220.0\nfn on_create() {\nlog(\"hi\")\n}\nfn on_update(dt: f32) {\nif input.down(Key.Right) {\ntransform.move(speed * dt, 0.0)\n}\n}\n";

    fn ok(src: &str) {
        if let Err(errors) = check("t.em", src) {
            panic!(
                "expected ok, got: {}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
    }

    fn err(src: &str) -> Vec<String> {
        check("t.em", src)
            .unwrap_err()
            .iter()
            .map(|e| e.to_string())
            .collect()
    }

    #[test]
    fn starter_checks() {
        ok(STARTER);
    }

    #[test]
    fn var_init_mismatch() {
        let errors = err("var x: i32 = 1.5\n");
        assert!(errors[0].contains("t.em:1:"), "{errors:?}");
        assert!(errors[0].contains("expected i32, found f32"));
    }

    #[test]
    fn undefined_name() {
        let errors = err("fn f() {\nx = 1\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("undefined name 'x'")),
            "{errors:?}"
        );
    }

    #[test]
    fn unknown_type() {
        let errors = err("var x: Foo = 1\n");
        assert!(
            errors.iter().any(|e| e.contains("unknown type 'Foo'")),
            "{errors:?}"
        );
    }

    #[test]
    fn void_var_rejected() {
        let errors = err("var x: void = log(\"a\")\n");
        assert!(
            errors.iter().any(|e| e.contains("cannot hold a value")),
            "{errors:?}"
        );
    }

    #[test]
    fn void_param_rejected() {
        let errors = err("fn f(a: void) {\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("cannot hold a value")),
            "{errors:?}"
        );
    }

    #[test]
    fn mixed_arith_rejected() {
        let errors = err("fn f() {\nvar x: f32 = 1 + 1.0\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("not defined for i32 and f32")),
            "{errors:?}"
        );
    }

    #[test]
    fn non_bool_condition() {
        let errors = err("fn f() {\nif 1 {\n}\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("condition must be bool")),
            "{errors:?}"
        );
    }

    #[test]
    fn host_arg_types() {
        let errors = err("fn f() {\ntransform.move(1, 2.0)\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("must be f32")),
            "{errors:?}"
        );
    }

    #[test]
    fn unknown_host_fn() {
        let errors = err("fn f() {\ninput.nope(1)\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("unknown host function")),
            "{errors:?}"
        );
    }

    #[test]
    fn deferred_conversions_rejected() {
        for src in [
            "fn f() {\nvar x: f32 = f32.of_i(1)\n}\n",
            "fn f() {\nvar x: i32 = i32.of_f(1.0)\n}\n",
            "fn f() {\nlog(str.of_f(1.0))\n}\n",
        ] {
            let errors = err(src);
            assert!(
                errors.iter().any(|e| e.contains("unknown host function")),
                "{src:?}: {errors:?}"
            );
        }
    }

    #[test]
    fn unknown_host_const() {
        let errors = err("fn f() {\nvar x: i32 = Key.Nope\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("unknown host path")),
            "{errors:?}"
        );
    }

    #[test]
    fn return_value_rejected() {
        let errors = err("fn f() {\nreturn 1\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("return void")),
            "{errors:?}"
        );
    }

    #[test]
    fn void_call_in_expr() {
        let errors = err("fn g() {\n}\nfn f() {\nvar x: i32 = g()\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("expected i32, found void")),
            "{errors:?}"
        );
    }

    #[test]
    fn assign_to_host_path() {
        let errors = err("fn f() {\ninput.down = true\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("cannot assign to a host path")),
            "{errors:?}"
        );
    }

    #[test]
    fn duplicate_local() {
        let errors = err("fn f() {\nvar x: i32 = 1\nvar x: i32 = 2\n}\n");
        assert!(
            errors.iter().any(|e| e.contains("already defined")),
            "{errors:?}"
        );
    }

    #[test]
    fn arity_checked() {
        let errors = err("fn f() {\nlog(\"a\", \"b\")\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("takes 1 argument(s), got 2")),
            "{errors:?}"
        );
    }

    #[test]
    fn forward_fn_call_ok() {
        ok("fn f() {\ng()\n}\nfn g() {\n}\n");
    }

    #[test]
    fn on_update_signature() {
        let errors = err("fn on_update(x: i32) {\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("on_update must take exactly one f32")),
            "{errors:?}"
        );
        let errors = err("fn on_create(a: i32) {\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("on_create must take no parameters")),
            "{errors:?}"
        );
    }

    #[test]
    fn bare_expr_statement_rejected() {
        let errors = err("fn f() {\n1 + 2\n}\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("only calls and assignments")),
            "{errors:?}"
        );
    }

    #[test]
    fn syntax_error_flows_through() {
        let errors = err("var x = 1\n");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("t.em:1:"));
    }
}
