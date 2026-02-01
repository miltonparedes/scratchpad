use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

fn build_open_command(path: &Path, viewer: Option<&str>) -> Command {
    if let Some(viewer) = viewer {
        let mut cmd = Command::new(viewer);
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

pub fn open_path_blocking(path: &Path, viewer: Option<&str>) -> Result<()> {
    let status = build_open_command(path, viewer)
        .status()
        .with_context(|| format!("Failed to open {}", path.display()))?;
    if !status.success() {
        return Err(anyhow!("Open command failed with status: {}", status));
    }
    Ok(())
}

pub fn open_path_nonblocking(path: &Path, viewer: Option<&str>) -> Result<()> {
    build_open_command(path, viewer)
        .spawn()
        .with_context(|| format!("Failed to open {}", path.display()))?;
    Ok(())
}
