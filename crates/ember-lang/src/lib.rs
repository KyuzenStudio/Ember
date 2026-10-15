pub mod ast;
pub mod compile;
pub mod host;
pub mod lexer;
pub mod parser;
pub mod semantic;

use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn empty(at: usize) -> Self {
        Self::new(at, at)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub span: Span,
}

impl Error {
    pub fn new(file: &str, src: &str, offset: usize, span: Span, message: String) -> Self {
        let (line, col) = line_col(src, offset);
        Self {
            file: file.to_string(),
            line,
            col,
            message,
            span,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.col, self.message
        )
    }
}

impl std::error::Error for Error {}

pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut end = offset.min(src.len());
    while !src.is_char_boundary(end) {
        end -= 1;
    }
    let head = &src[..end];
    let line = head.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = match head.rfind('\n') {
        Some(i) => head[i + 1..].chars().count() + 1,
        None => head.chars().count() + 1,
    };
    (line, col)
}

pub fn parse(file: &str, src: &str) -> Result<ast::Program, Error> {
    let tokens = lexer::lex(file, src)?;
    parser::parse(file, src, &tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_first_line() {
        assert_eq!(line_col("ab", 0), (1, 1));
        assert_eq!(line_col("ab", 2), (1, 3));
    }

    #[test]
    fn line_col_second_line() {
        assert_eq!(line_col("a\nbc", 2), (2, 1));
        assert_eq!(line_col("a\nbc", 4), (2, 3));
    }

    #[test]
    fn line_col_clamps_offset() {
        assert_eq!(line_col("ab", 99), (1, 3));
    }

    #[test]
    fn error_display_has_location() {
        let e = Error::new("p.em", "var x", 4, Span::empty(4), "oops".to_string());
        assert_eq!(e.to_string(), "p.em:1:5: oops");
    }
}
