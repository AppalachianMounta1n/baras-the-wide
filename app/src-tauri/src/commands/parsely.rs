//! Parsely.io upload commands

use std::io::Write;
use std::path::PathBuf;

use encoding_rs::WINDOWS_1252;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::multipart::{Form, Part};
use tauri::State;

use crate::service::ServiceHandle;

const PARSELY_URL: &str = "https://parsely.io/api/upload2";
const USER_AGENT: &str = "BARAS v0.1.0";

/// Response from Parsely upload
#[derive(Debug, serde::Serialize)]
pub struct ParselyUploadResponse {
    pub success: bool,
    pub link: Option<String>,
    pub error: Option<String>,
}

/// Upload a log file to Parsely.io
#[tauri::command]
pub async fn upload_to_parsely(
    path: PathBuf,
    handle: State<'_, ServiceHandle>,
) -> Result<ParselyUploadResponse, String> {
    // Read the log file
    let log_content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read log file: {}", e))?;

    // Encode as Windows-1252 and gzip compress
    let (encoded, _, _) = WINDOWS_1252.encode(&log_content);
    let compressed = gzip_compress(&encoded)
        .map_err(|e| format!("Failed to compress: {}", e))?;

    // Get filename for the upload
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("combat.txt")
        .to_string();

    // Build multipart form
    let file_part = Part::bytes(compressed)
        .file_name(filename)
        .mime_str("text/html")
        .map_err(|e| format!("Failed to create file part: {}", e))?;

    let mut form = Form::new()
        .part("file", file_part)
        .text("public", "1");

    // Add credentials if configured
    let config = handle.config().await;
    if !config.parsely.username.is_empty() {
        form = form.text("username", config.parsely.username.clone());
        form = form.text("password", config.parsely.password.clone());
        if !config.parsely.guild.is_empty() {
            form = form.text("guild", config.parsely.guild.clone());
        }
    }

    // Send request
    let client = reqwest::Client::new();
    let response = client
        .post(PARSELY_URL)
        .header("User-Agent", USER_AGENT)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| format!("Upload failed: {}", e))?;

    let response_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Parse XML response
    parse_parsely_response(&response_text)
}

/// Gzip compress data
fn gzip_compress(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Parse Parsely XML response
fn parse_parsely_response(xml: &str) -> Result<ParselyUploadResponse, String> {
    // Check for errors
    if xml.contains("NOT OK") || xml.contains("error") {
        return Ok(ParselyUploadResponse {
            success: false,
            link: None,
            error: Some(xml.to_string()),
        });
    }

    // Extract link from <file> element
    // Simple parsing without XML library: find <file>...</file>
    if let Some(start) = xml.find("<file>") {
        if let Some(end) = xml.find("</file>") {
            let link = &xml[start + 6..end];
            return Ok(ParselyUploadResponse {
                success: true,
                link: Some(link.to_string()),
                error: None,
            });
        }
    }

    Ok(ParselyUploadResponse {
        success: false,
        link: None,
        error: Some(format!("Unexpected response: {}", xml)),
    })
}
