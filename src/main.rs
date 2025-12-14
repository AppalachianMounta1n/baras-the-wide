use clap::{Parser, Subcommand};
use std::io::Write;
use std::time::Instant;

use baras::app_state::AppState;
use baras::reader::{read_log_file, tail_log_file};
use baras::repl::readline;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), String> {
    let state = Arc::new(RwLock::new(AppState::new()));

    loop {
        let line = readline()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match respond(line, Arc::clone(&state)).await {
            Ok(quit) => {
                if quit {
                    break;
                }
            }
            Err(err) => {
                write!(std::io::stdout(), "{err}").map_err(|e| e.to_string())?;
                std::io::stdout().flush().map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

#[derive(Parser)]
#[command(version, about = "cli")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    ParseFile {
        #[arg(short, long)]
        path: String,
    },
    Stats,
    Exit,
    Config,
}

async fn respond(line: &str, state: Arc<RwLock<AppState>>) -> Result<bool, String> {
    let mut args = shlex::split(line).ok_or("error: Invalid quoting")?;
    args.insert(0, "baras".to_string());
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;

    match &cli.command {
        Some(Commands::ParseFile { path }) => {
            let mut s = state.write().await;
            s.set_active_file(path);
            let timer = Instant::now();
            let active_path = s.active_file.as_ref().expect("a");
            let (events, end_pos) =
                read_log_file(active_path).expect("failed to parse log file {path}");
            let ms = timer.elapsed().as_millis();
            {
                println!("parsed {} events in {}ms", events.len(), ms);
                s.current_byte = Some(end_pos);
                s.events = events.clone();
            }

            let state_clone = Arc::clone(&state);
            let resolved_path = s.active_file.clone().expect("invalid file path");
            drop(s);
            tokio::spawn(async move {
                tail_log_file(resolved_path, state_clone).await.ok();
            });

            println!("tailing started");
        }
        Some(Commands::Stats) => {
            let s = state.read().await;
            println!("total events: {}", s.events.len());
        }
        Some(Commands::Config) => println!("{}", state.read().await.config.log_directory),
        Some(Commands::Exit) => {
            write!(std::io::stdout(), "quitting...").map_err(|e| e.to_string())?;
            std::io::stdout().flush().map_err(|e| e.to_string())?;
            return Ok(true);
        }
        None => {}
    }
    Ok(false)
}
