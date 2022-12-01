use std::path::Path;

use crate::parse;

pub fn grade<P: AsRef<Path>>(workspace: P, verbose: bool) -> anyhow::Result<()> {
    parse::grade(
        workspace,
        verbose,
        &[parse::Test {
            path: Path::new("cs375_minimal/graph1i.pas"),
            input: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/cs375_minimal/graph1i.pas"
            )),
            table: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sample_symtab/graph1_table.txt"
            )),
            tree: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sample_trees/graph1i.sample"
            )),
        }],
    )
}
