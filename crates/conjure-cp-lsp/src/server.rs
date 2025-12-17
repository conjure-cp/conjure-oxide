use serde::{Deserialize, Serialize};
use serde_json::Value;

use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::{Error, Result},
    lsp_types::{
        ExecuteCommandOptions, ExecuteCommandParams, InitializeParams, InitializeResult,
        InitializedParams, MessageType, ServerCapabilities, notification::Notification,
    },
};

use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Diagnostic as ParserDiagnostic;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Severity as ParserSeverity;
use tower_lsp::lsp_types::Range as LspRange;
use tower_lsp::lsp_types::Position as LspPosition;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Range as ParserRange;
use conjure_cp_essence_parser::diagnostics::diagnostics_api::Position as ParserPosition;

use tower_lsp::lsp_types::Diagnostic as LspDiagnostic;

#[derive(Debug, Serialize, Deserialize)]
struct NotifactionParams {
    title: String,
    message: String,
    description: String,
}

enum CustomNotifciation {}

impl Notification for CustomNotifciation {
    type Params = NotifactionParams;

    const METHOD: &'static str = "custom/notification";
}

#[derive(Debug)]
struct Backend {
    client: Client,
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
    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        if params.command == "custom.notification" {
            //one of the commands that we support (see line 34)
            self.client
                .send_notification::<CustomNotifciation>(NotifactionParams {
                    //send_notification is defined by the client
                    title: String::from("Hello Notification"),
                    message: String::from("This is a test message"),
                    description: String::from("This is a description"),
                })
                .await;

            self.client
                .log_message(
                    MessageType::INFO,
                    format!("Command executed successfully with params: {params:?}"),
                )
                .await;
            Ok(None)
        }
        //can add additional commands here in an if-else block
        else {
            Err(Error::invalid_request())
        }
    }
    // underline errors on file open
    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        // get_diagnostics takes in source code as &str and returns Vec<Diagnostic>
        let diagnostics = get_diagnostics(&params.text_document.text); // get diagnostics from cp-essence-parser
        let lsp_diagnostics = convert_diagnostics(diagnostics); // convert to LSP diagnostics

        self.client.publish_diagnostics(
            params.text_document.uri,
            lsp_diagnostics,
            None,
        ).await;
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