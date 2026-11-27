use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, ensure};

pub(crate) fn tool(project: &Path, name: &str) -> anyhow::Result<Command> {
    let path = std::path::absolute(project)?.join("node_modules/.bin").join(name);
    ensure!(
        path.exists(),
        "`{}` not found: run `npm install` in `{}`",
        path.display(),
        project.display()
    );
    Ok(Command::new(path))
}

pub(crate) fn run(cmd: &mut Command) -> anyhow::Result<()> {
    tracing::info!(command = ?cmd, "running");
    let status = cmd.status().with_context(|| format!("failed to run {cmd:?}"))?;
    ensure!(status.success(), "{cmd:?} failed with {status}");
    Ok(())
}
