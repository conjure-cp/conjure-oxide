// use conjure_cp_core::ast::Model;
// use tree_sitter::Tree;

use tower_lsp::{
    Client,
    LanguageServer,
    LspService,
    Server,
    jsonrpc::Result, //add Error if needed later, currently unused
    lsp_types::*,
};

use crate::handlers::cache::{CacheCont, create_cache};

// use std::clone;
// use std::collections::HashMap;

// use std::sync::Arc;
// use tokio::sync::RwLock;
use moka::future::Cache;

#[derive(Debug)]
pub struct Backend {
    pub client: Client,
    pub lsp_cache: Cache<Url, CacheCont>,
}

impl Backend {
    pub fn new(client: Client, lsp_cache: Cache<Url, CacheCont>) -> Self {
        Backend { client, lsp_cache } //add cache here
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
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        // change: Some(TextDocumentSyncKind::FULL),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
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

        //set up cache here NVM SET UP IN MAIN
        // let cache = create_cache().await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    // underline errors on file open
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.handle_did_open(params).await;
    }
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.handle_did_save(params).await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.handle_did_change(params).await;
    }
}

#[tokio::main]
pub async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    // let documents = Arc::new(RwLock::new(HashMap::new()));
    // let cache = Cache<Url, CacheCont>::new();

    let lsp_cache = create_cache().await;

    let (service, socket) =
        // LspService::build(|client| Backend::new(client, Arc::clone(&documents))).finish();
        LspService::build(|client| Backend::new(client, lsp_cache)).finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
