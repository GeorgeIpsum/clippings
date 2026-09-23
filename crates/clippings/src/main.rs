use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::report::scan_report;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "clippings", version, about = "Fast TODO scanning for VS Code")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan directories and print the todos found.
    Scan {
        /// Directories to scan.
        #[arg(required = true)]
        roots: Vec<PathBuf>,
        /// JSON file with `clippings.*` settings in the extension's configuration shape.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Include hidden files and directories.
        #[arg(long)]
        hidden: bool,
        /// Do not honour .gitignore, .ignore or .rgignore files.
        #[arg(long)]
        no_ignore: bool,
        /// Print a JSON report instead of one line per todo.
        #[arg(long)]
        json: bool,
    },
    /// Print version, target and protocol version as JSON, for the extension's binary check.
    Probe,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Scan {
            roots,
            config,
            hidden,
            no_ignore,
            json,
        } => {
            let mut cfg: CoreConfig = match config {
                Some(p) => serde_json::from_slice(
                    &std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?,
                )?,
                None => CoreConfig::default(),
            };
            cfg.include_hidden_files |= hidden;
            cfg.respect_ignore_files &= !no_ignore;
            let roots: Vec<PathBuf> = roots
                .iter()
                .map(|r| dunce::canonicalize(r).with_context(|| format!("root {}", r.display())))
                .collect::<Result<_>>()?;
            let report = scan_report(&roots, &cfg, Arc::new(NativeFs))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for f in &report.files {
                    for t in &f.todos {
                        println!(
                            "{}:{}:{}: {} {}",
                            f.path,
                            t.start.line + 1,
                            t.start.character + 1,
                            t.tag,
                            t.after
                        );
                    }
                }
            }
        }
        Command::Probe => {
            let info = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
                "protocolVersion": clippings_core::PROTOCOL_VERSION,
            });
            println!("{info}");
        }
    }
    Ok(())
}
