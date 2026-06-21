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

    /// Create a design from a markdown spec file or stdin.
    CreateDesign {
        #[arg(long)]
        title: String,
        #[arg(long, conflicts_with = "stdin")]
        file: Option<std::path::PathBuf>,
        #[arg(long)]
        stdin: bool,
    },

    /// Add a ticket to a design.
    AddTicket {
        #[arg(long)]
        design: i64,
        #[arg(long)]
        title: String,
        #[arg(long)]
        spec: String,
        /// JSON array of checkable criteria, e.g. '["cargo test => PASS"]'
        #[arg(long)]
        criteria: String,
    },

    /// Claim the oldest queued ticket (queued -> implementing).
    Next {
        #[arg(long)]
        design: Option<i64>,
    },

    /// Show a full ticket including its parent design_md (JSON).
    Show { ticket_id: i64 },

    /// List tickets for a design (JSON array).
    List {
        #[arg(long)]
        design: i64,
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
        design: Option<i64>,
    },

    /// Subcommands for working with designs.
    Design {
        #[command(subcommand)]
        cmd: DesignCmd,
    },

    /// Serve the read-only live board UI over HTTP.
    Serve {
        #[arg(long, default_value_t = 4141)]
        port: u16,
    },
}

#[derive(Subcommand)]
enum DesignCmd {
    /// List all designs (JSON array, newest first).
    List,
    /// Print a design's raw markdown (NOT JSON; for humans).
    Show { design_id: i64 },
}

fn run(cli: Cli) -> Result<Value> {
    match cli.command {
        Cmd::Init => commands::init(),
        Cmd::CreateDesign { title, file, stdin } => {
            commands::create_design(&title, file.as_deref(), stdin)
        }
        Cmd::AddTicket {
            design,
            title,
            spec,
            criteria,
        } => commands::add_ticket(design, &title, &spec, &criteria),
        Cmd::Next { design } => commands::next(design),
        Cmd::Show { ticket_id } => commands::show(ticket_id),
        Cmd::List { design } => commands::list(design),
        Cmd::Update {
            ticket_id,
            status,
            context,
            bump_attempts,
        } => commands::update(ticket_id, &status, context.as_deref(), bump_attempts),
        Cmd::NeedsHuman { design } => commands::needs_human(design),
        Cmd::Design { cmd } => match cmd {
            DesignCmd::List => commands::designs(),
            DesignCmd::Show { design_id } => commands::design(design_id),
        },
        Cmd::Serve { .. } => unreachable!("serve is handled in main"),
    }
}

fn main() {
    let cli = Cli::parse();
    // `serve` runs forever and emits no JSON — handle it before the JSON printer.
    if let Cmd::Serve { port } = &cli.command {
        if let Err(err) = serve::serve(*port) {
            eprintln!("{}", serde_json::json!({"ok": false, "error": err.to_string()}));
            std::process::exit(1);
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
        Err(err) => {
            let envelope = serde_json::json!({"ok": false, "error": err.to_string()});
            eprintln!("{}", serde_json::to_string(&envelope).unwrap());
            std::process::exit(1);
        }
    }
}
