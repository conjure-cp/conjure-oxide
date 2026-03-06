use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp_core::ast::Model;
use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::RecoverableParseError;
use conjure_cp_essence_parser::parse_essence_with_context_and_map;
use tower_lsp::{lsp_types::Diagnostic as LspDiagnostic, lsp_types::*};

use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic as ParserDiagnostic;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Position as ParserPosition;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Range as ParserRange;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Severity as ParserSeverity;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use tower_lsp::lsp_types::Position as LspPosition;
use tower_lsp::lsp_types::Range as LspRange;

use moka::future::Cache;
use tree_sitter::InputEdit;
use tree_sitter::Point;
use tree_sitter::Tree;

use crate::handlers::cache;
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

                let cst_tree = tree_sitter::Parser::new().parse(&text, None).unwrap();

                let context = Arc::new(RwLock::new(Context::default()));
                let mut errors: Vec<RecoverableParseError> = Vec::new();

                let parsed = parse_essence_with_context_and_map(&text, context, &mut errors, Some(&cst_tree));
                let (ast_model, source_map) = parsed.unwrap().unwrap();
                CacheCont {
                    sourcemap: Some(source_map),       // need to get this using parse_essence_with_context_and_map
                    ast: ast_model, // need to get this using parse_essence_with_context_and_map
                    cst: cst_tree, // get this onOpen using tree-sitter directly, then send it to parse_essence_with_context_and_map to get sourcemap and ast
                    contents: text.clone(),
                    version: 0,
                }
            })
            .await;
        // parse_essence_with_context_and_map(src, context, errors, tree)

        //NOT SURE THAT THIS NEEDS TO EXIST BUT PUTTING HERE IN CASE IT DOES
        // let cached = cache.get_with(uri.clone(), async {
        //     panic!("This should never run");
        // }).await; 

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
        //check whether changes are purely whitespace
        //if changes are purely whitespace, check whether they

        let uri = params.text_document.uri;
        let lsp_cache = &self.lsp_cache;

        // let range = params.content_changes.first().unwrap().range.unwrap();

        if let Some(change) = params.content_changes.first() {
            if let Some(cache_conts) = lsp_cache.get(&uri).await {
                //this should replace the whole of the range of the changes?
                //cannot see how this wouldn't work sooooooo it better lwk
                //might be worth printing before and after on client
                //just to see if working?
                let mut new_text = cache_conts.contents.clone();
                if let Some(lsp_range) = change.range {
                    //convert range for string conversion here
                    // new_text.replace_range(range, replace_with);

                    let start_byte = position_to_byte(&cache_conts.contents, lsp_range.start);
                    let end_byte = position_to_byte(&cache_conts.contents, lsp_range.end);
                    new_text.replace_range(start_byte..end_byte, &change.text);
                } else {
                    new_text = change.text.clone();
                }

                let new_tree: Tree = if let Some(lsp_range) = change.range {
                    let start_byte = position_to_byte(&cache_conts.contents, lsp_range.start);
                    let old_end_byte =  position_to_byte(&cache_conts.contents, lsp_range.end);
                    let new_end_byte = old_end_byte + change.text.len();
                    let start_position = position_to_treesitter_point(lsp_range.start);
                    let old_end_position = get_end_position(&cache_conts.contents.clone());
                    let new_end_position = position_to_treesitter_point(lsp_range.end);
                    // let edit_range = tree_sitter::InputEdit::
                    let mut old_cst = cache_conts.cst.clone();
                    old_cst.edit(&tree_sitter::InputEdit{
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position,
                        old_end_position,
                        new_end_position,
                    });
                    tree_sitter::Parser::new().parse(&new_text, Some(&old_cst)).unwrap()
                } else {
                    tree_sitter::Parser::new().parse(&new_text, Some(&cache_conts.cst)).unwrap()
                };

                let context = Arc::new(RwLock::new(Context::default()));
                let mut errors: Vec<RecoverableParseError> = Vec::new();

                let (ast_model, source_map) = parse_essence_with_context_and_map(&new_text, context, &mut errors, Some(&new_tree)).unwrap().unwrap();

                let new_cache_conts = CacheCont {
                    sourcemap: Some(source_map),
                    ast: ast_model,
                    cst: new_tree,
                    contents: new_text.clone(),
                    version: params.text_document.version,
                };
                
                lsp_cache.insert(uri.clone(), new_cache_conts.clone()).await;

                self.client
                    .log_message(MessageType::INFO, "Did change document")
                    .await;
            }
        }
        
        if let Some(new_cache_conts) = lsp_cache.get(&uri).await {

            self.client
                .log_message(MessageType::INFO, "Did save document")
                .await;
            self.handle_diagnostics(&uri, new_cache_conts).await;
        }
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

//need to convert from character and line to byte value in a file
fn position_to_byte(text: &str, position: Position) -> usize {
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
