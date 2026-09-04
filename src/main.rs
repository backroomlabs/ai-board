mod commands;
mod db;
mod models;
mod serve;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "abd", about = "AI Board — SQLite-backed orchestration CLI")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Initialize the board database (idempotent).
    Init,

    /// Claim the oldest queued ticket (queued -> implementing).
    Next {
        #[arg(long)]
        spec_id: Option<i64>,
    },

    /// Update a ticket's status / context / attempts.
    Update {
        ticket_id: i64,
        #[arg(long)]
        status: String,
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        bump_attempts: bool,
    },

    /// Return the stranded needs_human ticket, if any (JSON).
    NeedsHuman {
        #[arg(long)]
        spec_id: Option<i64>,
    },

    /// Commands for working with specs.
    Spec {
        #[command(subcommand)]
        cmd: SpecCmd,
    },

    /// Commands for working with tickets.
    Ticket {
        #[command(subcommand)]
        cmd: TicketCmd,
    },

    /// Serve the live editable board UI over HTTP.
    Serve {
        #[arg(long, default_value_t = 4141)]
        port: u16,
    },
}

#[derive(Subcommand)]
enum TicketCmd {
    /// Add a ticket to a spec.
    Add {
        #[arg(long)]
        spec_id: i64,
        #[arg(long)]
        title: String,
        #[arg(long)]
        description: String,
        /// JSON array of prose definitions of done, e.g. '["towers attack in range"]'
        #[arg(long)]
        dod: String,
    },
    /// Show a ticket (JSON).
    Show { ticket_id: i64 },
    /// List tickets for a spec (JSON array).
    List {
        #[arg(long)]
        spec_id: i64,
    },
}

#[derive(Subcommand)]
enum SpecCmd {
    /// Add a spec from a content file or stdin.
    Add {
        #[arg(long)]
        title: String,
        #[arg(long, conflicts_with = "stdin")]
        file: Option<std::path::PathBuf>,
        #[arg(long)]
        stdin: bool,
    },
    /// List all specs as JSON, newest first.
    List,
    /// Print a spec's raw content.
    Get { spec_id: i64 },
}

fn run(cli: Cli) -> Result<Value> {
    match cli.command {
        Cmd::Init => commands::init(),
        Cmd::Next { spec_id } => commands::next(spec_id),
        Cmd::Update {
            ticket_id,
            status,
            context,
            bump_attempts,
        } => commands::update(ticket_id, &status, context.as_deref(), bump_attempts),
        Cmd::NeedsHuman { spec_id } => commands::needs_human(spec_id),
        Cmd::Spec { cmd } => match cmd {
            SpecCmd::Add { title, file, stdin } => {
                commands::add_spec(&title, file.as_deref(), stdin)
            }
            SpecCmd::List => commands::specs(),
            SpecCmd::Get { spec_id } => commands::get_spec(spec_id),
        },
        Cmd::Ticket { cmd } => match cmd {
            TicketCmd::Add {
                spec_id,
                title,
                description,
                dod,
            } => commands::add_ticket(spec_id, &title, &description, &dod),
            TicketCmd::Show { ticket_id } => commands::show(ticket_id),
            TicketCmd::List { spec_id } => commands::list(spec_id),
        },
        Cmd::Serve { .. } => unreachable!("serve is handled in main"),
    }
}

fn exit_with_error(error: impl std::fmt::Display) -> ! {
    let envelope = serde_json::json!({"ok": false, "error": error.to_string()});
    eprintln!("{}", serde_json::to_string(&envelope).unwrap());
    std::process::exit(1);
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = db::preflight() {
        exit_with_error(error);
    }
    // `serve` runs forever and emits no JSON — handle it before the JSON printer.
    if let Cmd::Serve { port } = &cli.command {
        if let Err(error) = serve::serve(*port) {
            exit_with_error(error);
        }
        return;
    }
    match run(cli) {
        Ok(value) => {
            if let Some(raw) = value.get("__raw__").and_then(|v| v.as_str()) {
                print!("{raw}");
            } else {
                println!("{}", serde_json::to_string(&value).unwrap());
            }
        }
        Err(error) => exit_with_error(error),
    }
}
