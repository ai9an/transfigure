use std::ffi::OsString;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "transfigure",
    version,
    about = "Turn long commands into short, reusable commands",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create the per-user directories and restore managed launchers.
    Setup,
    /// Create a new shortcut.
    Create {
        /// Portable command name to create.
        name: String,
        /// Interpret `--then` tokens as boundaries between commands.
        #[arg(long)]
        chain: bool,
        /// Command definition. Place it after `--`.
        #[arg(last = true, required = true, allow_hyphen_values = true)]
        definition: Vec<String>,
    },
    /// Replace an existing shortcut definition.
    Update {
        name: String,
        #[arg(long)]
        chain: bool,
        #[arg(last = true, required = true, allow_hyphen_values = true)]
        definition: Vec<String>,
    },
    /// List configured shortcuts.
    List,
    /// Show every command in a shortcut.
    Show { name: String },
    /// Remove a shortcut and its managed launcher.
    Remove { name: String },
    /// Run a shortcut without using its generated launcher.
    Run {
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<OsString>,
    },
    #[command(name = "__run", hide = true)]
    InternalRun {
        name: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<OsString>,
    },
}
