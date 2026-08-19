use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, ExitStatus};

use tempfile::Builder;

use crate::config::{ArgumentTemplate, ShellKind, Shortcut, parse_argument_template};
use crate::{Error, Result, RunResult};

const ARGUMENT_ENV_PREFIX: &str = "TRANSFIGURE_RUNTIME_ARGUMENT_";

pub fn run(shortcut: &Shortcut, runtime_arguments: &[OsString]) -> Result<RunResult> {
    let plan = ArgumentPlan::for_shortcut(shortcut)?;
    plan.validate(runtime_arguments.len())?;
    match shortcut.shell {
        Some(shell) => run_shell(shortcut, runtime_arguments, &plan, shell),
        None => run_direct(shortcut, runtime_arguments, &plan),
    }
}

#[derive(Debug, Default)]
struct ArgumentPlan {
    uses_placeholders: bool,
    positional: BTreeSet<usize>,
    all: bool,
}

impl ArgumentPlan {
    fn for_shortcut(shortcut: &Shortcut) -> Result<Self> {
        let mut plan = Self::default();
        for command in &shortcut.commands {
            for argument in &command.args {
                match parse_argument_template(argument)? {
                    ArgumentTemplate::Literal(_) => {}
                    ArgumentTemplate::Positional(index) => {
                        plan.uses_placeholders = true;
                        plan.positional.insert(index);
                    }
                    ArgumentTemplate::All => {
                        plan.uses_placeholders = true;
                        plan.all = true;
                    }
                }
            }
        }
        Ok(plan)
    }

    fn validate(&self, argument_count: usize) -> Result<()> {
        if !self.uses_placeholders {
            return Ok(());
        }
        if let Some(index) = self
            .positional
            .iter()
            .find(|index| **index >= argument_count)
        {
            return Err(Error::Message(format!(
                "shortcut requires invocation argument {} for placeholder {{{}}}",
                index + 1,
                index + 1
            )));
        }
        if !self.all {
            for index in 0..argument_count {
                if !self.positional.contains(&index) {
                    return Err(Error::Message(format!(
                        "invocation argument {} is unused; add placeholder {{{}}} or {{*}} to the shortcut",
                        index + 1,
                        index + 1
                    )));
                }
            }
        }
        Ok(())
    }
}

fn run_direct(
    shortcut: &Shortcut,
    runtime_arguments: &[OsString],
    plan: &ArgumentPlan,
) -> Result<RunResult> {
    for (index, command) in shortcut.commands.iter().enumerate() {
        let program = literal_value(&command.program)?;
        let mut child = Command::new(program.as_ref());
        add_resolved_arguments(&mut child, &command.args, runtime_arguments)?;
        if !plan.uses_placeholders && index + 1 == shortcut.commands.len() {
            child.args(runtime_arguments);
        }
        let status = child.status().map_err(|source| {
            Error::Message(format!(
                "could not start step {} (`{}`): {source}",
                index + 1,
                command.program
            ))
        })?;
        if let Some(result) = failed_step(status, shortcut, index) {
            return Ok(result);
        }
    }
    Ok(RunResult::Success)
}

fn add_resolved_arguments(
    command: &mut Command,
    templates: &[String],
    runtime_arguments: &[OsString],
) -> Result<()> {
    for template in templates {
        match parse_argument_template(template)? {
            ArgumentTemplate::Literal(value) => {
                command.arg(value.as_ref());
            }
            ArgumentTemplate::Positional(index) => {
                command.arg(&runtime_arguments[index]);
            }
            ArgumentTemplate::All => {
                command.args(runtime_arguments);
            }
        }
    }
    Ok(())
}

fn literal_value(value: &str) -> Result<std::borrow::Cow<'_, str>> {
    match parse_argument_template(value)? {
        ArgumentTemplate::Literal(value) => Ok(value),
        _ => Err(Error::Message(
            "placeholders cannot be used as command program names".into(),
        )),
    }
}

