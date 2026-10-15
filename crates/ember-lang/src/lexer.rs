use crate::{Error, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Var,
    Fn,
    If,
    Else,
    While,
    Return,
    True,
    False,
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq2,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And2,
    Or2,
    Bang,
    Assign,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub fn desc(tok: &Tok) -> &'static str {
    match tok {
        Tok::Var => "'var'",
        Tok::Fn => "'fn'",
        Tok::If => "'if'",
        Tok::Else => "'else'",
        Tok::While => "'while'",
        Tok::Return => "'return'",
        Tok::True | Tok::False => "boolean literal",
        Tok::Ident(_) => "identifier",
        Tok::Int(_) | Tok::Float(_) => "number",
        Tok::Str(_) => "string",
        Tok::Plus => "'+'",
        Tok::Minus => "'-'",
        Tok::Star => "'*'",
        Tok::Slash => "'/'",
        Tok::Percent => "'%'",
        Tok::Eq2 => "'=='",
        Tok::BangEq => "'!='",
        Tok::Lt => "'<'",
        Tok::LtEq => "'<='",
        Tok::Gt => "'>'",
        Tok::GtEq => "'>='",
        Tok::And2 => "'&&'",
        Tok::Or2 => "'||'",
        Tok::Bang => "'!'",
        Tok::Assign => "'='",
        Tok::LParen => "'('",
        Tok::RParen => "')'",
        Tok::LBrace => "'{'",
        Tok::RBrace => "'}'",
        Tok::Comma => "','",
        Tok::Colon => "':'",
        Tok::Dot => "'.'",
    }
}

struct Lex<'a> {
    file: &'a str,
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lex<'a> {
    fn err(&self, at: usize, span: Span, message: String) -> Error {
        Error::new(
            self.file,
            std::str::from_utf8(self.src).unwrap_or(""),
            at,
            span,
            message,
        )
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    fn word(&mut self) -> (String, Span) {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        (
            String::from_utf8_lossy(&self.src[start..self.pos]).into_owned(),
            Span::new(start, self.pos),
        )
    }

    fn number(&mut self) -> Result<(Tok, Span), Error> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') && matches!(self.peek2(), Some(b'0'..=b'9')) {
            self.pos += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            let text = &self.src[start..self.pos];
            let text = std::str::from_utf8(text).unwrap_or("");
            match text.parse::<f64>() {
                Ok(v) => Ok((Tok::Float(v), Span::new(start, self.pos))),
                Err(_) => Err(self.err(
                    start,
                    Span::new(start, self.pos),
                    format!("invalid float '{text}'"),
                )),
            }
        } else {
            let text = &self.src[start..self.pos];
            let text = std::str::from_utf8(text).unwrap_or("");
            match text.parse::<i64>() {
                Ok(v) => Ok((Tok::Int(v), Span::new(start, self.pos))),
                Err(_) => Err(self.err(
                    start,
                    Span::new(start, self.pos),
                    format!("integer out of range '{text}'"),
                )),
            }
        }
    }

    fn string(&mut self) -> Result<(Tok, Span), Error> {
        let start = self.pos;
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                None => {
                    return Err(self.err(
                        start,
                        Span::new(start, self.pos),
                        "unterminated string".to_string(),
                    ));
                }
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\n') => {
                    return Err(self.err(
                        start,
                        Span::new(start, self.pos),
                        "unterminated string".to_string(),
                    ));
                }
                Some(b'\\') => {
                    let at = self.pos;
                    match self.peek2() {
                        Some(b'"') => out.push(b'"'),
                        Some(b'\\') => out.push(b'\\'),
                        Some(b'n') => out.push(b'\n'),
                        Some(b't') => out.push(b'\t'),
                        _ => {
                            return Err(self.err(
                                at,
                                Span::new(at, at + 2.min(self.src.len() - at)),
                                "unknown escape".to_string(),
                            ));
                        }
                    }
                    self.pos += 2;
                }
                Some(b) => {
                    out.push(b);
                    self.pos += 1;
                }
            }
        }
        match String::from_utf8(out) {
            Ok(s) => Ok((Tok::Str(s), Span::new(start, self.pos))),
            Err(_) => Err(self.err(
                start,
                Span::new(start, self.pos),
                "invalid utf-8 in string".to_string(),
            )),
        }
    }
}

