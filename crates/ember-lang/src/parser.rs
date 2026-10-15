use crate::ast::*;
use crate::lexer::{Tok, Token, desc};
use crate::{Error, Span};

pub fn parse(file: &str, src: &str, toks: &[Token]) -> Result<Program, Error> {
    P {
        file,
        src,
        toks,
        pos: 0,
    }
    .program()
}

struct P<'a> {
    file: &'a str,
    src: &'a str,
    toks: &'a [Token],
    pos: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.toks.get(self.pos)
    }

    fn err(&self, span: Span, message: String) -> Error {
        Error::new(self.file, self.src, span.start as usize, span, message)
    }

    fn err_eof(&self, message: String) -> Error {
        let at = self.src.len();
        self.err(Span::empty(at), message)
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }

    fn eat(&mut self, want: fn(&Tok) -> bool) -> Option<Token> {
        match self.peek() {
            Some(t) if want(&t.tok) => Some(self.bump()),
            _ => None,
        }
    }

    fn expect(&mut self, want: &'static str, ok: fn(&Tok) -> bool) -> Result<Token, Error> {
        match self.peek() {
            Some(t) if ok(&t.tok) => Ok(self.bump()),
            Some(t) => {
                let span = t.span;
                Err(self.err(span, format!("expected {want}, found {}", desc(&t.tok))))
            }
            None => Err(self.err_eof(format!("expected {want}, found end of file"))),
        }
    }

    fn program(&mut self) -> Result<Program, Error> {
        let mut items = Vec::new();
        while let Some(t) = self.peek() {
            match &t.tok {
                Tok::Var => {
                    let v = self.var_decl()?;
                    items.push(Item::GlobalVar(GlobalVar {
                        name: v.name,
                        ty: v.ty,
                        init: v.init,
                        span: v.span,
                    }));
                }
                Tok::Fn => items.push(Item::Function(self.function()?)),
                _ => {
                    let span = t.span;
                    return Err(self.err(
                        span,
                        format!("expected 'var' or 'fn', found {}", desc(&t.tok)),
                    ));
                }
            }
        }
        Ok(Program {
            items,
            span: Span::new(0, self.src.len()),
        })
    }

    fn ident(&mut self) -> Result<(String, Span), Error> {
        match self.peek() {
            Some(Token {
                tok: Tok::Ident(name),
                span,
            }) => {
                let out = (name.clone(), *span);
                self.pos += 1;
                Ok(out)
            }
            Some(t) => {
                let span = t.span;
                Err(self.err(span, format!("expected identifier, found {}", desc(&t.tok))))
            }
            None => Err(self.err_eof("expected identifier, found end of file".to_string())),
        }
    }

    fn ty(&mut self) -> Result<Type, Error> {
        let (name, span) = self.ident()?;
        Ok(Type { name, span })
    }

    fn var_decl(&mut self) -> Result<VarDecl, Error> {
        let kw = self.expect("'var'", |t| matches!(t, Tok::Var))?;
        let (name, _) = self.ident()?;
        self.expect("':'", |t| matches!(t, Tok::Colon))?;
        let ty = self.ty()?;
        self.expect("'='", |t| matches!(t, Tok::Assign))?;
        let init = self.expr()?;
        let span = Span::new(kw.span.start as usize, expr_end(&init) as usize);
        Ok(VarDecl {
            name,
            ty,
            init,
            span,
        })
    }

    fn function(&mut self) -> Result<Function, Error> {
        let kw = self.expect("'fn'", |t| matches!(t, Tok::Fn))?;
        let (name, _) = self.ident()?;
        self.expect("'('", |t| matches!(t, Tok::LParen))?;
        let mut params = Vec::new();
        if self.eat(|t| matches!(t, Tok::RParen)).is_none() {
            loop {
                let (pname, pspan) = self.ident()?;
                self.expect("':'", |t| matches!(t, Tok::Colon))?;
                let ty = self.ty()?;
                let span = Span::new(pspan.start as usize, ty.span.end as usize);
                params.push(Param {
                    name: pname,
                    ty,
                    span,
                });
                if self.eat(|t| matches!(t, Tok::Comma)).is_none() {
                    break;
                }
            }
            self.expect("')'", |t| matches!(t, Tok::RParen))?;
        }
        let body = self.block()?;
        let span = Span::new(kw.span.start as usize, body.span.end as usize);
        Ok(Function {
            name,
            params,
            body,
            span,
        })
    }

    fn block(&mut self) -> Result<Block, Error> {
        let open = self.expect("'{'", |t| matches!(t, Tok::LBrace))?;
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                None => return Err(self.err_eof("expected '}', found end of file".to_string())),
                Some(t) if matches!(t.tok, Tok::RBrace) => {
                    let close = self.bump();
                    let span = Span::new(open.span.start as usize, close.span.end as usize);
                    return Ok(Block { stmts, span });
                }
                Some(_) => stmts.push(self.stmt()?),
            }
        }
    }

    fn stmt(&mut self) -> Result<Stmt, Error> {
        match self.peek() {
            Some(t) => match &t.tok {
                Tok::Var => {
                    let v = self.var_decl()?;
                    Ok(Stmt::Var(v))
                }
                Tok::If => self.if_stmt(),
                Tok::While => {
                    let kw = self.bump();
                    let cond = self.expr()?;
                    let body = self.block()?;
                    let span = Span::new(kw.span.start as usize, body.span.end as usize);
                    Ok(Stmt::While { cond, body, span })
                }
                Tok::Return => {
                    let kw = self.bump();
                    let value = if self.expr_starts() {
                        Some(self.expr()?)
                    } else {
                        None
                    };
                    let end = match &value {
                        Some(e) => expr_end(e),
                        None => kw.span.end,
                    };
                    Ok(Stmt::Return {
                        value,
                        span: Span::new(kw.span.start as usize, end as usize),
                    })
                }
                _ => self.assign_or_expr(),
            },
            None => Err(self.err_eof("expected statement, found end of file".to_string())),
        }
    }

    fn expr_starts(&self) -> bool {
        matches!(
            self.peek().map(|t| &t.tok),
            Some(
                Tok::Ident(_)
                    | Tok::Int(_)
                    | Tok::Float(_)
                    | Tok::Str(_)
                    | Tok::True
                    | Tok::False
                    | Tok::LParen
                    | Tok::Minus
                    | Tok::Bang
            )
        )
    }

    fn assign_or_expr(&mut self) -> Result<Stmt, Error> {
        let lhs = self.logic_or()?;
        if self.eat(|t| matches!(t, Tok::Assign)).is_some() {
            match lhs {
                Expr::Path(target) => {
                    let value = self.logic_or()?;
                    let span = Span::new(target.span.start as usize, expr_end(&value) as usize);
                    Ok(Stmt::Assign {
                        target,
                        value,
                        span,
                    })
                }
                _ => Err(self.err(
                    expr_span(&lhs),
                    "assignment target must be a variable".to_string(),
                )),
            }
        } else {
            Ok(Stmt::Expr(lhs))
        }
    }

    fn if_stmt(&mut self) -> Result<Stmt, Error> {
        let kw = self.expect("'if'", |t| matches!(t, Tok::If))?;
        let cond = self.expr()?;
        let then = self.block()?;
        let mut end = then.span.end;
        let els = if self.eat(|t| matches!(t, Tok::Else)).is_some() {
            match self.peek() {
                Some(t) if matches!(t.tok, Tok::If) => {
                    let nested = self.if_stmt()?;
                    end = stmt_end(&nested);
                    Some(Else::If(Box::new(nested)))
                }
                _ => {
                    let b = self.block()?;
                    end = b.span.end;
                    Some(Else::Block(b))
                }
            }
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then,
            els,
            span: Span::new(kw.span.start as usize, end as usize),
        })
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        self.logic_or()
    }

    fn logic_or(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.logic_and()?;
        while self.eat(|t| matches!(t, Tok::Or2)).is_some() {
            let rhs = self.logic_and()?;
            lhs = binary(BinOp::Or, lhs, rhs);
        }
        Ok(lhs)
    }

    fn logic_and(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.equality()?;
        while self.eat(|t| matches!(t, Tok::And2)).is_some() {
            let rhs = self.equality()?;
            lhs = binary(BinOp::And, lhs, rhs);
        }
        Ok(lhs)
    }

    fn equality(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.comparison()?;
        loop {
            let op = if self.eat(|t| matches!(t, Tok::Eq2)).is_some() {
                BinOp::Eq
            } else if self.eat(|t| matches!(t, Tok::BangEq)).is_some() {
                BinOp::Ne
            } else {
                break;
            };
            let rhs = self.comparison()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn comparison(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.additive()?;
        loop {
            let op = if self.eat(|t| matches!(t, Tok::LtEq)).is_some() {
                BinOp::Le
            } else if self.eat(|t| matches!(t, Tok::GtEq)).is_some() {
                BinOp::Ge
            } else if self.eat(|t| matches!(t, Tok::Lt)).is_some() {
                BinOp::Lt
            } else if self.eat(|t| matches!(t, Tok::Gt)).is_some() {
                BinOp::Gt
            } else {
                break;
            };
            let rhs = self.additive()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn additive(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.multiplicative()?;
        loop {
            let op = if self.eat(|t| matches!(t, Tok::Plus)).is_some() {
                BinOp::Add
            } else if self.eat(|t| matches!(t, Tok::Minus)).is_some() {
                BinOp::Sub
            } else {
                break;
            };
            let rhs = self.multiplicative()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn multiplicative(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;
        loop {
            let op = if self.eat(|t| matches!(t, Tok::Star)).is_some() {
                BinOp::Mul
            } else if self.eat(|t| matches!(t, Tok::Slash)).is_some() {
                BinOp::Div
            } else if self.eat(|t| matches!(t, Tok::Percent)).is_some() {
                BinOp::Mod
            } else {
                break;
            };
            let rhs = self.unary()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if let Some(op) = self.eat(|t| matches!(t, Tok::Minus | Tok::Bang)) {
            let rhs = self.unary()?;
            let span = Span::new(op.span.start as usize, expr_end(&rhs) as usize);
            let op = if matches!(op.tok, Tok::Minus) {
                UnOp::Neg
            } else {
                UnOp::Not
            };
            return Ok(Expr::Unary {
                op,
                rhs: Box::new(rhs),
                span,
            });
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, Error> {
        let mut expr = self.primary()?;
        while self.eat(|t| matches!(t, Tok::LParen)).is_some() {
            let func = match expr {
                Expr::Path(p) => p,
                _ => {
                    return Err(self.err(
                        expr_span(&expr),
                        "cannot call a non-function value".to_string(),
                    ));
                }
            };
            let mut args = Vec::new();
            if self.eat(|t| matches!(t, Tok::RParen)).is_none() {
                loop {
                    args.push(self.expr()?);
                    if self.eat(|t| matches!(t, Tok::Comma)).is_none() {
                        break;
                    }
                }
                self.expect("')'", |t| matches!(t, Tok::RParen))?;
            }
            let end = self.toks[self.pos - 1].span.end;
            expr = Expr::Call {
                span: Span::new(func.span.start as usize, end as usize),
                func,
                args,
            };
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.peek() {
            Some(t) => match &t.tok {
                Tok::Int(v) => {
                    let (v, span) = (*v, t.span);
                    self.pos += 1;
                    Ok(Expr::Int(v, span))
                }
                Tok::Float(v) => {
                    let (v, span) = (*v, t.span);
                    self.pos += 1;
                    Ok(Expr::Float(v, span))
                }
                Tok::Str(s) => {
                    let (s, span) = (s.clone(), t.span);
                    self.pos += 1;
                    Ok(Expr::Str(s, span))
                }
                Tok::True => {
                    let span = t.span;
                    self.pos += 1;
                    Ok(Expr::Bool(true, span))
                }
                Tok::False => {
                    let span = t.span;
                    self.pos += 1;
                    Ok(Expr::Bool(false, span))
                }
                Tok::Ident(_) => Ok(Expr::Path(self.path()?)),
                Tok::LParen => {
                    self.pos += 1;
                    let inner = self.expr()?;
                    self.expect("')'", |t| matches!(t, Tok::RParen))?;
                    Ok(inner)
                }
                _ => {
                    let span = t.span;
                    Err(self.err(span, format!("expected expression, found {}", desc(&t.tok))))
                }
            },
            None => Err(self.err_eof("expected expression, found end of file".to_string())),
        }
    }

    fn path(&mut self) -> Result<Path, Error> {
        let (first, start) = self.ident()?;
        let mut parts = vec![first];
        let mut end = start.end;
        while self.eat(|t| matches!(t, Tok::Dot)).is_some() {
            let (next, span) = self.ident()?;
            end = span.end;
            parts.push(next);
        }
        Ok(Path {
            parts,
            span: Span::new(start.start as usize, end as usize),
        })
    }
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = Span::new(expr_span(&lhs).start as usize, expr_end(&rhs) as usize);
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span,
    }
}

fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Int(_, s) | Expr::Float(_, s) | Expr::Str(_, s) | Expr::Bool(_, s) => *s,
        Expr::Path(p) => p.span,
        Expr::Call { span, .. } | Expr::Unary { span, .. } | Expr::Binary { span, .. } => *span,
    }
}

fn expr_end(e: &Expr) -> u32 {
    expr_span(e).end
}

fn stmt_end(s: &Stmt) -> u32 {
    match s {
        Stmt::Var(v) => v.span.end,
        Stmt::Assign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::Return { span, .. } => span.end,
        Stmt::Expr(e) => expr_end(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(src: &str) -> Program {
        crate::parse("t.em", src).unwrap()
    }

    fn parse_err(src: &str) -> Error {
        crate::parse("t.em", src).unwrap_err()
    }

    #[test]
    fn starter_script_shape() {
        let src =
            "var speed: f32 = 220.0\nfn on_create() {\nlog(\"x\")\n}\nfn on_update(dt: f32) {\n}\n";
        let p = parse_one(src);
        assert_eq!(p.items.len(), 3);
        assert!(matches!(p.items[0], Item::GlobalVar(_)));
        if let Item::Function(f) = &p.items[2] {
            assert_eq!(f.name, "on_update");
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].ty.name, "f32");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn host_path_call() {
        let p = parse_one("fn f() {\ninput.down(Key.Right)\n}\n");
        if let Item::Function(f) = &p.items[0]
            && let Stmt::Expr(Expr::Call { func, args, .. }) = &f.body.stmts[0]
        {
            assert_eq!(func.parts, vec!["input".to_string(), "down".to_string()]);
            assert_eq!(args.len(), 1);
        } else {
            panic!("expected host call");
        }
    }

    #[test]
    fn precedence_arith() {
        let p = parse_one("fn f() {\n1 + 2 * 3\n}\n");
        if let Item::Function(f) = &p.items[0]
            && let Stmt::Expr(Expr::Binary {
                op: BinOp::Add,
                rhs,
                ..
            }) = &f.body.stmts[0]
            && let Expr::Binary { op: BinOp::Mul, .. } = rhs.as_ref()
        {
        } else {
            panic!("expected add(mul)");
        }
    }

    #[test]
    fn precedence_bool() {
        let p = parse_one("fn f() {\na && b || c\n}\n");
        if let Item::Function(f) = &p.items[0]
            && let Stmt::Expr(Expr::Binary {
                op: BinOp::Or, lhs, ..
            }) = &f.body.stmts[0]
            && let Expr::Binary { op: BinOp::And, .. } = lhs.as_ref()
        {
        } else {
            panic!("expected or(and)");
        }
    }

    #[test]
    fn unary_and_paren() {
        let p = parse_one("fn f() {\n!(a == 1)\n}\n");
        if let Item::Function(f) = &p.items[0]
            && let Stmt::Expr(Expr::Unary { op: UnOp::Not, .. }) = &f.body.stmts[0]
        {
        } else {
            panic!("expected not");
        }
    }

    #[test]
    fn assignment_shape() {
        let p = parse_one("fn f() {\nx = 1 + 2\n}\n");
        if let Item::Function(f) = &p.items[0]
            && let Stmt::Assign { target, value, .. } = &f.body.stmts[0]
        {
            assert_eq!(target.parts, vec!["x".to_string()]);
            assert!(matches!(value, Expr::Binary { .. }));
        } else {
            panic!("expected assign");
        }
    }

    #[test]
    fn if_else_while_return() {
        let src = "fn f() {\nif a {\nreturn 1\n} else {\nwhile b {\nreturn\n}\n}\n}\n";
        let p = parse_one(src);
        if let Item::Function(f) = &p.items[0]
            && let Stmt::If {
                els: Some(Else::Block(_)),
                ..
            } = &f.body.stmts[0]
        {
        } else {
            panic!("expected if-else");
        }
    }

    #[test]
    fn else_if_chains() {
        let p = parse_one("fn f() {\nif a {\n} else if b {\n}\n}\n");
        if let Item::Function(f) = &p.items[0]
            && let Stmt::If {
                els: Some(Else::If(_)),
                ..
            } = &f.body.stmts[0]
        {
        } else {
            panic!("expected else-if");
        }
    }

    #[test]
    fn error_missing_type_has_location() {
        let e = parse_err("var speed = 220.0\n");
        assert_eq!((e.line, e.col), (1, 11));
        assert!(e.to_string().starts_with("t.em:1:11:"));
    }

    #[test]
    fn error_unclosed_brace() {
        let e = parse_err("fn f() {\n");
        assert!(e.message.contains("'}'"));
    }

    #[test]
    fn error_call_non_function() {
        let e = parse_err("fn f() {\n(1)(2)\n}\n");
        assert!(e.message.contains("non-function"));
    }

    #[test]
    fn error_assign_to_call() {
        let e = parse_err("fn f() {\ng(1) = 2\n}\n");
        assert!(e.message.contains("must be a variable"));
    }

    #[test]
    fn error_trailing_item() {
        let e = parse_err("var a: i32 = 1\n42\n");
        assert!(e.message.contains("'var' or 'fn'"));
    }

    #[test]
    fn empty_program_ok() {
        assert!(parse_one("").items.is_empty());
        assert!(parse_one("// only a comment\n").items.is_empty());
    }

    #[test]
    fn spans_point_into_source() {
        let src = "var speed: f32 = 220.0\n";
        let p = parse_one(src);
        if let Item::GlobalVar(g) = &p.items[0] {
            assert_eq!(
                &src[g.span.start as usize..g.span.end as usize],
                "var speed: f32 = 220.0"
            );
        } else {
            panic!("expected global");
        }
    }
}
