use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::RecoverableParseError;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::error_to_diagnostic;
use conjure_cp_essence_parser::diagnostics::source_map::SourceMap;
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

// parse debounce is the amount of time to wait after a document change before re-parsing and updating diagnostics
// helps avoid excessive parsing on rapid successive edits
const PARSE_DEBOUNCE_MS: u64 = 120;

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
        publish_diagnostics(&self.client, &uri.clone(), cache_content).await;
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
        let mut provisional_sourcemap = cache_conts.sourcemap.clone();

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

                let start_position = position_to_treesitter_point(&new_text, lsp_range.start);
                let old_end_position = position_to_treesitter_point(&new_text, lsp_range.end);
                let new_end_position = calculate_new_end_position(&change.text, start_position);
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

                if let Some(map) = provisional_sourcemap.as_mut() {
                    shift_sourcemap_after_edit(map, start_byte, old_end_byte, new_end_byte);
                }

                new_text.replace_range(start_byte..old_end_byte, &change.text);
            } else {
                // Full content replacement.
                new_text = change.text.clone();
                edited_tree = None;
                provisional_sourcemap = None;
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

        // store updated text/tree IMMEDIATELY so subsequent incremental edits are based on
        // the latest document state, then parse & diagnose in a debounced task
        let provisional = CacheCont {
            sourcemap: provisional_sourcemap,
            ast: cache_conts.ast.clone(),
            errors: cache_conts.errors.clone(),
            cst: new_tree.clone(),
            contents: new_text.clone(),
            version: incoming_version,
        };
        lsp_cache.insert(uri.clone(), provisional).await;

        let lsp_cache = lsp_cache.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(PARSE_DEBOUNCE_MS)).await;

            let Some(current) = lsp_cache.get(&uri).await else {
                return;
            };

            // only parse the newest queued version
            if current.version != incoming_version {
                return;
            }

            let context = Arc::new(RwLock::new(Context::default()));
            let mut errors: Vec<RecoverableParseError> = Vec::new();
            let parsed = parse_essence_with_context_and_map(
                &current.contents,
                context,
                &mut errors,
                current.cst.as_ref(),
            );

            let parsed_cache = match parsed {
                Ok((Some(ast_model), source_map)) => CacheCont {
                    sourcemap: Some(source_map),
                    ast: Some(ast_model),
                    errors,
                    cst: current.cst.clone(),
                    contents: current.contents.clone(),
                    version: incoming_version,
                },
                Ok((None, source_map)) => CacheCont {
                    sourcemap: Some(source_map),
                    ast: None,
                    errors,
                    cst: current.cst.clone(),
                    contents: current.contents.clone(),
                    version: incoming_version,
                },
                Err(fatal) => CacheCont {
                    sourcemap: None,
                    ast: None,
                    errors: vec![RecoverableParseError::new(fatal.to_string(), None)],
                    cst: current.cst.clone(),
                    contents: current.contents.clone(),
                    version: incoming_version,
                },
            };

            if let Err(err) = client.semantic_tokens_refresh().await {
                client
                    .log_message(
                        MessageType::WARNING,
                        format!("semantic_tokens_refresh failed on change: {err}"),
                    )
                    .await;
            }

            if let Some(latest) = lsp_cache.get(&uri).await
                && latest.version != incoming_version
            {
                return;
            }
            lsp_cache.insert(uri.clone(), parsed_cache.clone()).await;

            publish_diagnostics(&client, &uri, parsed_cache).await;
        });
    }
}

async fn publish_diagnostics(client: &tower_lsp::Client, uri: &Url, cache_conts: CacheCont) {
    // Build diagnostics from the parse errors cached for this document.
    // parse_essence_with_context_and_map already produces both syntactic and semantic errors.
    let diagnostics: Vec<Diagnostic> = cache_conts
        .errors
        .into_iter()
        .map(|err| error_to_diagnostic(&err))
        .collect();

    let lsp_diagnostics = convert_diagnostics(diagnostics);

    client
        .publish_diagnostics(uri.clone(), lsp_diagnostics, None)
        .await;
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

fn shift_sourcemap_after_edit(
    source_map: &mut SourceMap,
    start_byte: usize,
    old_end_byte: usize,
    new_end_byte: usize,
) {
    let delta = new_end_byte as isize - old_end_byte as isize;

    for span in &mut source_map.spans {
        if span.end_byte <= start_byte {
            continue;
        }

        if span.start_byte >= old_end_byte {
            span.start_byte = shift_byte(span.start_byte, delta);
            span.end_byte = shift_byte(span.end_byte, delta);
            continue;
        }

        // if the edited region intersects this span, invalidate it
        //  until the debounced full parse
        span.start_byte = 0;
        span.end_byte = 0;
        span.hover_info = None;
    }

    source_map.by_byte = Default::default();
    for (idx, span) in source_map.spans.iter().enumerate() {
        if span.start_byte < span.end_byte {
            source_map
                .by_byte
                .insert(span.start_byte..span.end_byte, idx as u32);
        }
    }
}

// helpr to shift a byte position
fn shift_byte(byte: usize, delta: isize) -> usize {
    if delta >= 0 {
        byte.saturating_add(delta as usize)
    } else {
        byte.saturating_sub((-delta) as usize)
    }
}
