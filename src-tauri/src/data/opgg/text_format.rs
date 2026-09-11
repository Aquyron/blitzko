//! Parser for OP.GG MCP's response format.
//!
//! The `lol_get_champion_analysis` tool (and others) does not return plain
//! JSON. It returns a token-efficient text block: a set of `class Name:
//! field1,field2,...` schema declarations followed by one nested
//! constructor-call expression, e.g. `Data(Summary(AverageStats(78023,0.49)))`.
//! This reconstructs that into a normal `serde_json::Value`, using the
//! schema to recover field names for each positional argument list.
//! Confirmed live against the real endpoint (2026-09) — not a published
//! format, so re-verify this parser if OP.GG changes their output shape.

use std::collections::HashMap;
use std::fmt;

use serde_json::Value;

#[derive(Debug)]
pub enum ParseError {
    NoSchema,
    UnexpectedChar(char),
    BadNumber(String),
    UnexpectedToken(String),
    UnexpectedEnd,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NoSchema => write!(f, "no class schema found in response"),
            ParseError::UnexpectedChar(c) => write!(f, "unexpected character '{c}'"),
            ParseError::BadNumber(s) => write!(f, "invalid number literal '{s}'"),
            ParseError::UnexpectedToken(s) => write!(f, "unexpected token: {s}"),
            ParseError::UnexpectedEnd => write!(f, "unexpected end of input"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Str(String),
    Num(serde_json::Number),
    True,
    False,
    Null,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
}

type Schema = HashMap<String, Vec<String>>;

pub fn parse_response(text: &str) -> Result<Value, ParseError> {
    let lines: Vec<&str> = text.lines().collect();
    let last_class_line = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with("class "))
        .ok_or(ParseError::NoSchema)?;

    let schema = parse_schema(&lines[..=last_class_line]);
    let body = lines[(last_class_line + 1)..].join("\n");
    let tokens = tokenize(body.trim())?;

    let mut pos = 0;
    parse_value(&tokens, &mut pos, &schema)
}

fn parse_schema(header_lines: &[&str]) -> Schema {
    let mut schema = Schema::new();
    for line in header_lines {
        let Some(rest) = line.trim_start().strip_prefix("class ") else {
            continue;
        };
        let Some((name, fields)) = rest.split_once(':') else {
            continue;
        };
        let fields = fields
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect();
        schema.insert(name.trim().to_string(), fields);
    }
    schema
}

fn tokenize(s: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = s.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '[' => {
                tokens.push(Token::LBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '"' => {
                i += 1;
                let mut buf = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        buf.push(match chars[i] {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    } else {
                        buf.push(chars[i]);
                    }
                    i += 1;
                }
                i += 1;
                tokens.push(Token::Str(buf));
            }
            c if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit())) =>
            {
                let start = i;
                i += 1;
                while i < chars.len()
                    && (chars[i].is_ascii_digit()
                        || matches!(chars[i], '.' | 'e' | 'E' | '-' | '+'))
                {
                    i += 1;
                }
                let raw: String = chars[start..i].iter().collect();
                let is_float = raw.contains(['.', 'e', 'E']);
                let num = if is_float {
                    raw.parse::<f64>()
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                } else {
                    raw.parse::<i64>().ok().map(serde_json::Number::from)
                }
                .ok_or_else(|| ParseError::BadNumber(raw.clone()))?;
                tokens.push(Token::Num(num));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                tokens.push(match word.as_str() {
                    "true" => Token::True,
                    "false" => Token::False,
                    "null" => Token::Null,
                    _ => Token::Ident(word),
                });
            }
            other => return Err(ParseError::UnexpectedChar(other)),
        }
    }
    Ok(tokens)
}

fn parse_value(tokens: &[Token], pos: &mut usize, schema: &Schema) -> Result<Value, ParseError> {
    match tokens.get(*pos) {
        Some(Token::LBracket) => {
            *pos += 1;
            let mut items = Vec::new();
            if tokens.get(*pos) != Some(&Token::RBracket) {
                loop {
                    items.push(parse_value(tokens, pos, schema)?);
                    if tokens.get(*pos) == Some(&Token::Comma) {
                        *pos += 1;
                    } else {
                        break;
                    }
                }
            }
            expect(tokens, pos, &Token::RBracket)?;
            Ok(Value::Array(items))
        }
        Some(Token::Ident(name)) if tokens.get(*pos + 1) == Some(&Token::LParen) => {
            let name = name.clone();
            *pos += 2;
            let fields = schema.get(&name).cloned().unwrap_or_default();
            let mut obj = serde_json::Map::new();
            let mut field_names = fields.into_iter();
            if tokens.get(*pos) != Some(&Token::RParen) {
                loop {
                    let field_name = field_names
                        .next()
                        .unwrap_or_else(|| format!("_arg{}", obj.len()));
                    let v = parse_value(tokens, pos, schema)?;
                    obj.insert(field_name, v);
                    if tokens.get(*pos) == Some(&Token::Comma) {
                        *pos += 1;
                    } else {
                        break;
                    }
                }
            }
            expect(tokens, pos, &Token::RParen)?;
            Ok(Value::Object(obj))
        }
        Some(Token::Str(s)) => {
            let v = Value::String(s.clone());
            *pos += 1;
            Ok(v)
        }
        Some(Token::Num(n)) => {
            let v = Value::Number(n.clone());
            *pos += 1;
            Ok(v)
        }
        Some(Token::True) => {
            *pos += 1;
            Ok(Value::Bool(true))
        }
        Some(Token::False) => {
            *pos += 1;
            Ok(Value::Bool(false))
        }
        Some(Token::Null) => {
            *pos += 1;
            Ok(Value::Null)
        }
        Some(Token::Ident(bare)) => {
            let v = Value::String(bare.clone());
            *pos += 1;
            Ok(v)
        }
        Some(other) => Err(ParseError::UnexpectedToken(format!("{other:?}"))),
        None => Err(ParseError::UnexpectedEnd),
    }
}

fn expect(tokens: &[Token], pos: &mut usize, expected: &Token) -> Result<(), ParseError> {
    if tokens.get(*pos) == Some(expected) {
        *pos += 1;
        Ok(())
    } else {
        Err(ParseError::UnexpectedToken(format!(
            "expected {expected:?}, got {:?}",
            tokens.get(*pos)
        )))
    }
}
