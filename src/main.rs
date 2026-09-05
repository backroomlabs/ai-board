mod acp;
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

    /// Commands for working with tasks.
    Task {
        #[command(subcommand)]
        cmd: TaskCmd,
    },

    /// Serve the live editable board UI over HTTP.
    Serve {
        #[arg(long, default_value_t = 4141)]
        port: u16,
    },

    /// One-shot ACP prompt turn (JSONL on stdout).
    Acp {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        cwd: Option<std::path::PathBuf>,
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum TaskCmd {
    /// Add a task to a ticket.
    Add {
        #[arg(long)]
        ticket_id: i64,
        #[arg(long)]
        title: String,
        #[arg(long)]
        work_type: String,
        #[arg(long)]
        objective: String,
        #[arg(long)]
        criteria: String,
        #[arg(long)]
        context: Option<String>,
    },
    /// List tasks for a ticket (JSON array).
    List {
        #[arg(long)]
        ticket_id: i64,
    },
    /// Show a task (JSON).
    Show { task_id: i64 },
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
        Cmd::Task { cmd } => match cmd {
            TaskCmd::Add {
                ticket_id,
                title,
                work_type,
                objective,
                criteria,
                context,
            } => commands::add_task(
                ticket_id,
                &title,
                &work_type,
                &objective,
                &criteria,
                context.as_deref(),
            ),
            TaskCmd::List { ticket_id } => commands::list_tasks(ticket_id),
            TaskCmd::Show { task_id } => commands::show_task(task_id),
        },
        Cmd::Serve { .. } => unreachable!("serve is handled in main"),
        Cmd::Acp { .. } => unreachable!("acp is handled in main"),
    }
}

fn run_acp(
    agent_id: String,
    prompt: String,
    cwd: Option<std::path::PathBuf>,
    config: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let workspace = std::env::current_dir()?;
    let agents = acp::config::load_agents(home.as_deref(), &workspace, config.as_deref())?;
    let spec = acp::config::get_agent(&agents, &agent_id)?;
    let cwd = match cwd {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let cwd = if cwd.is_absolute() {
        cwd
    } else {
        std::env::current_dir()?.join(cwd)
    };
    acp::run::run(spec, &prompt, &cwd, &mut acp::event::JsonlStdoutSink)
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
    if let Cmd::Acp {
        agent,
        prompt,
        cwd,
        config,
    } = cli.command
    {
        if let Err(error) = run_acp(agent, prompt, cwd, config) {
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
