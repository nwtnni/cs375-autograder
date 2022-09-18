use std::iter;
use std::mem;
use std::path::Path;
use std::str;

use anyhow::anyhow;
use anyhow::Context as _;
use include_dir::include_dir;
use include_dir::Dir;

use crate::lex;

static EXPECTEDS: Dir = include_dir!("$CARGO_MANIFEST_DIR/sample_p1");

pub fn grade<P: AsRef<Path>>(workspace: P, verbose: bool) -> anyhow::Result<()> {
    let mut overflow = None;

    let different = move |expecteds: &mut iter::Peekable<str::Split<char>>,
                          actuals: &mut iter::Peekable<str::Split<char>>| {
        let expected = expecteds
            .next()
            .expect("[INTERNAL ERROR]: caller guarantees non-None");

        let actual = actuals
            .next()
            .expect("[INTERNAL ERROR]: caller guarantees non-None");

        let expected_token = parse(expected).with_context(|| {
            anyhow!(
                "[INTERNAL ERROR]: failed to parse expected token: {}",
                expected
            )
        })?;

        let different = match (expected_token, parse(actual)) {
            (Token::Overflow(expected), Some(Token::Overflow(actual))) => {
                if expected == actual {
                    overflow = Some(expected);
                    false
                } else {
                    true
                }
            }
            (Token::Overflow(_), _) => {
                expecteds.next();
                true
            }
            (_, Some(Token::Overflow(_))) => {
                actuals.next();
                true
            }
            (expected, Some(actual)) => !expected.equals(&actual, mem::take(&mut overflow)),
            (_, None) => true,
        };

        Ok(different)
    };

    lex::grade(workspace, verbose, "lexanc", &EXPECTEDS, different)
}

#[derive(Debug, PartialEq)]
enum Token {
    Start,
    Overflow(Overflow),
    Operator(u16),
    Delimiter(u16),
    Reserved(u16),
    Identifier(String),
    String(String),
    Number(Number),
}

impl Token {
    fn equals(&self, other: &Token, overflow: Option<Overflow>) -> bool {
        match (self, other) {
            (Token::Number(left), Token::Number(right)) => left.equals(right, overflow),
            _ => self == other,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Overflow {
    Float,
    Integer,
}

#[derive(Debug)]
enum Number {
    Float { mantissa: f32, exponent: i32 },
    Integer(i32),
}

impl Number {
    fn equals(&self, other: &Number, overflow: Option<Overflow>) -> bool {
        match (self, other, overflow) {
            (Number::Float { .. }, Number::Float { .. }, Some(Overflow::Float)) => true,
            (Number::Integer(_), Number::Integer(_), Some(Overflow::Integer)) => true,
            _ => self == other,
        }
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Float {
                    mantissa: left_mantissa,
                    exponent: left_exponent,
                },
                Self::Float {
                    mantissa: right_mantissa,
                    exponent: right_exponent,
                },
            ) => {
                left_exponent == right_exponent
                    && (left_mantissa - right_mantissa).abs() <= 0.000_001
            }
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (_, _) => false,
        }
    }
}

fn parse(line: &str) -> Option<Token> {
    match &*line.to_ascii_lowercase() {
        "started scanner test." => return Some(Token::Start),
        line if line.contains("float")
            && (line.contains("out of range") || line.contains("overflow")) =>
        {
            return Some(Token::Overflow(Overflow::Float))
        }
        line if line.contains("int")
            && (line.contains("out of range") || line.contains("overflow")) =>
        {
            return Some(Token::Overflow(Overflow::Integer))
        }
        _ => (),
    }

    let (_, line) = line.split_once(':')?;
    let (r#type, line) = line.trim_start().split_once(' ')?;
    match r#type {
        "0" => line
            .split_whitespace()
            .nth(1)?
            .parse::<u16>()
            .ok()
            .map(Token::Operator),
        "1" => line
            .split_whitespace()
            .nth(1)?
            .parse::<u16>()
            .ok()
            .map(Token::Delimiter),
        "2" => line
            .split_whitespace()
            .nth(1)?
            .parse::<u16>()
            .ok()
            .map(Token::Reserved),
        "3" => Some(Token::Identifier(line.to_string())),
        "4" => Some(Token::String(line.to_string())),
        "5" => {
            let mut iter = line.split_whitespace();

            match iter.nth(1)? {
                "0" => iter
                    .next()?
                    .parse()
                    .ok()
                    .map(Number::Integer)
                    .map(Token::Number),
                "1" => {
                    let value = iter.next()?;
                    value.parse::<f32>().ok()?;
                    let (mantissa, exponent) = value.split_once('e')?;
                    Some(Token::Number(Number::Float {
                        mantissa: mantissa.parse().ok()?,
                        exponent: exponent.parse().ok()?,
                    }))
                }
                _ => None,
            }
        }
        r#type => {
            println!("Unknown token type: {}", r#type);
            None
        }
    }
}
