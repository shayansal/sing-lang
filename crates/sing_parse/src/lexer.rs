use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Id(String),
    Attr(String),
    Int(String),
    Float(String),
    Str(String),
    Char(char),
    Sym(char),
    Op(String),
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Range<usize>,
}

pub fn lex(src: &str) -> Result<Vec<Token>, Vec<LexError>> {
    let mut lexer = Lexer {
        src,
        pos: 0,
        tokens: Vec::new(),
        errors: Vec::new(),
        at_item_start: true,
    };
    lexer.run();
    if lexer.errors.is_empty() {
        Ok(lexer.tokens)
    } else {
        Err(lexer.errors)
    }
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
    at_item_start: bool,
}

impl Lexer<'_> {
    fn run(&mut self) {
        while self.pos < self.src.len() {
            if self.skip_ws_or_comment() {
                continue;
            }

            let start = self.pos;
            let ch = self.peek_char().expect("pos is in bounds");

            if self.src[self.pos..].starts_with("b\"") {
                self.lex_byte_string();
                continue;
            }

            if is_id_start(ch) {
                self.lex_id_or_attr();
                continue;
            }

            if ch.is_ascii_digit() {
                self.lex_number();
                continue;
            }

            match ch {
                '"' => self.lex_string(),
                '\'' => self.lex_char(),
                _ => {
                    if let Some(op) = self.peek_two_char_op() {
                        self.pos += 2;
                        self.push(Tok::Op(op.to_string()), start..self.pos);
                    } else if is_single_symbol(ch) {
                        self.pos += ch.len_utf8();
                        self.push(Tok::Sym(ch), start..self.pos);
                    } else {
                        self.pos += ch.len_utf8();
                        self.error(format!("unexpected character `{ch}`"), start..self.pos);
                    }
                }
            }
        }

        let eof = self.src.len();
        self.tokens.push(Token {
            tok: Tok::Eof,
            span: eof..eof,
        });
    }

    fn skip_ws_or_comment(&mut self) -> bool {
        let rest = &self.src[self.pos..];
        if let Some(ch) = rest.chars().next() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
                return true;
            }
        }

        if rest.starts_with("//") {
            self.pos += 2;
            while let Some(ch) = self.peek_char() {
                self.pos += ch.len_utf8();
                if ch == '\n' {
                    break;
                }
            }
            return true;
        }

        if rest.starts_with("/*") {
            let start = self.pos;
            self.pos += 2;
            while self.pos < self.src.len() && !self.src[self.pos..].starts_with("*/") {
                let ch = self.peek_char().expect("pos is in bounds");
                self.pos += ch.len_utf8();
            }
            if self.pos < self.src.len() {
                self.pos += 2;
            } else {
                self.error("unterminated block comment", start..self.pos);
            }
            return true;
        }

        false
    }

    fn lex_id_or_attr(&mut self) {
        let start = self.pos;
        self.pos += self.peek_char().expect("pos is in bounds").len_utf8();
        while let Some(ch) = self.peek_char() {
            if is_id_continue(ch) {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }

        let text = &self.src[start..self.pos];
        if self.at_item_start
            && is_attr_cluster(text)
            && next_non_ws_or_comment(self.src, self.pos).is_some_and(is_top_item_start)
        {
            for (offset, ch) in text.char_indices() {
                let attr_start = start + offset;
                let attr_end = attr_start + ch.len_utf8();
                self.tokens.push(Token {
                    tok: Tok::Attr(ch.to_string()),
                    span: attr_start..attr_end,
                });
            }
            return;
        }

        self.push(Tok::Id(text.to_string()), start..self.pos);
    }

    fn lex_number(&mut self) {
        let start = self.pos;

        if self.src[self.pos..].starts_with("0x") || self.src[self.pos..].starts_with("0X") {
            self.pos += 2;
            while self
                .peek_char()
                .is_some_and(|ch| ch.is_ascii_hexdigit() || ch == '_')
            {
                self.pos += self.peek_char().expect("pos is in bounds").len_utf8();
            }
            self.lex_suffix();
            self.push(
                Tok::Int(self.src[start..self.pos].to_string()),
                start..self.pos,
            );
            return;
        }

        if self.src[self.pos..].starts_with("0b") || self.src[self.pos..].starts_with("0B") {
            self.pos += 2;
            while self
                .peek_char()
                .is_some_and(|ch| matches!(ch, '0' | '1' | '_'))
            {
                self.pos += self.peek_char().expect("pos is in bounds").len_utf8();
            }
            self.lex_suffix();
            self.push(
                Tok::Int(self.src[start..self.pos].to_string()),
                start..self.pos,
            );
            return;
        }

        while self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
        {
            self.pos += self.peek_char().expect("pos is in bounds").len_utf8();
        }

        let mut is_float = false;
        if self.src[self.pos..].starts_with('.')
            && !self.src[self.pos..].starts_with("..")
            && self
                .src
                .get(self.pos + 1..)
                .and_then(|s| s.chars().next())
                .is_some_and(|ch| ch.is_ascii_digit())
        {
            is_float = true;
            self.pos += 1;
            while self
                .peek_char()
                .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
            {
                self.pos += self.peek_char().expect("pos is in bounds").len_utf8();
            }
        }

        self.lex_suffix();
        let text = self.src[start..self.pos].to_string();
        if is_float {
            self.push(Tok::Float(text), start..self.pos);
        } else {
            self.push(Tok::Int(text), start..self.pos);
        }
    }

    fn lex_suffix(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.pos += self.peek_char().expect("pos is in bounds").len_utf8();
        }
    }

    fn lex_string(&mut self) {
        let start = self.pos;
        self.pos += 1;
        self.lex_string_body(start);
    }

    fn lex_byte_string(&mut self) {
        let start = self.pos;
        self.pos += 2;
        self.lex_string_body(start);
    }

    fn lex_string_body(&mut self, start: usize) {
        let mut out = String::new();
        while let Some(ch) = self.peek_char() {
            self.pos += ch.len_utf8();
            match ch {
                '"' => {
                    self.push(Tok::Str(out), start..self.pos);
                    return;
                }
                '\\' => {
                    if let Some(escaped) = self.peek_char() {
                        self.pos += escaped.len_utf8();
                        out.push(unescape(escaped));
                    } else {
                        break;
                    }
                }
                _ => out.push(ch),
            }
        }
        self.error("unterminated string literal", start..self.pos);
    }

    fn lex_char(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let value = match self.peek_char() {
            Some('\\') => {
                self.pos += 1;
                match self.peek_char() {
                    Some(ch) => {
                        self.pos += ch.len_utf8();
                        unescape(ch)
                    }
                    None => {
                        self.error("unterminated char literal", start..self.pos);
                        return;
                    }
                }
            }
            Some(ch) => {
                self.pos += ch.len_utf8();
                ch
            }
            None => {
                self.error("unterminated char literal", start..self.pos);
                return;
            }
        };

        if self.peek_char() == Some('\'') {
            self.pos += 1;
            self.push(Tok::Char(value), start..self.pos);
        } else {
            self.error("expected closing quote for char literal", start..self.pos);
        }
    }

    fn peek_two_char_op(&self) -> Option<&'static str> {
        const OPS: [&str; 15] = [
            ":=", "<=", ">=", "==", "!=", "+%", "-%", "*%", "+|", "-|", "*|", "+?", "-?", "*?",
            "..",
        ];
        OPS.into_iter()
            .find(|op| self.src[self.pos..].starts_with(op))
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn push(&mut self, tok: Tok, span: Range<usize>) {
        self.at_item_start = matches!(tok, Tok::Sym(';'));
        self.tokens.push(Token { tok, span });
    }

    fn error(&mut self, message: impl Into<String>, span: Range<usize>) {
        self.errors.push(LexError {
            message: message.into(),
            span,
        });
    }
}

