use conjure_cp_core::ast::Model;
use conjure_cp_essence_parser::{RecoverableParseError, diagnostics::source_map::SourceMap};
use moka::future::Cache;
use std::time::Duration;
use tower_lsp::lsp_types::*;
use tree_sitter::Tree;


#[derive(Clone, Debug)]
pub struct CacheCont {
    pub sourcemap: Option<SourceMap>,
    pub ast: Option<Model>,
    pub errors: Vec<RecoverableParseError>,
    pub cst: Option<Tree>,
    pub contents: String,
    //from DidChangeTextDocumentParams -> Versioned thingy -> version
    pub version: i32, //therefore can do dirty clean with version checking? which allows direct comparison
}

//create cache which will be used throughout lsp
pub async fn create_cache() -> Cache<Url, CacheCont> {
    Cache::builder()
        .max_capacity(10_000) //cache has a set upper size limit before eviction
        .time_to_live(Duration::from_secs(30 * 60)) //documents remain in cache while used for 30 minutes
        .time_to_idle(Duration::from_secs(5 * 60)) //documents remain in cache while idle for 5 minutes
        .build()
}