fn failed_step(status: ExitStatus, shortcut: &Shortcut, index: usize) -> Option<RunResult> {
    if status.success() {
        return None;
    }
    let code = status.code().unwrap_or(1);
    if shortcut.commands.len() > 1 {
        eprintln!(
            "transfigure: chain stopped at step {} (`{}`) with exit code {code}",
            index + 1,
            shortcut.commands[index].program
        );
    }
    Some(RunResult::ChildExit(code))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedShell {
    Powershell(&'static str),
    Sh,
}

impl ResolvedShell {
    fn from_kind(kind: ShellKind) -> Self {
        match kind {
            ShellKind::Auto if cfg!(windows) => Self::Powershell("powershell.exe"),
            ShellKind::Auto => Self::Sh,
            ShellKind::Powershell if cfg!(windows) => Self::Powershell("powershell.exe"),
            ShellKind::Powershell | ShellKind::Pwsh => Self::Powershell("pwsh"),
            ShellKind::Sh => Self::Sh,
        }
    }
}

fn run_shell(
    shortcut: &Shortcut,
    runtime_arguments: &[OsString],
    plan: &ArgumentPlan,
    shell: ShellKind,
) -> Result<RunResult> {
    let shell = ResolvedShell::from_kind(shell);
    let script = match shell {
        ResolvedShell::Powershell(_) => powershell_script(shortcut, runtime_arguments.len(), plan)?,
        ResolvedShell::Sh => sh_script(shortcut, runtime_arguments.len(), plan)?,
    };
    let suffix = match shell {
        ResolvedShell::Powershell(_) => ".ps1",
        ResolvedShell::Sh => ".sh",
    };
    let mut script_file = Builder::new()
        .prefix("transfigure-")
        .suffix(suffix)
        .tempfile()
        .map_err(|source| Error::Message(format!("could not create shell script: {source}")))?;
    if matches!(shell, ResolvedShell::Powershell(_)) {
        script_file
            .write_all(b"\xEF\xBB\xBF")
            .map_err(|source| Error::Message(format!("could not write shell script: {source}")))?;
    }
    script_file
        .write_all(script.as_bytes())
        .and_then(|_| script_file.flush())
        .map_err(|source| Error::Message(format!("could not write shell script: {source}")))?;
    let script_path = script_file.into_temp_path();

    let mut child = match shell {
        ResolvedShell::Powershell(program) => {
            let mut command = Command::new(program);
            command.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ]);
            command.arg(&script_path);
            command
        }
        ResolvedShell::Sh => {
            let mut command = Command::new("sh");
            command.arg(&script_path);
            command
        }
    };
    for (index, argument) in runtime_arguments.iter().enumerate() {
        child.env(argument_environment_name(index), argument);
    }
    let status = child.status().map_err(|source| {
        Error::Message(format!(
            "could not start {} shell: {source}",
            shell_name(shell)
        ))
    })?;
    if status.success() {
        Ok(RunResult::Success)
    } else {
        Ok(RunResult::ChildExit(status.code().unwrap_or(1)))
    }
}

fn shell_name(shell: ResolvedShell) -> &'static str {
    match shell {
        ResolvedShell::Powershell(_) => "PowerShell",
        ResolvedShell::Sh => "POSIX",
    }
}

fn argument_environment_name(index: usize) -> String {
    format!("{ARGUMENT_ENV_PREFIX}{index}")
}

