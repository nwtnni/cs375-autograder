use std::path::Path;

use crate::parse;

pub fn grade<P: AsRef<Path>>(workspace: P, verbose: bool) -> anyhow::Result<()> {
    parse::grade(
        workspace,
        verbose,
        &[parse::Test {
            path: Path::new("cs375_minimal/trivb.pas"),
            input: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/cs375_minimal/trivb.pas"
            )),
            table: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sample_symtab/trivb_table.txt"
            )),
            tree: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sample_trees/trivb.sample"
            )),
        }],
    )
}
