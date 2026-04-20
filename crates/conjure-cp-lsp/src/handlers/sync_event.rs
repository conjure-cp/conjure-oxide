use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::RecoverableParseError;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::error_to_diagnostic;
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

use tree_sitter::Point;
use tree_sitter::Tree;

use crate::handlers::cache::CacheCont;
use crate::server::Backend;

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
                let (cst_tree, _) = get_tree(&text).unwrap();

                let context = Arc::new(RwLock::new(Context::default()));
                let mut errors: Vec<RecoverableParseError> = Vec::new();

                let parsed = parse_essence_with_context_and_map(
                    &text,
                    context,
                    &mut errors,
                    Some(&cst_tree),
                );

                match parsed {
                    Ok((Some(ast_model), source_map)) => CacheCont {
                        sourcemap: Some(source_map),
                        ast: Some(ast_model),
                        errors,
                        cst: Some(cst_tree),
                        contents: text.clone(),
                        version: params.text_document.version,
                    },
                    Ok((None, source_map)) => CacheCont {
                        sourcemap: Some(source_map),
                        ast: None,
                        errors,
                        cst: Some(cst_tree),
                        contents: text.clone(),
                        version: params.text_document.version,
                    },
                    Err(fatal) => CacheCont {
                        sourcemap: None,
                        ast: None,
                        errors: vec![RecoverableParseError::new(fatal.to_string(), None)],
                        cst: Some(cst_tree),
                        contents: text.clone(),
                        version: params.text_document.version,
                    },
                } //this inserts the cache created above into the cache
            })
            .await;

        self.client
            .log_message(MessageType::INFO, "Did open document")
            .await;

        //diagnostic stuff here
        self.handle_diagnostics(&uri.clone(), cache_content).await;
        if let Err(err) = self.client.semantic_tokens_refresh().await {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("semantic_tokens_refresh failed on open: {err}"),
                )
                .await;
        }
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

        self.client
            .log_message(MessageType::INFO, "in document change")
            .await;

        if let Some(change) = params.content_changes.first()
            && let Some(cache_conts) = lsp_cache.get(&uri).await
        {
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
                let new_end_byte = start_byte + change.text.len();

                let start_position =
                    position_to_treesitter_point(&cache_conts.contents, lsp_range.start);
                let old_end_position =
                    position_to_treesitter_point(&cache_conts.contents, lsp_range.end);
                let new_end_position = calculate_new_end_position(&change.text, start_position);

                self.client
                    .log_message(MessageType::INFO, "before edit")
                    .await;
                if let Some(ref mut old_cst) = cache_conts.cst.clone() {
                    old_cst.edit(&tree_sitter::InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position,
                        old_end_position,
                        new_end_position,
                    });

                    // parse the new text with the edited tree as a starting point for incremental parsing
                    // TODO: handle _FRAGMENT_EXPRESSION like get_tree does
                    // maybe make separate helper for that or something
                    let mut parser = tree_sitter::Parser::new();
                    parser
                        .set_language(&tree_sitter_essence::LANGUAGE.into())
                        .unwrap();
                    tree_sitter::Parser::parse(&mut parser, &new_text, Some(old_cst)).unwrap()
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

            self.client
                .log_message(MessageType::INFO, new_text.clone())
                .await;

            let parsed = parse_essence_with_context_and_map(
                &new_text,
                context,
                &mut errors,
                Some(&new_tree),
            );

            let new_cache_conts = match parsed {
                Ok((Some(ast_model), source_map)) => {
                    self.client
                        .log_message(MessageType::LOG, "THIS ONE INSTEAD")
                        .await;
                    CacheCont {
                        sourcemap: Some(source_map),
                        ast: Some(ast_model),
                        errors,
                        cst: Some(new_tree),
                        contents: new_text.clone(),
                        version: params.text_document.version,
                    }
                }
                Ok((None, source_map)) => {
                    self.client
                        .log_message(MessageType::LOG, "jshdhshshshhs")
                        .await;
                    CacheCont {
                        sourcemap: Some(source_map),
                        ast: None,
                        errors,
                        cst: Some(new_tree),
                        contents: new_text.clone(),
                        version: params.text_document.version,
                    }
                }
                Err(fatal) => CacheCont {
                    sourcemap: None,
                    ast: None,
                    errors: vec![RecoverableParseError::new(fatal.to_string(), None)],
                    cst: Some(new_tree),
                    contents: new_text.clone(),
                    version: params.text_document.version,
                },
            };

            lsp_cache.insert(uri.clone(), new_cache_conts.clone()).await;

            self.handle_diagnostics(&uri, new_cache_conts).await;
            if let Err(err) = self.client.semantic_tokens_refresh().await {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("semantic_tokens_refresh failed on change: {err}"),
                    )
                    .await;
            }
        }
    }

    pub async fn handle_diagnostics(&self, uri: &Url, cache_conts: CacheCont) {
        // Get syntactic diagnostics from CST
        let syntactic_diagnostics = if let Some(ref cst) = cache_conts.cst {
            get_diagnostics(&cache_conts.contents, cst)
        } else {
            Vec::new()
        };

        // Get semantic diagnostics from errors
        let semantic_diagnostics: Vec<Diagnostic> = cache_conts
            .errors
            .into_iter()
            .map(|err| error_to_diagnostic(&err))
            .collect();

        // Combine all diagnostics
        let mut diagnostics = syntactic_diagnostics;
        diagnostics.extend(semantic_diagnostics);

        // Convert to LSP format
        let lsp_diagnostics = convert_diagnostics(diagnostics);

        // Publish diagnostics - this should be the ONLY call
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
    let row = position.line as usize;
    let line_start = line_start_byte(text.as_bytes(), row);
    let line_end = text[line_start..]
        .find('\n')
        .map(|off| line_start + off)
        .unwrap_or(text.len());
    let line_text = &text[line_start..line_end];
    let col_bytes = utf16_col_to_byte(line_text, position.character as usize);
    line_start + col_bytes
}

//need to convert from character and line to row and line
//this allows for incremental editing of treesitter
fn position_to_treesitter_point(text: &str, position: Position) -> Point {
    let row = position.line as usize;
    let line_start = line_start_byte(text.as_bytes(), row);
    let absolute = position_to_byte(text, position);
    Point::new(row, absolute.saturating_sub(line_start))
}

fn calculate_new_end_position(inserted_text: &str, start: Point) -> Point {
    let bytes = inserted_text.as_bytes();
    let newline_count = bytes.iter().filter(|&&b| b == b'\n').count();

    if newline_count == 0 {
        return Point::new(start.row, start.column + bytes.len());
    }

    let last_newline = bytes.iter().rposition(|&b| b == b'\n').unwrap_or(0);
    let trailing_bytes = bytes.len().saturating_sub(last_newline + 1);
    Point::new(start.row + newline_count, trailing_bytes)
}

fn line_start_byte(source: &[u8], row: usize) -> usize {
    let mut current_row = 0usize;
    let mut line_start = 0usize;
    for (idx, b) in source.iter().enumerate() {
        if current_row == row {
            break;
        }
        if *b == b'\n' {
            current_row += 1;
            line_start = idx + 1;
        }
    }
    line_start
}

fn utf16_col_to_byte(line: &str, utf16_col: usize) -> usize {
    let mut units = 0usize;
    for (idx, ch) in line.char_indices() {
        if units >= utf16_col {
            return idx;
        }
        let next = units + ch.len_utf16();
        if next > utf16_col {
            return idx;
        }
        units = next;
    }
    line.len()
}
