use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::RecoverableParseError;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::error_to_diagnostic;
use conjure_cp_essence_parser::diagnostics::source_map;
use conjure_cp_essence_parser::parse_essence_with_context_and_map;
use conjure_cp_essence_parser::util::get_tree;
use tower_lsp::{lsp_types::Diagnostic as LspDiagnostic, lsp_types::*};

use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic as ParserDiagnostic;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Position as ParserPosition;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Range as ParserRange;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Severity as ParserSeverity;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use tower_lsp::lsp_types::Position as LspPosition;
use tower_lsp::lsp_types::Range as LspRange;

// use moka::future::Cache;
// use tree_sitter::InputEdit;
use tree_sitter::Point;
use tree_sitter::Tree;

use crate::handlers::cache;
// use crate::handlers::cache;
use crate::handlers::cache::CacheCont;
use crate::server::Backend;

// use tokio::fs;

impl Backend {
    pub async fn handle_did_open(&self, params: DidOpenTextDocumentParams) {
        //on open, check whether cache has existing entry, if not, add to cache

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

                let (cst_tree, _) = get_tree(&text).unwrap();

                let context = Arc::new(RwLock::new(Context::default()));
                let mut errors: Vec<RecoverableParseError> = Vec::new();

                let parsed = parse_essence_with_context_and_map(
                    &text,
                    context,
                    &mut errors,
                    Some(&cst_tree),
                );

                let mut cache = match parsed {
                    Ok(Some((ast_model, source_map))) => {
                        CacheCont {
                            sourcemap: Some(source_map),
                            ast: Some(ast_model),
                            errors,
                            cst: Some(new_tree),
                            contents: new_text.clone(),
                            version: params.text_document.version,
                        }
                    }
                    Ok(None) => {
                        CacheCont { 
                            sourcemap: None,
                            ast: None, 
                            errors, 
                            cst: Some(new_tree), 
                            contents: new_text.clone(), 
                            version: params.text_document.version, 
                        }
                    }
                    Err(fatal) => {
                        CacheCont {
                            sourcemap: None,
                            ast: None,
                            errors: vec![RecoverableParseError::new(fatal.to_string(), None)],
                            cst: Some(new_tree),
                            contents: new_text.clone(),
                            version: params.text_document.version,
                        }
                    }
                };

                cache
            })
            .await;

        self.client
            .log_message(MessageType::INFO, "Did open document")
            .await;

        //diagnostic stuff here
        self.handle_diagnostics(&uri.clone(), new_cache).await;
    }
    pub async fn handle_did_save(&self, params: DidSaveTextDocumentParams) {
        //if save, do not update existing entry,simply access from cache
        let uri = params.text_document.uri;
        let lsp_cache = &self.lsp_cache;

        if let Some(lsp_cont) = lsp_cache.get(&uri).await {
            //CANNOT USE PRINTLNs AS THIS WILL BREAK CONNECTION WITH SERVER

            //check versioning? might modify for dirty clean later cos i dont fw current situ

            self.client
                .log_message(MessageType::INFO, "Did save document")
                .await;
            self.handle_diagnostics(&uri, lsp_cont).await;
        }
    }
    pub async fn handle_did_change(&self, params: DidChangeTextDocumentParams) {
        //on change, take change and range of change
        //modify existing document given uri and cache content to update the document version in cache
        //TODO: check whether changes are purely whitespace
        //if changes are purely whitespace, check whether they

        let uri = params.text_document.uri;
        let lsp_cache = &self.lsp_cache;

        if let Some(change) = params.content_changes.first() {
            if let Some(cache_conts) = lsp_cache.get(&uri).await {
                
                let mut new_text = cache_conts.contents.clone();
                if let Some(lsp_range) = change.range {
                    //convert range for string conversion here

                    let start_byte = position_to_byte(&cache_conts.contents, lsp_range.start);
                    let end_byte = position_to_byte(&cache_conts.contents, lsp_range.end);
                    new_text.replace_range(start_byte..end_byte, &change.text);
                } else {
                    new_text = change.text.clone();
                }

                let new_tree: Tree = if let Some(lsp_range) = change.range {
                    let start_byte = position_to_byte(&cache_conts.contents, lsp_range.start);
                    let old_end_byte = position_to_byte(&cache_conts.contents, lsp_range.end);
                    let new_end_byte = old_end_byte + change.text.len();

                    let start_position = position_to_treesitter_point(lsp_range.start);
                    let old_end_position = get_end_position(&cache_conts.contents.clone());
                    let new_end_position = position_to_treesitter_point(lsp_range.end);

                    if let Some(ref mut old_cst) = cache_conts.cst.clone() {
                        old_cst.edit(&tree_sitter::InputEdit {
                            start_byte,
                            old_end_byte,
                            new_end_byte,
                            start_position,
                            old_end_position,
                            new_end_position,
                        });
                        old_cst.clone()
                    } else {
                        // if cst was None due to a failure to parse,
                        // we should re-parse the entire new text instead of trying to edit a non-existent tree
                        // there could be a better way to handle this, but for now this is a safe fallback
                        get_tree(&new_text).unwrap().0
                    }
                } else {
                    get_tree(&new_text).unwrap().0
                };

                let context = Arc::new(RwLock::new(Context::default()));
                let mut errors: Vec<RecoverableParseError> = Vec::new();

                let parsed = parse_essence_with_context_and_map(
                    &new_text,
                    context,
                    &mut errors,
                    Some(&new_tree),
                );
                
                let new_cache_conts = match parsed {
                    Ok(Some((ast_model, source_map))) => {
                        CacheCont {
                            sourcemap: Some(source_map),
                            ast: Some(ast_model),
                            errors,
                            cst: Some(new_tree),
                            contents: new_text.clone(),
                            version: params.text_document.version,
                        }
                    }
                    Ok(None) => {
                        CacheCont { 
                            sourcemap: None,
                            ast: None, 
                            errors, 
                            cst: Some(new_tree), 
                            contents: new_text.clone(), 
                            version: params.text_document.version, 
                        }
                    }
                    Err(fatal) => {
                        CacheCont {
                            sourcemap: None,
                            ast: None,
                            errors: vec![RecoverableParseError::new(fatal.to_string(), None)],
                            cst: Some(new_tree),
                            contents: new_text.clone(),
                            version: params.text_document.version,
                        }
                    }
                };

                lsp_cache.insert(uri.clone(), new_cache_conts.clone()).await;

                self.client
                    .log_message(MessageType::INFO, "Document changed, cache updated")
                    .await;
            }
        }

        self.handle_diagnostics(&uri, new_cache_conts).await;
    }

    pub async fn handle_diagnostics(&self, uri: &Url, cache_conts: CacheCont) {
        //needs to be modified to use cst and ast from cache
        //using lsp_cache.get(&uri) assumedly and then feeding
        //these values back to the diagnostic thingy to get my diags

        //ideal situation is feed diagnostics struct and then let it use struct to return diagnostics
        // e.g.:
        //let diagnostics = get_diagnostics(&cache_conts);
        let syntactic_diagnostics =
            get_diagnostics(&cache_conts.contents, &(cache_conts.cst.as_ref().unwrap()));
        let semantic_diagnostics: Vec<Diagnostic> = cache_conts
            .errors
            .into_iter()
            .filter_map(|err: RecoverableParseError| Some(error_to_diagnostic(&err)))
            .collect();
        let mut diagnostics = syntactic_diagnostics;
        diagnostics.extend(semantic_diagnostics);

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

//need to convert from character and line to byte value in a file
pub fn position_to_byte(text: &str, position: Position) -> usize {
    //as_bytes converts a string into bytes which I could do with text but the issue is finding
    //the position from that point???
    let mut byte_offset = 0;
    //go through every line
    for (line_idx, line) in text.lines().enumerate() {
        if line_idx < position.line as usize {
            byte_offset += line.len() + 1; // +1 for newline
        } else {
            //can directly convert character as it's a byte offset alr
            byte_offset += position.character as usize;
            break;
        }
    }
    return byte_offset;
}

//need to convert from character and line to row and line
//this allows for incremental editing of treesitter
fn position_to_treesitter_point(position: Position) -> Point {
    return Point::new(position.line as usize, position.character as usize);
}

fn get_end_position(text: &str) -> Point {
    let mut row = 0 as usize;
    let mut column = 0 as usize;
    for char in text.chars() {
        if char == '\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    return Point::new(row, column);
}
