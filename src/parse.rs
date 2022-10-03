use std::env;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use ansi_term::Color;
use anyhow::anyhow;
use anyhow::Context as _;
use difference::Changeset;
use difference::Difference;

pub(crate) struct Test<'a> {
    pub(crate) path: &'a Path,
    pub(crate) input: &'a str,
    pub(crate) table: &'a str,
    pub(crate) tree: &'a str,
}

pub(crate) fn grade<P: AsRef<Path>>(
    workspace: P,
    verbose: bool,
    tests: &[Test],
) -> anyhow::Result<()> {
    let student = workspace.as_ref().file_name().unwrap();

    println!(
        "[{}] grading in workspace {}...",
        student.to_string_lossy(),
        workspace.as_ref().display()
    );

    env::set_current_dir(&workspace)?;

    match Command::new("make")
        .arg("parser")
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
        .context("Could not execute `make parser`")
    {
        Ok(_) => (),
        Err(error) => {
            println!("{}", error);
        }
    }

    let mut failures = 0;

    for test in tests {
        let differences = grade_test(test)
            .with_context(|| anyhow!("Failed to grade test {}", test.path.display()))?;
        let name = test.path.file_name().unwrap().to_string_lossy();

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
    Test {
        path: _,
        input,
        table,
        tree,
    }: &Test,
) -> anyhow::Result<Vec<Difference>> {
    let mut child = Command::new("./parser")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;

    let stdout = child.wait_with_output()?.stdout;
    let actual = String::from_utf8_lossy(&stdout);
    let mut differences = Vec::new();

    let actual_table = actual
        .find("Symbol table level 1")
        .and_then(|index| actual.get(index..))
        .ok_or_else(|| anyhow!("No symbol table found"))?
        .lines()
        .skip(1)
        .take_while(|line| line.trim().starts_with(|char: char| char.is_numeric()))
        .collect::<String>();

    differences.append(&mut Changeset::new(table, &actual_table, "\n").diffs);

    let actual_tree = actual
        .find("(program")
        .and_then(|index| actual.get(index..))
        .ok_or_else(|| anyhow!("No AST found"))?
        .trim();

    differences.append(&mut Changeset::new(tree, actual_tree, "\n").diffs);

    Ok(differences)
}
