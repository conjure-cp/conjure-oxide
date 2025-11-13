use serde::{Deserialize, Serialize};
use serde_json::Value;

use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::{Error, Result},
    lsp_types::notification::Notification,
    lsp_types::*,
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// use crate::conjure_cp_lsp::hover...

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
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

impl Backend {
    pub fn new(client: Client, documents: Arc<RwLock<HashMap<Url, String>>>) -> Self {
        Backend { client, documents }
    }
    async fn get_text(&self, uri: &Url) -> Option<String> {
        self.documents.read().await.get(uri).cloned()
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
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
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
        self.client
            .log_message(MessageType::INFO, "server shut down!") //client logs message of initialised
            .await;
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let text = params.text_document.text.clone();
        let uri = &params.text_document.uri;
        self.documents.write().await.insert(uri.clone(), text.clone());
        self.client
            .log_message(MessageType::INFO, "In did_open :)") //client logs message of initialised
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let line = params.text_document_position_params.position.line;
        let character = params.text_document_position_params.position.character;
        let text = self.get_text(&uri).await.unwrap_or_default();
        let word = get_hovered_word(&text, line as usize, character as usize);

        self.client
            .log_message(MessageType::INFO, word.unwrap_or_default())
            .await;

        // //find path of conjure to access conjure/docs/bits/etc for documentation
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String("You're hovering!".to_string())),
            range: None,
        }))
        // let document = &params.text_document_position_params.text_document.uri;

        // Ok(document.hover(params.text_document_position_params.position))
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
}

fn get_hovered_word(text: &str, line: usize, character: usize) -> Option<&str>{
    let text_line = text.lines().nth(line)?;
    let mut idx = 0;
    for word in text_line.split_whitespace() {
        let end = idx + word.len();
        if character <= end && character >= idx {
            return Some(word);
        }
        idx = end + 1;
    }
    None
}

#[tokio::main]
pub async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let documents = Arc::new(RwLock::new(HashMap::new()));

    let (service, socket) = LspService::build(|client| Backend::new(client, Arc::clone(&documents))).finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
