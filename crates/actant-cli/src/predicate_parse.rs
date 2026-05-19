//! Tiny recursive-descent parser for `actantdb watch <expr>`.
//!
//! Grammar (case-insensitive operators / `AND` / `OR` / `NOT`):
//!
//! ```text
//! expr     := or
//! or       := and ('OR' and)*
//! and      := not ('AND' not)*
//! not      := 'NOT' not | atom
//! atom     := '(' expr ')'
//!           | field op literal
//!           | 'EXISTS' '(' field ')'
//! field    := IDENT ('.' IDENT_OR_NUM)*
//! op       := '==' | '!=' | '<' | '<=' | '>' | '>='
//! literal  := STRING | NUMBER | 'true' | 'false' | 'null'
//! ```
//!
//! Produces an [`actant_subscribe::Predicate`] for use against a JSON payload.

use actant_subscribe::Predicate;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LParen,
    RParen,
    Dot,
    Comma,
    True,
    False,
    Null,
    And,
    Or,
    Not,
    Exists,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '(' {
            out.push(Tok::LParen);
            i += 1;
            continue;
        }
        if c == ')' {
            out.push(Tok::RParen);
            i += 1;
            continue;
        }
        if c == '.' {
            out.push(Tok::Dot);
            i += 1;
            continue;
        }
        if c == ',' {
            out.push(Tok::Comma);
            i += 1;
            continue;
        }
        if c == '=' && i + 1 < bytes.len() && bytes[i + 1] == b'=' {
            out.push(Tok::Eq);
            i += 2;
            continue;
        }
        if c == '!' && i + 1 < bytes.len() && bytes[i + 1] == b'=' {
            out.push(Tok::Ne);
            i += 2;
            continue;
        }
        if c == '<' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                out.push(Tok::Le);
                i += 2;
            } else {
                out.push(Tok::Lt);
                i += 1;
            }
            continue;
        }
        if c == '>' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                out.push(Tok::Ge);
                i += 2;
            } else {
                out.push(Tok::Gt);
                i += 1;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] as char != quote {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i >= bytes.len() {
                return Err("unterminated string literal".into());
            }
            let raw = &src[start..i];
            let unescaped = raw
                .replace("\\\"", "\"")
                .replace("\\'", "'")
                .replace("\\\\", "\\");
            out.push(Tok::Str(unescaped));
            i += 1; // closing quote
            continue;
        }
        if c.is_ascii_digit()
            || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            if c == '-' {
                i += 1;
            }
            while i < bytes.len() && {
                let ch = bytes[i] as char;
                ch.is_ascii_digit() || ch == '.' || ch == 'e' || ch == 'E' || ch == '+' || ch == '-'
            } {
                i += 1;
            }
            let s = &src[start..i];
            let n: f64 = s.parse().map_err(|e| format!("bad number `{s}`: {e}"))?;
            out.push(Tok::Num(n));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch.is_alphanumeric() || ch == '_' {
                    i += 1;
                } else {
                    break;
                }
            }
            let word = &src[start..i];
            let upper = word.to_ascii_uppercase();
            let tok = match upper.as_str() {
                "AND" => Tok::And,
                "OR" => Tok::Or,
                "NOT" => Tok::Not,
                "TRUE" => Tok::True,
                "FALSE" => Tok::False,
                "NULL" => Tok::Null,
                "EXISTS" => Tok::Exists,
                _ => Tok::Ident(word.to_string()),
            };
            out.push(tok);
            continue;
        }
        return Err(format!("unexpected character `{c}` at offset {i}"));
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_expr(&mut self) -> Result<Predicate, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Predicate, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Tok::Or)) {
            self.bump();
            let right = self.parse_and()?;
            left = match left {
                Predicate::Or(mut xs) => {
                    xs.push(right);
                    Predicate::Or(xs)
                }
                other => Predicate::Or(vec![other, right]),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Predicate, String> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Some(Tok::And)) {
            self.bump();
            let right = self.parse_not()?;
            left = match left {
                Predicate::And(mut xs) => {
                    xs.push(right);
                    Predicate::And(xs)
                }
                other => Predicate::And(vec![other, right]),
            };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Predicate, String> {
        if matches!(self.peek(), Some(Tok::Not)) {
            self.bump();
            let inner = self.parse_not()?;
            return Ok(Predicate::Not(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<Predicate, String> {
        match self.peek().cloned() {
            Some(Tok::LParen) => {
                self.bump();
                let inner = self.parse_expr()?;
                match self.bump() {
                    Some(Tok::RParen) => Ok(inner),
                    _ => Err("expected `)`".into()),
                }
            }
            Some(Tok::Exists) => {
                self.bump();
                match self.bump() {
                    Some(Tok::LParen) => {}
                    _ => return Err("expected `(` after EXISTS".into()),
                }
                let field = self.parse_field()?;
                match self.bump() {
                    Some(Tok::RParen) => {}
                    _ => return Err("expected `)` after EXISTS field".into()),
                }
                Ok(Predicate::Exists { field })
            }
            Some(Tok::True) => {
                self.bump();
                Ok(Predicate::True)
            }
            Some(Tok::False) => {
                self.bump();
                Ok(Predicate::False)
            }
            Some(Tok::Ident(_)) => {
                let field = self.parse_field()?;
                let op = self
                    .bump()
                    .ok_or_else(|| "expected operator after field".to_string())?;
                let value = self.parse_literal()?;
                Ok(match op {
                    Tok::Eq => Predicate::Eq { field, value },
                    Tok::Ne => Predicate::Ne { field, value },
                    Tok::Lt => Predicate::Lt { field, value },
                    Tok::Le => Predicate::Le { field, value },
                    Tok::Gt => Predicate::Gt { field, value },
                    Tok::Ge => Predicate::Ge { field, value },
                    other => return Err(format!("expected comparison operator, got {other:?}")),
                })
            }
            Some(other) => Err(format!("unexpected token {other:?}")),
            None => Err("unexpected end of expression".into()),
        }
    }

    fn parse_field(&mut self) -> Result<String, String> {
        let first = match self.bump() {
            Some(Tok::Ident(s)) => s,
            _ => return Err("expected identifier".into()),
        };
        let mut path = first;
        while matches!(self.peek(), Some(Tok::Dot)) {
            self.bump();
            let seg = match self.bump() {
                Some(Tok::Ident(s)) => s,
                Some(Tok::Num(n)) if n.fract() == 0.0 && n >= 0.0 => (n as u64).to_string(),
                _ => return Err("expected identifier or index after `.`".into()),
            };
            path.push('.');
            path.push_str(&seg);
        }
        Ok(path)
    }

    fn parse_literal(&mut self) -> Result<Value, String> {
        match self.bump() {
            Some(Tok::Str(s)) => Ok(Value::String(s)),
            Some(Tok::Num(n)) => Ok(serde_json::Number::from_f64(n)
                .map(Value::Number)
                .unwrap_or(Value::Null)),
            Some(Tok::True) => Ok(Value::Bool(true)),
            Some(Tok::False) => Ok(Value::Bool(false)),
            Some(Tok::Null) => Ok(Value::Null),
            Some(other) => Err(format!("expected literal, got {other:?}")),
            None => Err("expected literal".into()),
        }
    }
}

/// Parse a human-readable expression into a [`Predicate`].
pub fn parse(src: &str) -> Result<Predicate, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let pred = p.parse_expr()?;
    if p.pos != p.toks.len() {
        return Err(format!(
            "trailing tokens after expression at position {}",
            p.pos
        ));
    }
    Ok(pred)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simple_eq() {
        let p = parse(r#"kind == "tool_call_completed""#).unwrap();
        assert!(p.evaluate(&json!({"kind": "tool_call_completed"})));
        assert!(!p.evaluate(&json!({"kind": "other"})));
    }

    #[test]
    fn and_or_not() {
        let p = parse(r#"kind == "x" AND (payload.status == "ok" OR NOT payload.retry == true)"#)
            .unwrap();
        assert!(p.evaluate(&json!({"kind":"x","payload":{"status":"ok"}})));
    }

    #[test]
    fn exists() {
        let p = parse("EXISTS(payload.error)").unwrap();
        assert!(p.evaluate(&json!({"payload":{"error":"boom"}})));
        assert!(!p.evaluate(&json!({"payload":{}})));
    }

    #[test]
    fn numbers() {
        let p = parse("payload.took_ms >= 100").unwrap();
        assert!(p.evaluate(&json!({"payload":{"took_ms": 200}})));
        assert!(!p.evaluate(&json!({"payload":{"took_ms": 50}})));
    }
}