fn powershell_script(
    shortcut: &Shortcut,
    runtime_argument_count: usize,
    plan: &ArgumentPlan,
) -> Result<String> {
    let mut script = String::from(
        r#"$ErrorActionPreference = 'Stop'
trap { [Console]::Error.WriteLine("transfigure: shell step failed: $($_.Exception.Message)"); exit 1 }
"#,
    );
    for (index, command) in shortcut.commands.iter().enumerate() {
        script.push_str("$LASTEXITCODE = 0\n& ");
        script.push_str(&powershell_quote(literal_value(&command.program)?.as_ref()));
        append_powershell_arguments(
            &mut script,
            &command.args,
            runtime_argument_count,
            !plan.uses_placeholders && index + 1 == shortcut.commands.len(),
        )?;
        script.push_str("\n$tfSucceeded = $?\n$tfExitCode = $LASTEXITCODE\n");
        script.push_str("if (-not $tfSucceeded -or $tfExitCode -ne 0) {\n");
        script.push_str("  if ($tfExitCode -eq 0) { $tfExitCode = 1 }\n");
        let message = format!(
            "transfigure: shell chain stopped at step {} (`{}`) with exit code ",
            index + 1,
            command.program
        );
        script.push_str("  [Console]::Error.WriteLine(");
        script.push_str(&powershell_quote(&message));
        script.push_str(" + $tfExitCode)\n  exit $tfExitCode\n}\n");
    }
    Ok(script)
}

fn append_powershell_arguments(
    script: &mut String,
    templates: &[String],
    runtime_argument_count: usize,
    append_runtime_arguments: bool,
) -> Result<()> {
    for template in templates {
        match parse_argument_template(template)? {
            ArgumentTemplate::Literal(value) => {
                script.push(' ');
                if is_powershell_parameter(value.as_ref()) {
                    script.push_str(value.as_ref());
                } else {
                    script.push_str(&powershell_quote(value.as_ref()));
                }
            }
            ArgumentTemplate::Positional(index) => {
                script.push_str(" $env:");
                script.push_str(&argument_environment_name(index));
            }
            ArgumentTemplate::All => {
                append_powershell_runtime_arguments(script, runtime_argument_count);
            }
        }
    }
    if append_runtime_arguments {
        append_powershell_runtime_arguments(script, runtime_argument_count);
    }
    Ok(())
}

fn append_powershell_runtime_arguments(script: &mut String, count: usize) {
    for index in 0..count {
        script.push_str(" $env:");
        script.push_str(&argument_environment_name(index));
    }
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn is_powershell_parameter(value: &str) -> bool {
    let mut characters = value.strip_prefix('-').unwrap_or_default().chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic())
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn sh_script(
    shortcut: &Shortcut,
    runtime_argument_count: usize,
    plan: &ArgumentPlan,
) -> Result<String> {
    let mut script = String::new();
    for (index, command) in shortcut.commands.iter().enumerate() {
        script.push_str(&sh_quote(literal_value(&command.program)?.as_ref()));
        append_sh_arguments(
            &mut script,
            &command.args,
            runtime_argument_count,
            !plan.uses_placeholders && index + 1 == shortcut.commands.len(),
        )?;
        script.push_str(
            r#"
tf_status=$?
if [ "$tf_status" -ne 0 ]; then
  printf '%s\n' "#,
        );
        let message = format!(
            "transfigure: shell chain stopped at step {} (`{}`) with exit code ",
            index + 1,
            command.program
        );
        script.push_str(&sh_quote(&message));
        script.push_str(
            r#""$tf_status" >&2
  exit "$tf_status"
fi
"#,
        );
    }
    Ok(script)
}

fn append_sh_arguments(
    script: &mut String,
    templates: &[String],
    runtime_argument_count: usize,
    append_runtime_arguments: bool,
) -> Result<()> {
    for template in templates {
        match parse_argument_template(template)? {
            ArgumentTemplate::Literal(value) => {
                script.push(' ');
                script.push_str(&sh_quote(value.as_ref()));
            }
            ArgumentTemplate::Positional(index) => append_sh_runtime_argument(script, index),
            ArgumentTemplate::All => append_sh_runtime_arguments(script, runtime_argument_count),
        }
    }
    if append_runtime_arguments {
        append_sh_runtime_arguments(script, runtime_argument_count);
    }
    Ok(())
}

fn append_sh_runtime_arguments(script: &mut String, count: usize) {
    for index in 0..count {
        append_sh_runtime_argument(script, index);
    }
}

fn append_sh_runtime_argument(script: &mut String, index: usize) {
    script.push_str(r#" "${"#);
    script.push_str(&argument_environment_name(index));
    script.push_str(r#"}""#);
}

fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}