fn is_id_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_id_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_attr_cluster(text: &str) -> bool {
    const ATTRS: &str = "KVPCUHDGZRINOELS";
    !text.is_empty()
        && text.len() <= 2
        && text
            .chars()
            .all(|ch| ch.is_ascii_uppercase() && ATTRS.contains(ch))
}

fn is_top_item_start(ch: char) -> bool {
    matches!(
        ch,
        'm' | 'u' | 'a' | 'k' | 't' | 'e' | 'q' | 'i' | 'f' | 'x' | 'p' | '#'
    )
}

fn next_non_ws_or_comment(src: &str, mut pos: usize) -> Option<char> {
    loop {
        let rest = src.get(pos..)?;
        let ch = rest.chars().next()?;
        if ch.is_whitespace() {
            pos += ch.len_utf8();
            continue;
        }
        if rest.starts_with("//") {
            pos += 2;
            while let Some(ch) = src.get(pos..).and_then(|s| s.chars().next()) {
                pos += ch.len_utf8();
                if ch == '\n' {
                    break;
                }
            }
            continue;
        }
        if rest.starts_with("/*") {
            pos += 2;
            while pos < src.len() && !src[pos..].starts_with("*/") {
                let ch = src[pos..].chars().next()?;
                pos += ch.len_utf8();
            }
            if pos < src.len() {
                pos += 2;
                continue;
            }
            return None;
        }
        return Some(ch);
    }
}

fn is_single_symbol(ch: char) -> bool {
    "(){}[],:;.!?~&|^+-*/%=<>#\\_".contains(ch)
}

fn unescape(ch: char) -> char {
    match ch {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '0' => '\0',
        other => other,
    }
}
