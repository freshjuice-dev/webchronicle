mod cli;
mod config;
mod scraper;
mod ledger;
mod builder;
mod server;
mod assets;
mod templates;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init => cli::init(),
        Command::Scrape => cli::run_scrape(),
        Command::Build => cli::run_build(),
        Command::Serve { port } => cli::run_serve(port),
        Command::Clean => cli::run_clean(),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}