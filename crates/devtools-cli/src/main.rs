use clap::{Parser, Subcommand};
use cove_agent::transport::{self, Handler};
use cove_devtools_cli::{Client, definitions};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(
    name = "cove-devtools",
    version,
    about = "Cove DevTools controller, Inspector and MCP"
)]
struct Cli {
    #[arg(long, global = true)]
    endpoint: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the local controller and serve the Inspector.
    Serve {
        #[arg(long, default_value_t = 9229)]
        port: u16,
        /// Directory produced by the browser build.
        #[arg(long)]
        web_root: Option<PathBuf>,
        /// Attach platform,http://127.0.0.1:port,session-id. Repeat for other devices or tabs.
        #[arg(long)]
        webdriver: Vec<String>,
    },
    /// Connect an MCP client over stdin/stdout.
    Mcp,
    /// List command schemas.
    Tools,
    /// Call the controller and print the result.
    Call {
        name: String,
        #[arg(default_value = "{}")]
        arguments: String,
        /// Save the original lossless PNG.
        #[arg(long)]
        image: Option<PathBuf>,
    },
}

fn run() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    let endpoint = cli.endpoint;
    let token = std::env::var("COVE_DEVTOOLS_TOKEN").ok();
    match cli.command {
        Command::Serve {
            port,
            web_root,
            webdriver,
        } => {
            let token = token
                .filter(|token| !token.is_empty())
                .map_or_else(cove_devtools_cli::random_id, Ok)?;
            if token.len() > 256
                || !token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
            {
                return Err("COVE_DEVTOOLS_TOKEN must contain only letters, digits, hyphens and underscores.".into());
            }
            cove_devtools_cli::serve(port, token, web_root, webdriver)?;
        }
        Command::Mcp => {
            // Start MCP even before a controller is paired, so tools/list works.
            let server = cove_devtools_cli::mcp_server(Arc::new(Client::new(endpoint, token)?))?;
            let handler: Handler = Arc::new(move |request| server.respond(request));
            transport::serve_stdio(handler)?;
        }
        Command::Tools => {
            let tools: Vec<_> = definitions()
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
            let client = Arc::new(Client::new(endpoint, token)?);
            let arguments: Value = serde_json::from_str(&arguments)?;
            let result = cove_devtools_cli::toolbox(client.clone())?.execute(&name, &arguments)?;
            let mut value = result.value().unwrap_or(Value::Null);
            if let Some(path) = image
                && !result.is_error
            {
                let resource = value["resource"]
                    .as_str()
                    .ok_or("Command returned no capture.")?;
                std::fs::write(&path, client.image(resource)?)?;
                value["saved"] = json!(path);
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
