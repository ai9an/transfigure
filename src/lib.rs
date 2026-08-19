pub mod cli;
pub mod config;
pub mod launcher;
pub mod paths;
pub mod runner;

use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

pub const CONFIG_VERSION: u32 = 1;
pub const APP_NAME: &str = "transfigure";

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("configuration in {path} is invalid: {source}")]
    InvalidConfig {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub enum RunResult {
    Success,
    ChildExit(i32),
}

pub fn execute(cli: cli::Cli) -> Result<RunResult> {
    use cli::Command;

    let paths = paths::AppPaths::discover()?;

    match cli.command {
        Command::Setup => {
            setup(&paths)?;
            Ok(RunResult::Success)
        }
        Command::Create {
            name,
            chain,
            definition,
        } => {
            config::validate_name(&name)?;
            let shortcut = config::Shortcut::parse(definition, chain)?;
            let mut store = config::Store::load(&paths.config_file)?;
            if store.find_name(&name).is_some() {
                return Err(Error::Message(format!(
                    "shortcut '{name}' already exists; use `transfigure update {name} -- ...`"
                )));
            }
            launcher::ensure_available(&paths, &name)?;
            store.shortcuts.insert(name.clone(), shortcut);
            store.save(&paths.config_file)?;
            if let Err(error) = launcher::sync(&paths, &name) {
                store.shortcuts.remove(&name);
                let _ = store.save(&paths.config_file);
                return Err(error);
            }
            println!("Created shortcut '{name}'.");
            Ok(RunResult::Success)
        }
        Command::Update {
            name,
            chain,
            definition,
        } => {
            let shortcut = config::Shortcut::parse(definition, chain)?;
            let mut store = config::Store::load(&paths.config_file)?;
            let stored_name = store
                .find_name(&name)
                .map(str::to_owned)
                .ok_or_else(|| Error::Message(format!("shortcut '{name}' does not exist")))?;
            launcher::ensure_available(&paths, &stored_name)?;
            store.shortcuts.insert(stored_name.clone(), shortcut);
            store.save(&paths.config_file)?;
            launcher::sync(&paths, &stored_name)?;
            println!("Updated shortcut '{stored_name}'.");
            Ok(RunResult::Success)
        }
        Command::List => {
            let store = config::Store::load(&paths.config_file)?;
            if store.shortcuts.is_empty() {
                println!("No shortcuts configured.");
            } else {
                for (name, shortcut) in store.shortcuts {
                    println!("{name}\t{}", shortcut.summary());
                }
            }
            Ok(RunResult::Success)
        }
        Command::Show { name } => {
            let store = config::Store::load(&paths.config_file)?;
            let (stored_name, shortcut) = store
                .find(&name)
                .ok_or_else(|| Error::Message(format!("shortcut '{name}' does not exist")))?;
            println!("{stored_name}");
            for (index, command) in shortcut.commands.iter().enumerate() {
                println!("  {}: {}", index + 1, command.display());
            }
            Ok(RunResult::Success)
        }
        Command::Remove { name } => {
            let mut store = config::Store::load(&paths.config_file)?;
            let stored_name = store
                .find_name(&name)
                .map(str::to_owned)
                .ok_or_else(|| Error::Message(format!("shortcut '{name}' does not exist")))?;
            launcher::ensure_available(&paths, &stored_name)?;
            store.shortcuts.remove(&stored_name);
            store.save(&paths.config_file)?;
            launcher::remove(&paths, &stored_name)?;
            println!("Removed shortcut '{stored_name}'.");
            Ok(RunResult::Success)
        }
        Command::Run { name, arguments } | Command::InternalRun { name, arguments } => {
            run_named(&paths, &name, arguments)
        }
    }
}

fn run_named(paths: &paths::AppPaths, name: &str, arguments: Vec<OsString>) -> Result<RunResult> {
    let store = config::Store::load(&paths.config_file)?;
    let (_, shortcut) = store
        .find(name)
        .ok_or_else(|| Error::Message(format!("shortcut '{name}' does not exist")))?;
    runner::run(shortcut, &arguments)
}

fn setup(paths: &paths::AppPaths) -> Result<()> {
    paths.ensure_directories()?;
    let store = config::Store::load(&paths.config_file)?;
    for name in store.shortcuts.keys() {
        launcher::ensure_available(paths, name)?;
        launcher::sync(paths, name)?;
    }

    println!("Transfigure bin directory: {}", paths.bin_dir.display());
    if paths.bin_is_on_path() {
        println!(
            "PATH is configured. {} shortcut(s) ready.",
            store.shortcuts.len()
        );
    } else {
        println!("PATH does not include the Transfigure bin directory.");
        println!("{}", paths.path_guidance());
    }
    Ok(())
}
