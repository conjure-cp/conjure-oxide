use moka::future::Cache;
use std::time::Duration;
use conjure_cp_core::ast::Model;
use tree_sitter::Tree;
use tower_lsp::lsp_types::*;


#[derive(Clone,Debug)]
pub struct CacheCont {
    //sourcemap
    pub ast: Model,
    pub cst: Tree,
    pub contents: String,
    //from DidChangeTextDocumentParams -> Versioned thingy -> version
    pub version: i32, //therefore can do dirty clean with version checking? which allows direct comparison
}

pub async fn create_cache() -> Cache<Url, CacheCont> {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(30 * 60))
        .time_to_idle(Duration::from_secs(5 * 60))
        .eviction_listener(|key, _value, cause| {
            println!("Evicted document {} - cause {:?}", key, cause);
        })
        .build()
}