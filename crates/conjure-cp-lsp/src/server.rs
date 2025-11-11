use serde::{Deserialize, Serialize};
use serde_json::Value;

use tower_lsp::{
    Client, LanguageServer, LspService, Server,
    jsonrpc::{Error, Result},
    lsp_types::notification::Notification,
    lsp_types::*,
};

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
                hover_provider: Some(HoverProviderCapability::Simple(true)),
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
        let text_document = params.text_document;
        
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        self.client
            .log_message(MessageType::INFO, "hovering! :) Recieved {}") //client logs message of initialised
            .await;

        self.client
            .log_message(MessageType::INFO, params.text_document_position_params.text_document.uri)
            .await;

        self.client
            .log_message(MessageType::INFO, params.text_document_position_params.position.line)
            .await;

        self.client
            .log_message(MessageType::INFO, params.text_document_position_params.position.character)
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

#[tokio::main]
pub async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend { client }).finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
