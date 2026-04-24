use std::path::PathBuf;

pub const DEFAULT_OUTPUT_HTML: &str = "parser-benchmark-table/parser_benchmark_table.html";
pub const REPO_CACHE_DIR: &str = "parser-benchmark-table/.cache/repos";
pub const CONJURE_REPO_URL: &str = "https://github.com/conjure-cp/conjure.git";
pub const ESSENCE_CATALOG_REPO_URL: &str = "https://github.com/conjure-cp/EssenceCatalog.git";

#[derive(Clone, Copy, Debug)]
pub enum ParserSelection {
    NativeOnly,
    ViaConjureOnly,
    Both,
}

#[derive(Clone, Copy, Debug)]
pub struct RepoSelection {
    pub conjure_oxide: bool,
    pub conjure: bool,
    pub essence_catalog: bool,
}

#[derive(Clone, Debug)]
pub struct InputGroup {
    pub repo_name: String,
    pub repo_root: PathBuf,
    pub primary_file: PathBuf,
    pub param_file: Option<PathBuf>,
    pub group_kind: &'static str,
}

#[derive(Clone, Debug)]
pub struct ParseResult {
    pub pass: bool,
    pub summary: &'static str,
    pub output_or_error: String,
}

#[derive(Clone, Debug)]
pub struct RowResult {
    pub repo_name: String,
    pub kind: &'static str,
    pub test_name: String,
    pub primary_relative: String,
    pub param_relative: String,
    pub primary_contents: String,
    pub param_contents: String,
    pub native: Option<ParseResult>,
    pub via_conjure: Option<ParseResult>,
}
