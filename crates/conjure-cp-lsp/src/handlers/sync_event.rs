use tower_lsp::{lsp_types::Diagnostic as LspDiagnostic, lsp_types::*};

use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic as ParserDiagnostic;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Position as ParserPosition;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Range as ParserRange;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Severity as ParserSeverity;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use tower_lsp::lsp_types::Position as LspPosition;
use tower_lsp::lsp_types::Range as LspRange;

use crate::server::Backend;

// use tokio::fs;

impl Backend {
    pub async fn handle_did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text.clone();
        self.documents
            .write()
            .await
            .insert(uri.to_string().clone(), text.clone());

        self.client
            .log_message(MessageType::INFO, "Did open document")
            .await;

        //diagnostic stuff here
        self.handle_diagnostics(&uri.clone(), text.clone()).await;
    }
    pub async fn handle_did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.documents
                .write()
                .await
                .insert(uri.to_string().clone(), text.clone());

            self.client
                .log_message(MessageType::INFO, "Did save document")
                .await;

            self.handle_diagnostics(&uri.clone(), text.clone()).await;
        }
    }
    pub async fn handle_did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        if let Some(change) = params.content_changes.first() {
            let text = change.text.clone();

            self.client
                .log_message(MessageType::INFO, format!("New text: {}", text))
                .await;

            self.documents
                .write()
                .await
                .insert(uri.to_string().clone(), text.clone());

            //diagnostic stuff here
            self.handle_diagnostics(&uri.clone(), text.clone()).await;
        }
    }

    pub async fn handle_diagnostics(&self, uri: &Url, code: String) {
        let diagnostics = get_diagnostics(&code);
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
