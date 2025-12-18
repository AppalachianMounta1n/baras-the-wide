
#[tauri::command]
pub fn default_log_path() -> String {
    dirs::home_dir().unwrap().join("baras/test-log-files/combat_2025-12-10_18_12_15_087604.txt").to_string_lossy().to_string()
}
