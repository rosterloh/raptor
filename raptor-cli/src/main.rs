//! `raptorctl`: CLI + TUI client for raptor's Management API. Pure client —
//! every request goes over HTTP Basic auth to a running `raptor serve`
//! (see `raptor/src/auth/mgmt.rs`). All DTOs come from `raptor-api-types`.

mod api;
mod client;
mod commands;
mod config;
mod hash;
mod print;
mod tui;

use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "raptorctl", version, about = "CLI and TUI for raptor")]
struct Cli {
    /// Server URL, overrides the saved config and $RAPTOR_URL.
    #[arg(long, global = true)]
    url: Option<String>,
    /// Username, overrides the saved config and $RAPTOR_USER.
    #[arg(long, global = true)]
    user: Option<String>,
    /// Print raw server JSON instead of a table.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Save server URL and credentials to ~/.config/raptor/cli.toml
    Login {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        user: Option<String>,
    },
    /// Remove the saved config file
    Logout,
    #[command(subcommand)]
    Target(commands::TargetCmd),
    #[command(subcommand)]
    Module(commands::ModuleCmd),
    #[command(subcommand)]
    Artifact(commands::ArtifactCmd),
    #[command(subcommand)]
    Ds(commands::DsCmd),
    #[command(subcommand)]
    Action(commands::ActionCmd),
    /// Create a software module, upload its artifact, and create a
    /// distribution set wrapping it — one call for the common single-module
    /// release, threaded by id instead of `--json` + `jq`.
    Publish {
        file: std::path::PathBuf,
        #[arg(long)]
        version: String,
        /// Defaults to the filename with a trailing `-<version>.swu` stripped.
        #[arg(long)]
        name: Option<String>,
        /// os | application | firmware — used for both the module and the
        /// distribution set it's wrapped in.
        #[arg(long = "type", default_value = "os")]
        module_type: String,
        #[arg(long)]
        vendor: Option<String>,
    },
    /// Fleet-wide statistics
    Status,
    /// Interactive terminal dashboard
    Tui {
        /// Poll interval in seconds; 0 disables auto-refresh.
        #[arg(long, default_value_t = 5)]
        refresh: u64,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Cmd::Login { url, user } = cli.cmd {
        if let Err(e) = commands::login(url, user).await {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    if let Cmd::Logout = cli.cmd {
        if let Err(e) = Config::delete() {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
        println!("logged out");
        return;
    }

    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    };
    let resolved = match cfg.resolve(cli.url, cli.user) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    };
    let client = client::Client::new(&resolved);

    let result = match cli.cmd {
        Cmd::Login { .. } | Cmd::Logout => unreachable!("handled above"),
        Cmd::Target(c) => commands::target(&client, c, cli.json).await,
        Cmd::Module(c) => commands::module(&client, c, cli.json).await,
        Cmd::Artifact(c) => commands::artifact(&client, c, cli.json).await,
        Cmd::Ds(c) => commands::ds(&client, c, cli.json).await,
        Cmd::Action(c) => commands::action(&client, c, cli.json).await,
        Cmd::Publish {
            file,
            version,
            name,
            module_type,
            vendor,
        } => {
            commands::publish(
                &client,
                commands::PublishArgs {
                    file,
                    version,
                    name,
                    module_type,
                    vendor,
                },
                cli.json,
            )
            .await
        }
        Cmd::Status => commands::status(&client, cli.json).await,
        Cmd::Tui { refresh } => tui::run(client, refresh).await,
    };

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        let unauthorized = e.to_string().contains("unauthorized");
        std::process::exit(if unauthorized { 2 } else { 1 });
    }
}
