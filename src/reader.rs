use crate::{CombatEvent, parse_line};
use memchr::memchr_iter;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs;
use std::io::Result;
use std::io::SeekFrom;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

pub fn read_log_file<P: AsRef<Path>>(path: P) -> Result<(Vec<CombatEvent>, u64)> {
    let file = fs::File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let bytes = mmap.as_ref();
    let end_pos = bytes.len() as u64;

    // Find all line boundaries
    let mut line_ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for end in memchr_iter(b'\n', bytes) {
        if end > start {
            line_ranges.push((start, end));
        }
        start = end + 1;
    }
    if start < bytes.len() {
        line_ranges.push((start, bytes.len()));
    }

    let events: Vec<CombatEvent> = line_ranges
        .par_iter()
        .enumerate()
        .filter_map(|(idx, &(start, end))| {
            let line = unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) };
            parse_line(idx as u64 + 1, line)
        })
        .collect();

    Ok((events, end_pos))
}

pub async fn tail_log_file<P: AsRef<Path>>(
    path: P,
    start_index: u64,
    start_byte: u64,
    tx: mpsc::Sender<CombatEvent>,
) -> Result<()> {
    let file = File::open(&path).await?;
    let mut reader = BufReader::new(file);
    let mut idx = start_index;

    reader.seek(SeekFrom::Start(start_byte)).await?;

    let mut line = String::new();

    loop {
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // No new data, wait briefly before checking again
                sleep(Duration::from_millis(100)).await;
            }
            Ok(_) => {
                idx += 1;
                if let Some(parsed_event) = parse_line(idx, &line) {
                    tx.send(parsed_event).await.ok();
                };
                print!("Parsed line {}", idx);
                line.clear();
            }
            Err(e) => {
                eprintln!("Error reading file line: {} ", e);
                break;
            }
        }
    }

    Ok(())
}
