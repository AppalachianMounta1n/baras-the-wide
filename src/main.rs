use clap::{Parser, Subcommand};
use std::time::Instant;

use baras::reader::{read_log_file, tail_log_file};

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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::ParseFile { path }) => {
            let timer = Instant::now();
            let data = read_log_file(path).expect("failed to parse log file {path}");
            let ms = timer.elapsed().as_millis();
            println!("parsed {} events in {}ms", data.0.len(), ms);
        }
        Some(Commands::TailFile { path }) => {
            tail_log_file(path, 1)
                .await
                .expect("failed to tail log file");
        }
        None => {}
    }
}
