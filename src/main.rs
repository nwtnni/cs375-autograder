use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
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
use zip::read::ZipArchive;

#[derive(Parser)]
#[clap(about)]
enum Command {
    Prepare {
        /// ZIP file containing student submissions.
        #[clap(long, default_value = "submissions.zip")]
        submissions: PathBuf,

        /// Directory of skeleton (template) code to compile student code against.
        #[clap(long, default_value = "cs375_minimal")]
        skeleton: PathBuf,

        /// Directory to output unzipped student code, with the skeleton code.
        #[clap(short, long)]
        workspace: PathBuf,
    },

    /// Grade a submission (or multiple submissions).
    Grade {
        /// Project to grade (one of p1, p2, ..., p6).
        #[clap(short, long)]
        project: Project,

        #[clap(short, long)]
        workspace: PathBuf,
    },
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

    match command {
        Command::Prepare {
            submissions,
            skeleton,
            workspace,
        } => {
            fs::create_dir_all(&workspace)?;

            let mut workspace = workspace.canonicalize()?;
            let mut archive = File::open(&submissions)
                .map(BufReader::new)
                .map(ZipArchive::new)??;

            let mut students = BTreeMap::default();

            archive
                .file_names()
                .filter_map(|path| {
                    let (student, _) = path.split_once('_')?;
                    let (name, extension) = path.rsplit_once('.')?;
                    let (_, name) = name
                        .trim_end_matches(|char| matches!(char, '-' | '0'..='9'))
                        .rsplit_once('_')?;

                    Some((student, path, format!("{}.{}", name, extension)))
                })
                .for_each(|(student, path, name)| {
                    students
                        .entry(String::from(student))
                        .or_insert_with(Vec::new)
                        .push((String::from(path), name))
                });

            let skeletons = Path::new(&skeleton)
                .canonicalize()?
                .read_dir()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>();

            for (student, paths) in &students {
                workspace.push(student);
                fs::create_dir(&workspace)?;

                for (path, name) in paths {
                    workspace.push(name);

                    eprintln!(
                        "[{}]: copying submission {} to {}",
                        student,
                        path,
                        workspace.display()
                    );

                    std::io::copy(
                        &mut archive.by_name(path)?,
                        &mut fs::File::create(&workspace).map(BufWriter::new)?,
                    )?;

                    workspace.pop();
                }

                for skeleton in &skeletons {
                    workspace.push(skeleton.file_name().unwrap());
                    fs::copy(&skeleton, &workspace)?;
                    workspace.pop();
                }

                workspace.pop();
            }
        }

        Command::Grade {
            workspace,
            project: Project::P1,
        } => {
            env::set_current_dir(&workspace)?;

            let mut tests = P1_TESTS.files().collect::<Vec<_>>();
            let mut outputs = P1_OUTPUTS.files().collect::<Vec<_>>();

            tests.sort_by_key(|file| file.path().file_name().unwrap());
            outputs.sort_by_key(|file| file.path().file_name().unwrap());

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
                }
            }

            for (test, output) in tests.iter().zip(&outputs) {
                print!(
                    "- [{}]:",
                    test.path().file_name().unwrap().to_string_lossy()
                );

                let mut child = process::Command::new(workspace.join("lexanc"))
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

    Ok(())
}
