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
}

#[tokio::main]
pub async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend { client }).finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
