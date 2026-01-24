mod walk;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use stbl_core::assemble::assemble_site;
use stbl_core::model::DiagnosticLevel;

#[derive(Debug, Parser)]
#[command(name = "stbl")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan { articles_dir: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan { articles_dir } => run_scan(&articles_dir),
    }
}

fn run_scan(articles_dir: &PathBuf) -> Result<()> {
    let root = std::env::current_dir().context("failed to read current directory")?;
    let docs = walk::walk_content(&root, articles_dir)?;
    match assemble_site(docs) {
        Ok(site) => {
            println!("pages: {}", site.pages.len());
            println!("series: {}", site.series.len());
            Ok(())
        }
        Err(diagnostics) => {
            for diag in diagnostics {
                let label = match diag.level {
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::Error => "error",
                };
                if let Some(path) = diag.source_path {
                    eprintln!("{label}: {path}: {}", diag.message);
                } else {
                    eprintln!("{label}: {}", diag.message);
                }
            }
            std::process::exit(1);
        }
    }
}
