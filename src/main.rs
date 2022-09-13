mod p1;

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
        verbose: bool,

        #[clap(short, long)]
        workspace: PathBuf,
    },
}

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
            verbose,
        } => {
            p1::grade(&workspace, verbose)?;
        }
    }

    Ok(())
}