pub fn lex(file: &str, src: &str) -> Result<Vec<Token>, Error> {
    let bytes = src.as_bytes();
    let mut lx = Lex {
        file,
        src: bytes,
        pos: 0,
    };
    let mut out = Vec::new();
    while lx.pos < bytes.len() {
        let b = bytes[lx.pos];
        if b.is_ascii_whitespace() {
            lx.pos += 1;
            continue;
        }
        if b == b'/' && lx.peek2() == Some(b'/') {
            while matches!(lx.peek(), Some(c) if c != b'\n') {
                lx.pos += 1;
            }
            continue;
        }
        let start = lx.pos;
        let tok = match b {
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b'{' => Tok::LBrace,
            b'}' => Tok::RBrace,
            b',' => Tok::Comma,
            b':' => Tok::Colon,
            b'.' => Tok::Dot,
            b'+' => Tok::Plus,
            b'-' => Tok::Minus,
            b'*' => Tok::Star,
            b'/' => Tok::Slash,
            b'%' => Tok::Percent,
            b'=' if lx.peek2() == Some(b'=') => {
                lx.pos += 1;
                Tok::Eq2
            }
            b'=' => Tok::Assign,
            b'!' if lx.peek2() == Some(b'=') => {
                lx.pos += 1;
                Tok::BangEq
            }
            b'!' => Tok::Bang,
            b'<' if lx.peek2() == Some(b'=') => {
                lx.pos += 1;
                Tok::LtEq
            }
            b'<' => Tok::Lt,
            b'>' if lx.peek2() == Some(b'=') => {
                lx.pos += 1;
                Tok::GtEq
            }
            b'>' => Tok::Gt,
            b'&' if lx.peek2() == Some(b'&') => {
                lx.pos += 1;
                Tok::And2
            }
            b'|' if lx.peek2() == Some(b'|') => {
                lx.pos += 1;
                Tok::Or2
            }
            b'"' => {
                let (t, span) = lx.string()?;
                out.push(Token { tok: t, span });
                continue;
            }
            b'0'..=b'9' => {
                let (t, span) = lx.number()?;
                out.push(Token { tok: t, span });
                continue;
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                let (w, span) = lx.word();
                let t = match w.as_str() {
                    "var" => Tok::Var,
                    "fn" => Tok::Fn,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "return" => Tok::Return,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    _ => Tok::Ident(w),
                };
                out.push(Token { tok: t, span });
                continue;
            }
            _ => {
                let ch = String::from_utf8_lossy(&bytes[start..start + 1]);
                return Err(lx.err(
                    start,
                    Span::new(start, start + 1),
                    format!("unexpected character '{ch}'"),
                ));
            }
        };
        lx.pos += 1;
        out.push(Token {
            tok,
            span: Span::new(start, lx.pos),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        lex("t.em", src)
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    #[test]
    fn keywords_and_symbols() {
        assert_eq!(
            toks("var fn if else while return true false"),
            vec![
                Tok::Var,
                Tok::Fn,
                Tok::If,
                Tok::Else,
                Tok::While,
                Tok::Return,
                Tok::True,
                Tok::False
            ]
        );
        assert_eq!(
            toks("+-*/% == != < <= > >= && || ! = ( ) { } , : ."),
            vec![
                Tok::Plus,
                Tok::Minus,
                Tok::Star,
                Tok::Slash,
                Tok::Percent,
                Tok::Eq2,
                Tok::BangEq,
                Tok::Lt,
                Tok::LtEq,
                Tok::Gt,
                Tok::GtEq,
                Tok::And2,
                Tok::Or2,
                Tok::Bang,
                Tok::Assign,
                Tok::LParen,
                Tok::RParen,
                Tok::LBrace,
                Tok::RBrace,
                Tok::Comma,
                Tok::Colon,
                Tok::Dot,
            ]
        );
    }

    #[test]
    fn ident_number_string() {
        assert_eq!(
            toks("speed x1 _a 42 220.0 \"hi\""),
            vec![
                Tok::Ident("speed".to_string()),
                Tok::Ident("x1".to_string()),
                Tok::Ident("_a".to_string()),
                Tok::Int(42),
                Tok::Float(220.0),
                Tok::Str("hi".to_string()),
            ]
        );
    }

    #[test]
    fn string_escapes() {
        assert_eq!(
            toks("\"a\\\"b\\\\c\\nd\""),
            vec![Tok::Str("a\"b\\c\nd".to_string())]
        );
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(toks("// hello\nvar // tail"), vec![Tok::Var]);
    }

    #[test]
    fn spans_cover_source() {
        let src = "var x";
        let ts = lex("t.em", src).unwrap();
        assert_eq!(ts[0].span, Span::new(0, 3));
        assert_eq!(ts[1].span, Span::new(4, 5));
    }

    #[test]
    fn error_unterminated_string_has_location() {
        let e = lex("p.em", "var x = \"abc").unwrap_err();
        assert_eq!((e.line, e.col), (1, 9));
        assert!(e.to_string().starts_with("p.em:1:9:"));
    }

    #[test]
    fn error_bad_escape_second_line() {
        let e = lex("p.em", "var a = 1\nvar b = \"\\q\"").unwrap_err();
        assert_eq!((e.line, e.col), (2, 10));
    }

    #[test]
    fn error_unexpected_character() {
        let e = lex("p.em", "var @").unwrap_err();
        assert!(e.message.contains('@'));
    }

    #[test]
    fn error_integer_overflow() {
        let e = lex("p.em", "9999999999999999999999").unwrap_err();
        assert!(e.message.contains("out of range"));
    }

    #[test]
    fn int_dot_splits() {
        assert_eq!(
            toks("5.foo"),
            vec![Tok::Int(5), Tok::Dot, Tok::Ident("foo".to_string())]
        );
    }
}
