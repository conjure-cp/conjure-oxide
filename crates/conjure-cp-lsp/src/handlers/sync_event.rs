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
    }
    pub async fn handle_did_save(&self, params: DidSaveTextDocumentParams) {
        // Diagnostics are driven by did_change. Re-publishing cached diagnostics on save can
        // race with in-flight did_change parsing and temporarily re-show stale diagnostics.
        let uri = params.text_document.uri;
        let _ = uri; // keep param usage explicit for now
        self.client
            .log_message(MessageType::INFO, "Did save document")
            .await;
    }
    pub async fn handle_did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let incoming_version = params.text_document.version;
        let lsp_cache = &self.lsp_cache;

        self.client
            .log_message(MessageType::INFO, "in document change")
            .await;

        let Some(cache_conts) = lsp_cache.get(&uri).await else {
            self.client
                .log_message(MessageType::WARNING, "DidChange for uncached document")
                .await;
            return;
        };

        // Drop stale/out-of-order changes.
        if incoming_version <= cache_conts.version {
            return;
        }

        let mut new_text = cache_conts.contents.clone();
        let mut edited_tree = cache_conts.cst.clone();

        // LSP may send multiple incremental edits in one notification.
        for change in &params.content_changes {
            if let Some(lsp_range) = change.range {
                let start_byte = position_to_byte(&new_text, lsp_range.start);
                let old_end_byte = position_to_byte(&new_text, lsp_range.end);

                if start_byte > old_end_byte || old_end_byte > new_text.len() {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            "Ignoring invalid edit range in DidChange",
                        )
                        .await;
                    continue;
                }

                let start_position = position_to_treesitter_point(lsp_range.start);
                let old_end_position = position_to_treesitter_point(lsp_range.end);
                let new_end_position = calculate_new_end_position(&change.text, lsp_range.start);
                let new_end_byte = start_byte + change.text.len();

                if let Some(tree) = edited_tree.as_mut() {
                    tree.edit(&tree_sitter::InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position,
                        old_end_position,
                        new_end_position,
                    });
                }

                new_text.replace_range(start_byte..old_end_byte, &change.text);
            } else {
                // Full content replacement.
                new_text = change.text.clone();
                edited_tree = None;
            }
        }

        let mut new_tree: Option<Tree> = if let Some(ref old_tree) = edited_tree {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_essence::LANGUAGE.into())
                .unwrap();
            parser.parse(&new_text, Some(old_tree))
        } else {
            None
        };

        if new_tree.is_none() {
            new_tree = get_tree(&new_text).map(|(tree, _)| tree);
        }

        let context = Arc::new(RwLock::new(Context::default()));
        let mut errors: Vec<RecoverableParseError> = Vec::new();
        let parsed =
            parse_essence_with_context_and_map(&new_text, context, &mut errors, new_tree.as_ref());

        let new_cache_conts = match parsed {
            Ok((Some(ast_model), source_map)) => CacheCont {
                sourcemap: Some(source_map),
                ast: Some(ast_model),
                errors,
                cst: new_tree.clone(),
                contents: new_text.clone(),
                version: incoming_version,
            },
            Ok((None, source_map)) => CacheCont {
                sourcemap: Some(source_map),
                ast: None,
                errors,
                cst: new_tree.clone(),
                contents: new_text.clone(),
                version: incoming_version,
            },
            Err(fatal) => CacheCont {
                sourcemap: None,
                ast: None,
                errors: vec![RecoverableParseError::new(fatal.to_string(), None)],
                cst: new_tree.clone(),
                contents: new_text.clone(),
                version: incoming_version,
            },
        };

        // Drop stale parse results if a newer update landed while parsing.
        if let Some(latest) = lsp_cache.get(&uri).await
            && latest.version >= incoming_version
        {
            return;
        }

        lsp_cache.insert(uri.clone(), new_cache_conts.clone()).await;
        self.handle_diagnostics(&uri, new_cache_conts).await;
    }

    pub async fn handle_diagnostics(&self, uri: &Url, cache_conts: CacheCont) {
        // Build diagnostics from the parse errors cached for this document.
        // parse_essence_with_context_and_map already produces both syntactic and semantic errors.
        let diagnostics: Vec<Diagnostic> = cache_conts
            .errors
            .into_iter()
            .map(|err| error_to_diagnostic(&err))
            .collect();

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
    byte_offset
}

//need to convert from character and line to row and line
//this allows for incremental editing of treesitter
fn position_to_treesitter_point(position: Position) -> Point {
    Point::new(position.line as usize, position.character as usize)
}

fn calculate_new_end_position(text: &str, start: Position) -> Point {
    let mut row = start.line as usize;
    let mut column = start.character as usize;
    for ch in text.chars() {
        if ch == '\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Point::new(row, column)
}
