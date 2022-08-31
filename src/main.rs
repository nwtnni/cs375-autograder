use std::str::FromStr;

use anyhow::anyhow;
use clap::Parser;

#[derive(Parser)]
#[clap(about)]
struct Command {
    /// Project to grade (one of p1, p2, ..., p6)
    project: Project,
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

fn main() {
    let command = Command::parse();

    match command.project {
        Project::P1 => todo!(),
    }
}
