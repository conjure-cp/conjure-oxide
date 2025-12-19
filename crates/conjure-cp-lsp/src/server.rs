use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::{Result}, //add Error if needed later, currently unused
    lsp_types::*,
};

use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic as ParserDiagnostic;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Severity as ParserSeverity;
use tower_lsp::lsp_types::Range as LspRange;
use tower_lsp::lsp_types::Position as LspPosition;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Range as ParserRange;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Position as ParserPosition;

use tower_lsp::lsp_types::Diagnostic as LspDiagnostic;

use tokio::fs;

#[derive(Debug)]
struct Backend {
    client: Client,
}

impl Backend {
    pub async fn handle_diagnostic(&self, uri: &Url) {
        // if let Ok(path) = uri.to_file_path().ok() {
        let file_path = uri.to_file_path().ok();
        
        if let Some(path) = file_path {
            match fs::read_to_string(&path).await {
                Ok(content) => {
                    // get_diagnostics takes in source code as &str and returns Vec<Diagnostic>
                    let diagnostics = get_diagnostics(&content); // get diagnostics from cp-essence-parser
                    let lsp_diagnostics = convert_diagnostics(diagnostics); // convert to LSP diagnostics // convert to LSP diagnostics
                    
                    // Publish diagnostics back to the client
                    self.client
                        .publish_diagnostics(uri.clone(), lsp_diagnostics, None)
                        .await;
                }
                Err(e) => {
                    eprintln! ("Failed to read file {}: {}", path. display(), e);
                }
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    //this is the server implementation and manages the server response to client requests
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        //first request of client to server
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                //
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![String::from("custom.notification")],
                    work_done_progress_options: Default::default(),
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        //request after recieving result of initialise() and before anything else
        self.client
            .log_message(MessageType::INFO, "server initialised!") //client logs message of initialised
            .await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    // underline errors on file open
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = &params.text_document.uri;
        dbg!("did open", uri);
        // get_diagnostics takes in source code as &str and returns Vec<Diagnostic>

        self.handle_diagnostic(uri).await;
    }
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = &params.text_document.uri;
        dbg!("did save", uri);

        self.handle_diagnostic(uri).await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = &params.text_document.uri;
        dbg!("did change", uri);

        self.handle_diagnostic(uri).await;
    }
}

#[tokio::main]
pub async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend { client }).finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}

// convert diagnostics from cp-essence-parser to LSP diagnostics
pub fn convert_diagnostics(diagnostics: Vec<ParserDiagnostic>) -> Vec<LspDiagnostic> {
    // map each ParserDiagnostic to LspDiagnostic
    diagnostics.into_iter().map(|diag| {
        LspDiagnostic {
            range: parser_to_lsp_range(diag.range),
            severity: match diag.severity {
                ParserSeverity::Error => Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
                ParserSeverity::Warn => Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                ParserSeverity::Info => Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION),
                ParserSeverity::Hint => Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT),
            },
            code: None, // for now
            code_description: None, // also for now
            source: Some(diag.source.to_string()),
            message: diag.message,
            related_information: None,
            tags: None,
            data: None,
        }
    }).collect()
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