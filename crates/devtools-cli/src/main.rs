use clap::{Parser, Subcommand};
use cove_agent::transport::{self, Handler};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "cove-devtools",
    version,
    about = "Cove DevTools MCP server and AI tools"
)]
struct Cli {
    /// Port, ws:// endpoint or @session.json. Defaults to COVE_DEVTOOLS_ENDPOINT or 9229.
    #[arg(long, global = true)]
    endpoint: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Serve MCP over stdin/stdout; keeps the application connection alive.
    Mcp,
    /// List tool names, descriptions and input schemas without starting the app.
    Tools,
    /// Call a tool with a JSON arguments object.
    Call {
        name: String,
        #[arg(default_value = "{}")]
        arguments: String,
        /// Save image content here instead of printing base64.
        #[arg(long)]
        image: Option<std::path::PathBuf>,
    },
}

fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let target = cli
        .endpoint
        .or_else(|| std::env::var("COVE_DEVTOOLS_ENDPOINT").ok())
        .or_else(|| {
            std::env::var("COVE_DEVTOOLS_SESSION_FILE")
                .ok()
                .map(|path| format!("@{path}"))
        })
        .or_else(|| std::env::var("COVE_DEVTOOLS_PORT").ok())
        .unwrap_or_else(|| "9229".into());
    match cli.command {
        Command::Mcp => {
            let server = cove_devtools_cli::mcp_server(target)?;
            let handler: Handler = Arc::new(move |request| server.respond(request));
            transport::serve_stdio(handler)?;
        }
        Command::Tools => {
            let tools: Vec<_> = cove_devtools_cli::definitions()
                .iter()
                .map(|tool| tool.mcp_definition())
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"tools": tools}))?
            );
        }
        Command::Call {
            name,
            arguments,
            image,
        } => {
            let arguments: Value = serde_json::from_str(&arguments)?;
            let result = cove_devtools_cli::toolbox(target)?.execute(&name, &arguments)?;
            if let Some(path) = image {
                let first = result.images.first().ok_or("Tool returned no image.")?;
                use base64::Engine;
                std::fs::write(
                    path,
                    base64::engine::general_purpose::STANDARD.decode(&first.data)?,
                )?;
            }
            let mut value = result.value().unwrap_or(Value::Null);
            if !result.images.is_empty() {
                value["images"] = json!(result.images.iter().map(|image| json!({"mimeType": image.mime_type, "encodedBytes": image.data.len()})).collect::<Vec<_>>());
            }
            println!("{}", serde_json::to_string_pretty(&value)?);
            return Ok(!result.is_error);
        }
    }
    Ok(true)
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
