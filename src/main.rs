use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use clap::Parser;
use zip::read::ZipArchive;

use cs375_autograder::p1;
use cs375_autograder::p2;
use cs375_autograder::p3;
use cs375_autograder::p4;

#[derive(Parser)]
#[clap(about)]
enum Command {
    Prepare {
        /// ZIP file containing student submissions.
        #[clap(long, default_value = "submissions.zip", use_delimiter = true)]
        submissions: Vec<PathBuf>,

        /// Directory of skeleton (template) code to compile student code against.
        #[clap(long, default_value = "cs375_minimal")]
        skeleton: PathBuf,

        /// Directory to output unzipped student code, with the skeleton code.
        workspace: PathBuf,
    },

    /// Grade a submission (or multiple submissions).
    Grade {
        /// Project to grade (one of p1, p2, ..., p6).
        #[clap(short, long)]
        project: Project,

        #[clap(short, long)]
        verbose: bool,

        workspaces: Vec<PathBuf>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Project {
    /// Lexer (lexanc.c)
    P1,

    /// Lex (lexan.l)
    P2,

    /// Parse (trivb.pas)
    P3,

    /// Parse (graph1.pas)
    P4,
}

impl FromStr for Project {
    type Err = anyhow::Error;
    fn from_str(project: &str) -> Result<Self, Self::Err> {
        match project {
            "1" | "p1" | "P1" => Ok(Project::P1),
            "2" | "p2" | "P2" => Ok(Project::P2),
            "3" | "p3" | "P3" => Ok(Project::P3),
            "4" | "p4" | "P4" => Ok(Project::P4),
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
            let mut archives = Vec::new();
            let mut students = BTreeMap::default();

            for (index, archive) in submissions.iter().enumerate() {
                let archive = File::open(&archive)
                    .map(BufReader::new)
                    .map(ZipArchive::new)??;

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
                            .or_insert_with(BTreeMap::new)
                            .insert(name, (index, String::from(path)));
                    });

                archives.push(archive);
            }

            let skeletons = Path::new(&skeleton)
                .canonicalize()?
                .read_dir()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>();

            for (student, paths) in &students {
                workspace.push(student);
                fs::create_dir(&workspace)?;

                for skeleton in &skeletons {
                    workspace.push(skeleton.file_name().unwrap());
                    fs::copy(&skeleton, &workspace)?;
                    workspace.pop();
                }

                for (name, (index, path)) in paths {
                    workspace.push(name);

                    eprintln!(
                        "[{}]: copying submission {}/{} to {}",
                        student,
                        submissions[*index].display(),
                        path,
                        workspace.display()
                    );

                    std::io::copy(
                        &mut archives[*index].by_name(path)?,
                        &mut fs::File::create(&workspace).map(BufWriter::new)?,
                    )?;

                    workspace.pop();
                }

                workspace.pop();
            }
        }

        Command::Grade {
            workspaces,
            project,
            verbose,
        } => {
            for workspace in workspaces {
                match match project {
                    Project::P1 => p1::grade(&workspace, verbose),
                    Project::P2 => p2::grade(&workspace, verbose),
                    Project::P3 => p3::grade(&workspace, verbose),
                    Project::P4 => p4::grade(&workspace, verbose),
                } {
                    Ok(()) => (),
                    Err(error) => {
                        eprintln!("Error grading workspace: {}", workspace.display());
                        eprintln!("{:?}", error);
                    }
                }
            }
        }
    }

    Ok(())
}
