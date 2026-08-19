use clap::Parser;
use transfigure::{RunResult, cli::Cli};

fn main() {
    let cli = Cli::parse();
    match transfigure::execute(cli) {
        Ok(RunResult::Success) => {}
        Ok(RunResult::ChildExit(code)) => std::process::exit(code),
        Err(error) => {
            eprintln!("transfigure: {error}");
            std::process::exit(1);
        }
    }
}
