use std::fmt::format;

use serde::{Deserialize, Serialize};
use tokio::stream;
use tower_lsp::{async_trait, jsonrpc::Error, lsp_types::{notification::Notification, ExecuteCommandOptions, ExecuteCommandParams, InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities}, LanguageServer, LspService};



#[derive(Debug, Serialize, Deserialize)] 
struct NotifactionParams { //to be fed into notifciation?
    title: String,
    message: String,
    description: String,
}

enum CustomNotifciation {}

impl Notification for CustomNotifciation {
    type Params =  NotifactionParams;

    const METHOD: &'static str = "custom/notification";
}

#[derive(Debug)]
struct Backend {
    client: Client,
}

#[async_trait]
impl LanguageServer for Backend { //this is the server implementation and manages the server response to client requests
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> { //first request of client to server
        Ok(
            InitializeResult {
                 capabilities: Null, 
                 server_info: ServerCapabilities { //
                    execute_command_provider = Some(ExecuteCommandOptions{
                        commands: vec![String::from("custom.notification")],
                        work_done_progress_options: Default::default()
                    }),
                    ..ServerCapabilities::default()
                },
                ..Default::default()
            }
        )
    }
    async fn initialized(&self, _: InitializedParams) { //request after recieving result of initialise() and before anything else
        self.client 
            .log_message(MessageType::INFO, "server initialised!") //client logs message of initialised
            .await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn execute_command(&self, params: ExecuteCommandParams) ->
    Result<Option<Value>> {
        if params.command == "custom.notification" { //one of the commands that we support (see line 34)
            self.client
            .send_notification::<CustomNotifciation>(NotifactionParams { //send_notification is defined by the client
                title: String::from("Hello Notification"),
                message: String::from("This is a test message"),
                description: String::from("This is a description")
            }).await;

            self.client
            .log_message(MessageType::INFO, format!("Command executed successfully with params: {params:?}"))
            .await;
        Ok(None)
        } //can add additional commands here in an if-else block
        else {
            Err(Error::invalid_request())
        }
    }

    //LOOK AT LANGUAGESERVER EXAMPLES ON TOWER_LSP WHEN DONE HERE
    // async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
    //      Ok(Some(Hover {
    //          contents: HoverContents::Scalar(
    //              MarkedString::String("You're hovering!".to_string())
    //          ),
    //          range: None
    //      }))
    //  }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
    .await
    .unwrap();

    let (stream, _) = listener.accept()
    .await
    .unwrap();

    let (read, write) = tokio::io::split(stream);

    let (service, socket) = LspService::new(|client|Backend{ client });

    Server::new(read,write).serve(service).await;


}

