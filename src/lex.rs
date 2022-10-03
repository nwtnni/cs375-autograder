use std::env;
use std::io::Write as _;
use std::iter;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::str;

use ansi_term::Color;
use anyhow::anyhow;
use anyhow::Context as _;
use difference::Changeset;
use difference::Difference;
use include_dir::include_dir;
use include_dir::Dir;

static TESTS: Dir = include_dir!("$CARGO_MANIFEST_DIR/test_p1");

pub fn grade<P: AsRef<Path>, F>(
    workspace: P,
    verbose: bool,
    target: &str,
    expecteds: &Dir,
    mut different: F,
) -> anyhow::Result<()>
where
    F: FnMut(
        &mut iter::Peekable<str::Split<char>>,
        &mut iter::Peekable<str::Split<char>>,
    ) -> anyhow::Result<bool>,
{
    let student = workspace.as_ref().file_name().unwrap();

    println!(
        "[{}] grading in workspace {}...",
        student.to_string_lossy(),
        workspace.as_ref().display()
    );

    env::set_current_dir(&workspace)?;

    let mut tests = TESTS.files().collect::<Vec<_>>();
    let mut expecteds = expecteds.files().collect::<Vec<_>>();

    tests.sort_by_key(|file| file.path().file_name().unwrap());
    expecteds.sort_by_key(|file| file.path().file_name().unwrap());

    match Command::new("make")
        .arg(target)
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
        .with_context(|| anyhow!("Could not execute `make {}`", target))
    {
        Ok(_) => (),
        Err(error) => {
            println!("{}", error);
        }
    }

    let mut failures = 0;

    for (test, expected) in tests.iter().zip(&expecteds) {
        let differences = grade_test(target, test, expected, &mut different)
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

fn grade_test<F>(
    target: &str,
    test: &include_dir::File,
    expected: &include_dir::File,
    mut different: F,
) -> anyhow::Result<Vec<Difference>>
where
    F: FnMut(
        &mut iter::Peekable<str::Split<char>>,
        &mut iter::Peekable<str::Split<char>>,
    ) -> anyhow::Result<bool>,
{
    let mut child = Command::new(format!("./{}", target))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(test.contents())?;

    let expected = expected.contents_utf8().unwrap_or_default();
    let stdout = child.wait_with_output()?.stdout;
    let actual = String::from_utf8_lossy(&stdout);

    if actual == expected {
        return Ok(Vec::new());
    }

    let mut differences = Vec::new();
    let mut expecteds = expected.trim_end_matches('\n').split('\n').peekable();
    let mut actuals = actual.trim_end_matches('\n').split('\n').peekable();

    while let (Some(expected), Some(actual)) = (expecteds.peek().copied(), actuals.peek().copied())
    {
        if different(&mut expecteds, &mut actuals)? {
            differences.append(&mut Changeset::new(expected, actual, "\n").diffs);
        }
    }

    for expected in expecteds {
        differences.append(&mut Changeset::new(expected, "", "\n").diffs);
    }

    for actual in actuals {
        differences.append(&mut Changeset::new("", actual, "\n").diffs);
    }

    Ok(differences)
}
