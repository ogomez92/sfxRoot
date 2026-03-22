//! System utility commands.

use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_shell::ShellExt;

use crate::error::Result;

/// Open a file's location in the system file explorer.
#[tauri::command]
pub async fn open_in_explorer(app: AppHandle, path: String) -> Result<()> {
    let file_path = Path::new(&path);

    // Get the parent directory if it's a file (used on Linux)
    #[allow(unused_variables)]
    let dir_to_open = if file_path.is_file() {
        file_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone())
    } else {
        path.clone()
    };

    // Use the shell plugin to open the directory
    #[cfg(target_os = "macos")]
    {
        app.shell()
            .command("open")
            .args(["-R", &path])
            .spawn()
            .ok();
    }

    #[cfg(target_os = "windows")]
    {
        app.shell()
            .command("explorer")
            .args(["/select,", &path])
            .spawn()
            .ok();
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open for the directory
        app.shell()
            .command("xdg-open")
            .args([&dir_to_open])
            .spawn()
            .ok();
    }

    Ok(())
}

/// Copy text to the system clipboard.
#[tauri::command]
pub async fn copy_to_clipboard(app: AppHandle, text: String) -> Result<()> {
    app.clipboard().write_text(text).ok();
    Ok(())
}
