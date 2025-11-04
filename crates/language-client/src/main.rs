use std::{error::Error};
use serde_json::json;

// use tower_lsp::lsp_types::request::Initialize;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}}; //, net::TcpStream};
// use tracing_subscriber::fmt::format;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = tokio::net::TcpStream::connect("127.0.0.1:8080")
    .await?;

    let initialise_request = json!({
        "jsonrpc" : "2.0",
        "id" : 1,
        "method": "initialize",
        "params" : {
            "processid" : null,
            "rootUri" : null,
            "capabilities" : {}
        }
    });

    let initialize_request_str = initialise_request.to_string();

    let intialize_request_formatted = format!(
        "Content-Length: {}\r\n\r\n{}",
        initialize_request_str.len(),
        initialize_request_str
    );

    stream.write_all(intialize_request_formatted.as_bytes())
    .await?;

    let mut buffer = [0; 1024];
    let n = stream.read(& mut buffer)
    .await?;

    let response = String::from_utf8_lossy(&buffer[..n]);

    println!("Recieved initialize response: {}", response);

    let execute_command_request = json!({
        "jsonrpc" : "2.0",
        "id" : 2,
        "method": "workspace/executeCommand",
        "params" : {
            "command" : "custom.notification",
            "params" : {
                "command" : "custom.notification",
                "arguments" : [
                    {
                        "title" : "Hello",
                        "message" : "Hello from client",
                        "description" : "this is a custom notification from client"
                    }
                ]
            }
        }
    });

    json!({
        "jsonrpc" : "2.0",
        "id" : 1,
        "method": "initialize",
        "params" : {
            "processid" : null,
            "rootUri" : null,
            "capabilities" : {}
        }
    });


    let execute_command_str = execute_command_request.to_string();

    let formatted_command_request = format!(
        "Content-Length: {}\r\n\r\n{}",
        execute_command_str.len(),
        execute_command_str
    );

    stream.write_all(formatted_command_request.as_bytes())
    .await?;


    let mut buffer: [u8; 1024] = [0; 1024];
    let n = stream.read(& mut buffer)
    .await?;

    let response = String::from_utf8_lossy(&buffer[..n]);

    println!("Recieved custom notification response: {}", response);


    Ok(())
}