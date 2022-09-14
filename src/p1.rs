use std::env;
use std::io::Write as _;
use std::mem;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use ansi_term::Color;
use anyhow::anyhow;
use anyhow::Context as _;
use difference::Changeset;
use difference::Difference;
use include_dir::include_dir;
use include_dir::Dir;

static TESTS: Dir = include_dir!("$CARGO_MANIFEST_DIR/test_p1");
static EXPECTEDS: Dir = include_dir!("$CARGO_MANIFEST_DIR/sample_p1");

pub fn grade<P: AsRef<Path>>(workspace: P, verbose: bool) -> anyhow::Result<()> {
    let student = workspace.as_ref().file_name().unwrap();

    println!(
        "[{}] grading in workspace {}...",
        student.to_string_lossy(),
        workspace.as_ref().display()
    );

    env::set_current_dir(&workspace)?;

    let mut tests = TESTS.files().collect::<Vec<_>>();
    let mut expecteds = EXPECTEDS.files().collect::<Vec<_>>();

    tests.sort_by_key(|file| file.path().file_name().unwrap());
    expecteds.sort_by_key(|file| file.path().file_name().unwrap());

    match Command::new("make")
        .arg("lexanc")
        .spawn()
        .context("Could not execute `make`")?
        .wait()
        .map_err(anyhow::Error::new)
        .and_then(|status| {
            if status.success() {
                Ok(status)
            } else {
                Err(anyhow!(status))
            }
        })
        .context("Could not execute `make lexanc`")
    {
        Ok(_) => (),
        Err(error) => {
            println!("{}", error);
        }
    }

    let mut failures = 0;

    for (test, expected) in tests.iter().zip(&expecteds) {
        let differences = grade_test(test, expected)
            .with_context(|| anyhow!("Failed to grade test {}", test.path().display()))?;
        let name = test.path().file_name().unwrap().to_string_lossy();

        match differences.is_empty() {
            true if verbose => println!("- [{}]: pass", name),
            true => (),
            false => {
                println!("- [{}]: fail", name);
                failures += 1;
            }
        }

        for difference in differences {
            match difference {
                difference::Difference::Same(_) => (),
                difference::Difference::Add(added) => {
                    print!("{}", Color::Green.paint("+ "));
                    println!("{}", Color::Green.paint(added));
                }
                difference::Difference::Rem(removed) => {
                    print!("{}", Color::Red.paint("- "));
                    println!("{}", Color::Red.paint(removed));
                }
            }
        }
    }

    println!(
        "{}",
        Color::Blue.paint(format!(
            "[{}]: passed {} out of {}",
            student.to_string_lossy(),
            tests.len() - failures,
            tests.len()
        ))
    );

    Ok(())
}

fn grade_test(
    test: &include_dir::File,
    expected: &include_dir::File,
) -> anyhow::Result<Vec<Difference>> {
    let mut child = Command::new("./lexanc")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(test.contents())?;

    let stdout = child.wait_with_output()?.stdout;
    let actual = String::from_utf8_lossy(&stdout);
    let expected = expected.contents_utf8().unwrap_or_default();

    if actual == expected {
        return Ok(Vec::new());
    }

    let mut differences = Vec::new();
    let mut overflow = None;

    let mut actuals = actual.trim_end_matches('\n').split('\n').peekable();
    let mut expecteds = expected.trim_end_matches('\n').split('\n').peekable();

    while let (Some(actual), Some(expected)) = (actuals.peek().copied(), expecteds.peek().copied())
    {
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

        if different {
            differences.append(&mut Changeset::new(expected, actual, "\n").diffs);
        }

        actuals.next();
        expecteds.next();
    }

    for actual in actuals {
        differences.append(&mut Changeset::new("", actual, "\n").diffs);
    }

    for expected in expecteds {
        differences.append(&mut Changeset::new(expected, "", "\n").diffs);
    }

    Ok(differences)
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
