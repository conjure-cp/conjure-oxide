use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp_core::ast::Model;
use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::RecoverableParseError;
use tower_lsp::{lsp_types::Diagnostic as LspDiagnostic, lsp_types::*};

use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic as ParserDiagnostic;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Position as ParserPosition;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Range as ParserRange;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Severity as ParserSeverity;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use tower_lsp::lsp_types::Position as LspPosition;
use tower_lsp::lsp_types::Range as LspRange;

use moka::future::Cache;
use tree_sitter::Tree;

use crate::handlers::cache;
use crate::handlers::cache::CacheCont;
use crate::server::Backend;

// use tokio::fs;

impl Backend {
    pub async fn handle_did_open(&self, params: DidOpenTextDocumentParams) {
        //on open, check whether cache has existing entry, if not, add to cache

        // here's how i'd define the default values for arse_essence_with_context_and_map
        // probably needs to be moved somewhere else tho
        let context = Arc::new(RwLock::new(Context::default()));
        let mut errors: Vec<RecoverableParseError> = Vec::new();

        let uri = params.text_document.uri;
        let text = params.text_document.text.clone();

        let lsp_cache = &self.lsp_cache;
        //basically look to see if in cache and if not in cache, fetch from source
        //the closure? only runs on a cache miss
        let cache_content = lsp_cache
            .get_with(uri.clone(), async {
                self.client
                    .log_message(MessageType::INFO, "Cache miss! Loading into cache now")
                    .await;
                CacheCont {
                    sourcemap: None,       // need to get this using parse_essence_with_context_and_map
                    ast: Model::default(), // need to get this using parse_essence_with_context_and_map
                    cst: tree_sitter::Parser::new().parse(&text, None).unwrap(), // get this onOpen using tree-sitter directly, then send it to parse_essence_with_context_and_map to get sourcemap and ast
                    contents: text,
                    version: 0,
                }
            })
            .await;

        self.client
            .log_message(MessageType::INFO, "Did open document")
            .await;

        //diagnostic stuff here
        self.handle_diagnostics(&uri.clone(), cache_content).await;
    }
    pub async fn handle_did_save(&self, params: DidSaveTextDocumentParams) {
        //if save, do not update existing entry,simply access from cache
        let uri = params.text_document.uri;

        let lsp_cache = &self.lsp_cache;

        if let Some(lsp_cont) = lsp_cache.get(&uri).await {
            //CANNOT USE PRINTLNs AS THIS WILL BREAK CONNECTION WITH SERVER
            // println!("Found document version: {}", lsp_cont.version) //just for proof of concept
            self.client
                .log_message(MessageType::INFO, "Did save document")
                .await;
            self.handle_diagnostics(&uri, lsp_cont);
        }
    }
    pub async fn handle_did_change(&self, params: DidChangeTextDocumentParams) {
        //on change, take change and range of change
        //modify existing document given uri and cache content to update the document version in cache
        //check whether changes are purely whitespace
        //if changes are purely whitespace, check whether they

        let uri = params.text_document.uri;
        let lsp_cache = &self.lsp_cache;

        if let Some(cache_conts) = lsp_cache.get(&uri).await {}

        //need to check versions against each other, update version in
        //cache, check what the changes were

        // if let Some(change) = params.content_changes.first() {
        //     let text = change.text.clone();

        //     self.client
        //         .log_message(MessageType::INFO, format!("New text: {}", text))
        //         .await;

        //     self.documents
        //         .write()
        //         .await
        //         .insert(uri.to_string().clone(), text.clone());

        //     //diagnostic stuff here
        //     self.handle_diagnostics(&uri.clone(), text.clone()).await;
        // }
    }

    // pub async fn handle_diagnostics(&self, uri: &Url, code: String) {
    pub async fn handle_diagnostics(&self, uri: &Url, cache_conts: CacheCont) {
        //needs to be modified to use cst and ast from cache
        //using lsp_cache.get(&uri) assumedly and then feeding
        //these values back to the diagnostic thingy to get my diags

        //ideal situation is feed diagnostics struct and then let it use struct to return diagnostics
        // e.g.:
        //let diagnostics = get_diagnostics(&cache_conts);
        let diagnostics = get_diagnostics(&cache_conts.contents); //temp
        let lsp_diagnostics = convert_diagnostics(diagnostics);

        // Publish diagnostics back to the client
        self.client
            .publish_diagnostics(uri.clone(), lsp_diagnostics, None)
            .await;
    }
}

// convert diagnostics from cp-essence-parser to LSP diagnostics
pub fn convert_diagnostics(diagnostics: Vec<ParserDiagnostic>) -> Vec<LspDiagnostic> {
    // map each ParserDiagnostic to LspDiagnostic
    diagnostics
        .into_iter()
        .map(|diag| {
            LspDiagnostic {
                range: parser_to_lsp_range(diag.range),
                severity: match diag.severity {
                    ParserSeverity::Error => Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
                    ParserSeverity::Warn => Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                    ParserSeverity::Info => {
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION)
                    }
                    ParserSeverity::Hint => Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT),
                },
                code: None,             // for now
                code_description: None, // also for now
                source: Some(diag.source.to_string()),
                message: diag.message,
                related_information: None,
                tags: None,
                data: None,
            }
        })
        .collect()
}

// playing that position converts properly
pub fn parser_to_lsp_range(range: ParserRange) -> LspRange {
    LspRange {
        start: parser_to_lsp_position(range.start),
        end: parser_to_lsp_position(range.end),
    }
}

pub fn parser_to_lsp_position(position: ParserPosition) -> LspPosition {
    LspPosition {
        line: position.line,
        character: position.character,
    }
}
