use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::process;
use std::process::Stdio;
use std::str::FromStr;

use ansi_term::Color;
use anyhow::anyhow;
use anyhow::Context;
use clap::Parser;
use difference::Changeset;
use include_dir::include_dir;
use include_dir::Dir;
use tempdir::TempDir;
use zip::read::ZipArchive;

#[derive(Parser)]
#[clap(about)]
struct Command {
    /// Project to grade (one of p1, p2, ..., p6)
    project: Project,
}

static P1_TESTS: Dir = include_dir!("$CARGO_MANIFEST_DIR/test_p1");
static P1_OUTPUTS: Dir = include_dir!("$CARGO_MANIFEST_DIR/sample_p1");

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Project {
    /// Lexer (lexanc.c)
    P1,
}

impl FromStr for Project {
    type Err = anyhow::Error;
    fn from_str(project: &str) -> Result<Self, Self::Err> {
        match project {
            "1" | "p1" | "P1" => Ok(Project::P1),
            _ => Err(anyhow!("Invalid project `{}`", project)),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let command = Command::parse();

    match command.project {
        Project::P1 => {
            let workspace = TempDir::new("cs375-p1-workspace")?;
            let submissions = TempDir::new("cs375-p1-submissions")?;

            let mut archive = File::open("submissions.zip")
                .map(BufReader::new)
                .map(ZipArchive::new)??;

            archive.extract(&submissions)?;

            for entry in Path::new("cs375_minimal")
                .read_dir()?
                .filter_map(Result::ok)
            {
                assert!(entry.file_type()?.is_file());
                let original = entry.path();
                let temporary = workspace.path().join(original.file_name().unwrap());
                fs::copy(original, temporary)?;
            }

            env::set_current_dir(&workspace)?;

            let mut tests = P1_TESTS.files().collect::<Vec<_>>();
            let mut outputs = P1_OUTPUTS.files().collect::<Vec<_>>();

            tests.sort_by_key(|file| file.path().file_name().unwrap());
            outputs.sort_by_key(|file| file.path().file_name().unwrap());

            for submission in archive
                .file_names()
                .map(String::from)
                .filter(|name| name.ends_with(".c"))
            {
                eprintln!("[{}]", submission);

                fs::copy(
                    submissions.path().join(&submission),
                    workspace.path().join("lexanc.c"),
                )?;

                let header = format!("{}.h", submission.trim_end_matches(".c"));

                fs::remove_file(workspace.path().join("lexanc.h")).ok();
                fs::copy(
                    submissions.path().join(&header),
                    workspace.path().join("lexanc.h"),
                )
                .ok();

                match process::Command::new("make")
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
                        continue;
                    }
                }

                for (test, output) in tests.iter().zip(&outputs) {
                    print!(
                        "- [{}]:",
                        test.path().file_name().unwrap().to_string_lossy()
                    );

                    let mut child = process::Command::new(workspace.path().join("lexanc"))
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::inherit())
                        .spawn()?;

                    child.stdin.as_mut().unwrap().write_all(test.contents())?;
                    let stdout = child.wait_with_output()?.stdout;
                    let actual = String::from_utf8_lossy(&stdout);
                    let actual = actual.trim();
                    let expected = output.contents_utf8().unwrap_or_default().trim();

                    if actual == expected {
                        println!(" pass");
                        continue;
                    } else {
                        println!(" fail");
                    }

                    for diff in Changeset::new(expected, actual, "\n").diffs {
                        match diff {
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
            }
        }
    }

    Ok(())
}
