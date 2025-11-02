use std::fmt::format;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// use tokio::stream;
use tower_lsp::{Client, Server, jsonrpc::{Result,Error}, lsp_types::{notification::Notification, ExecuteCommandOptions, ExecuteCommandParams, InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities}, LanguageServer, LspService};
use log::info;


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

#[tower_lsp::async_trait]
impl LanguageServer for Backend { //this is the server implementation and manages the server response to client requests
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> { //first request of client to server
        Ok(
            InitializeResult {
                 server_info: None, 
                 capabilities: ServerCapabilities { //
                    execute_command_provider : Some(ExecuteCommandOptions{
                        commands: vec![String::from("custom.notification")],
                        work_done_progress_options: Default::default()
                    }),
                    ..ServerCapabilities::default()
                },
                // semantic_tokens_provider: Some(
                //     SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                //         SemanticTokensRegistrationOptions {
                //             text_document_registration_options: {
                //                 TextDocumentRegistrationOptions {
                //                     document_selector: Some(vec![DocumentFilter {
                //                         language: Some("nrs".to_string()),
                //                         scheme: Some("file".to_string()),
                //                         pattern: None,
                //                     }]),
                //                 }
                //             },
                //             semantic_tokens_options: SemanticTokensOptions {
                //                 work_done_progress_options: WorkDoneProgressOptions::default(),
                //                 legend: SemanticTokensLegend {
                //                     token_types: LEGEND_TYPE.into(),
                //                     token_modifiers: vec![],
                //                 },
                //                 range: Some(true),
                //                 full: Some(SemanticTokensFullOptions::Bool(true)),
                //             },
                //             static_registration_options: StaticRegistrationOptions::default(),
                //         },
                //     ),
                // ),
                ..Default::default()
            },
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
pub async fn main() {
    // tracing_subscriber::fmt().init();

    // let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
    // .await
    // .unwrap();

    // let (stream, _) = listener.accept()
    // .await
    // .unwrap();

    // let (read, write) = tokio::io::split(stream);

    // let (service, socket) = LspService::new(|client|Backend{ client });

    // Server::new(read,write,socket).serve(service).await;

    // env_logger::init();
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Essence LSP server");
    println!("Starting Essence LSP server");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
    })
    .finish();

    info!("Finished initialising server! Waiting for client connection");
    println!("Finished initialising server! Waiting for client connection");
    
    Server::new(stdin,stdout,socket).serve(service).await;


}

