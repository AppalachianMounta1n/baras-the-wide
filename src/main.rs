use clap::{Parser, Subcommand};
use std::io::Write;
use std::time::Instant;
use tokio::sync::mpsc;

use baras::reader::{read_log_file, tail_log_file};

#[tokio::main]
async fn main() -> Result<(), String> {
    loop {
        let line = readline()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match respond(line) {
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

fn respond(line: &str) -> Result<bool, String> {
    let mut args = shlex::split(line).ok_or("error: Invalid quoting")?;
    args.insert(0, "baras".to_string());
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    match &cli.command {
        Some(Commands::ParseFile { path }) => {
            let timer = Instant::now();
            let data = read_log_file(path).expect("failed to parse log file {path}");
            let ms = timer.elapsed().as_millis();
            println!("parsed {} events in {}ms", data.0.len(), ms);
        }
        Some(Commands::TailFile { path: _ }) => {
            println!("a");
        }
        Some(Commands::Exit) => {
            write!(std::io::stdout(), "quitting...").map_err(|e| e.to_string())?;
            std::io::stdout().flush().map_err(|e| e.to_string())?;
            return Ok(true);
        }
        None => {}
    }
    Ok(false)
}

#[derive(Parser)]
#[command(version, about = "test")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// test command
    ParseFile {
        #[arg(short, long)]
        path: String,
    },
    TailFile {
        #[arg(short, long)]
        path: String,
    },
    Exit,
}
fn readline() -> Result<String, String> {
    write!(std::io::stdout(), "$ ").map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer)
}
