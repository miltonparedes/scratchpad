use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

/// Split a command string into program and arguments.
/// e.g. `"code --wait"` → `("code", ["--wait"])`
fn split_command(command: &str) -> (&str, Vec<&str>) {
    let mut parts = command.split_whitespace();
    let program = parts.next().unwrap_or(command);
    let args: Vec<&str> = parts.collect();
    (program, args)
}

fn build_open_command(path: &Path, viewer: Option<&str>) -> Command {
    if let Some(viewer) = viewer {
        let (program, args) = split_command(viewer);
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.arg(path);
        cmd
    } else if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    } else if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", ""]);
        cmd.arg(path);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    }
}

/// Open a path with the system default or specified viewer (blocking)
pub fn open_path_blocking(path: &Path, viewer: Option<&str>) -> Result<()> {
    let status = build_open_command(path, viewer)
        .status()
        .with_context(|| format!("Failed to open {}", path.display()))?;
    if !status.success() {
        return Err(anyhow!("Open command failed with status: {status}"));
    }
    Ok(())
}

/// Open a path with the system default or specified viewer (non-blocking)
pub fn open_path_nonblocking(path: &Path, viewer: Option<&str>) -> Result<()> {
    build_open_command(path, viewer)
        .spawn()
        .with_context(|| format!("Failed to open {}", path.display()))?;
    Ok(())
}

/// Open a file with the specified editor (blocking, waits for editor to close)
pub fn open_with_editor(path: &Path, editor: Option<&str>) -> Result<()> {
    let editor = editor
        .map(String::from)
        .or_else(|| std::env::var("EDITOR").ok())
        .or_else(|| std::env::var("VISUAL").ok())
        .unwrap_or_else(|| "vi".to_string());

    let (program, args) = split_command(&editor);
    let status = Command::new(program)
        .args(args)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to open {} with {editor}", path.display()))?;

    if !status.success() {
        return Err(anyhow!("Editor exited with status: {status}"));
    }
    Ok(())
}

/// Open a file with the specified editor (non-blocking)
#[allow(dead_code)]
pub fn open_with_editor_nonblocking(path: &Path, editor: Option<&str>) -> Result<()> {
    let editor = editor
        .map(String::from)
        .or_else(|| std::env::var("EDITOR").ok())
        .or_else(|| std::env::var("VISUAL").ok())
        .unwrap_or_else(|| "vi".to_string());

    let (program, args) = split_command(&editor);
    Command::new(program)
        .args(args)
        .arg(path)
        .spawn()
        .with_context(|| format!("Failed to open {} with {editor}", path.display()))?;

    Ok(())
}

/// Open a folder with the system file manager
pub fn open_folder(path: &Path) -> Result<()> {
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).status()
    } else {
        Command::new("xdg-open").arg(path).status()
    }
    .with_context(|| format!("Failed to open folder {}", path.display()))?;

    if !status.success() {
        return Err(anyhow!("File manager exited with status: {status}"));
    }
    Ok(())
}

/// Open a folder with the system file manager (non-blocking)
pub fn open_folder_nonblocking(path: &Path) -> Result<()> {
    if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    }
    .with_context(|| format!("Failed to open folder {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_command_single_word() {
        let (program, args) = split_command("nvim");
        assert_eq!(program, "nvim");
        assert!(args.is_empty());
    }

    #[test]
    fn test_split_command_with_args() {
        let (program, args) = split_command("code --wait");
        assert_eq!(program, "code");
        assert_eq!(args, vec!["--wait"]);
    }

    #[test]
    fn test_split_command_multiple_args() {
        let (program, args) = split_command("bat --paging=always --style=numbers");
        assert_eq!(program, "bat");
        assert_eq!(args, vec!["--paging=always", "--style=numbers"]);
    }

    #[test]
    fn test_split_command_extra_whitespace() {
        let (program, args) = split_command("code   --wait   --new-window");
        assert_eq!(program, "code");
        assert_eq!(args, vec!["--wait", "--new-window"]);
    }
}
