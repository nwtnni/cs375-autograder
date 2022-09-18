use std::iter;
use std::path::Path;
use std::str;

use include_dir::include_dir;
use include_dir::Dir;

use crate::lex;

static EXPECTEDS: Dir = include_dir!("$CARGO_MANIFEST_DIR/sample_p2");

pub fn grade<P: AsRef<Path>>(workspace: P, verbose: bool) -> anyhow::Result<()> {
    let different = move |expecteds: &mut iter::Peekable<str::Split<char>>,
                          actuals: &mut iter::Peekable<str::Split<char>>| {
        let expected = expecteds
            .next()
            .expect("[INTERNAL ERROR]: caller guarantees non-None");

        let actual = actuals
            .next()
            .expect("[INTERNAL ERROR]: caller guarantees non-None");

        Ok(expected != actual)
    };

    lex::grade(workspace, verbose, "lexer", &EXPECTEDS, different)
}
