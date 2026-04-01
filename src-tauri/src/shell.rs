use anyhow::{anyhow, Result};
use std::{path::Path, process::Command};

pub fn open_file(path: &str) -> Result<()> {
    run_platform_command(path, false)
}

pub fn reveal_file(path: &str) -> Result<()> {
    run_platform_command(path, true)
}

fn run_platform_command(path: &str, reveal: bool) -> Result<()> {
    if cfg!(target_os = "macos") {
        let status = if reveal {
            Command::new("open").arg("-R").arg(path).status()?
        } else {
            Command::new("open").arg(path).status()?
        };
        if status.success() {
            return Ok(());
        }
    } else if cfg!(target_os = "windows") {
        let status = if reveal {
            Command::new("explorer")
                .arg("/select,")
                .arg(path)
                .status()?
        } else {
            Command::new("cmd")
                .args(["/C", "start", "", path])
                .status()?
        };
        if status.success() {
            return Ok(());
        }
    } else if cfg!(target_os = "linux") {
        let status = if reveal {
            let parent = Path::new(path)
                .parent()
                .ok_or_else(|| anyhow!("could not reveal file parent directory"))?;
            Command::new("xdg-open").arg(parent).status()?
        } else {
            Command::new("xdg-open").arg(path).status()?
        };
        if status.success() {
            return Ok(());
        }
    }

    Err(anyhow!("failed to run platform file action"))
}
