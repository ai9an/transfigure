use std::ffi::OsString;
use std::process::Command;

use crate::config::Shortcut;
use crate::{Error, Result, RunResult};

pub fn run(shortcut: &Shortcut, runtime_arguments: &[OsString]) -> Result<RunResult> {
    for (index, command) in shortcut.commands.iter().enumerate() {
        let last = index + 1 == shortcut.commands.len();
        let mut child = Command::new(&command.program);
        child.args(&command.args);
        if last {
            child.args(runtime_arguments);
        }
        let status = child.status().map_err(|source| {
            Error::Message(format!(
                "could not start step {} (`{}`): {source}",
                index + 1,
                command.program
            ))
        })?;
        if !status.success() {
            let code = status.code().unwrap_or(1);
            if shortcut.commands.len() > 1 {
                eprintln!(
                    "transfigure: chain stopped at step {} (`{}`) with exit code {code}",
                    index + 1,
                    command.program
                );
            }
            return Ok(RunResult::ChildExit(code));
        }
    }
    Ok(RunResult::Success)
}
