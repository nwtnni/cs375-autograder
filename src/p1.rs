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

pub fn grade<P: AsRef<Path>>(workspace: P) -> anyhow::Result<()> {
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

    for (test, expected) in tests.iter().zip(&expecteds) {
        print!(
            "- [{}]:",
            test.path().file_name().unwrap().to_string_lossy()
        );

        let differences = grade_test(test, expected)?;

        match differences.is_empty() {
            true => println!(" {}", Color::Green.paint("pass")),
            false => println!(" {}", Color::Red.paint("fail")),
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
    let mut overflow = false;

    for (actual, expected) in actual.trim().split('\n').zip(expected.trim().split('\n')) {
        const STARTED: &str = "Started scanner test";
        const OVERFLOW: &str = "out of range";

        if actual.contains(STARTED) && expected.contains(STARTED) {
            continue;
        }

        if actual.contains(OVERFLOW) && expected.contains(OVERFLOW) {
            overflow = true;
            continue;
        }

        let expected_token = parse(expected).with_context(|| {
            anyhow!(
                "[INTERNAL ERROR]: failed to parse expected token: {}",
                expected
            )
        })?;

        match parse(actual) {
            Some(actual_token)
                if actual_token.equals(&expected_token, mem::take(&mut overflow)) => {}
            _ => differences.append(&mut Changeset::new(expected, actual, "\n").diffs),
        };
    }

    Ok(differences)
}

#[derive(PartialEq)]
enum Token {
    Operator(u16),
    Delimiter(u16),
    Reserved(u16),
    Identifier(String),
    String(String),
    Number(Number),
}

impl Token {
    fn equals(&self, other: &Token, overflow: bool) -> bool {
        match (self, other) {
            (Token::Number(left), Token::Number(right)) => left.equals(right, overflow),
            _ => self == other,
        }
    }
}

enum Number {
    Float { mantissa: f32, exponent: i32 },
    Integer(i32),
}

impl Number {
    fn equals(&self, other: &Number, overflow: bool) -> bool {
        match (self, other, overflow) {
            (Number::Float { .. }, Number::Float { .. }, true) => true,
            (Number::Integer(_), Number::Integer(_), true) => true,
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
    let mut iter = line.split_whitespace().skip(1);

    let token = match iter.next()? {
        "0" => Token::Operator(iter.nth(1)?.parse::<u16>().ok()?),
        "1" => Token::Delimiter(iter.nth(1)?.parse::<u16>().ok()?),
        "2" => Token::Reserved(iter.nth(1)?.parse::<u16>().ok()?),
        "3" => Token::Identifier(iter.nth(1)?.to_string()),
        "4" => Token::String(iter.nth(1)?.to_string()),
        "5" => match iter.by_ref().nth(1)? {
            "0" => Token::Number(Number::Integer(iter.next()?.parse().ok()?)),
            "1" => {
                let value = iter.next()?;
                value.parse::<f32>().ok()?;
                let (mantissa, exponent) = value.split_once('e')?;
                Token::Number(Number::Float {
                    mantissa: mantissa.parse().ok()?,
                    exponent: exponent.parse().ok()?,
                })
            }
            _ => return None,
        },
        _ => return None,
    };

    Some(token)
}
