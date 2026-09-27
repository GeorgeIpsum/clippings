use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clippings_core::config::CoreConfig;
use clippings_core::fs::NativeFs;
use clippings_core::report::scan_report;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::filter::LevelFilter;

mod watch;

/// mimalloc on Windows, the system allocator elsewhere (spec 8.2).
#[cfg(windows)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Writes `line` to `out`, terminated with a newline. Standard Unix
/// behaviour for a pipe whose reader has gone away (`head`, a killed
/// watcher client): stop quietly with exit code 0 instead of reporting a
/// broken pipe as an error.
fn write_line(out: &mut impl Write, line: &str) -> Result<()> {
    if let Err(e) = writeln!(out, "{line}") {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(e.into());
    }
    Ok(())
}

/// Flushes `out`, with the same broken-pipe handling as [`write_line`].
fn flush(out: &mut impl Write) -> Result<()> {
    if let Err(e) = out.flush() {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(e.into());
    }
    Ok(())
}

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
    /// Run the language server over stdin and stdout.
    Lsp,
    /// Scan, then print a JSON line for every file whose todos change.
    Watch {
        /// Directories to watch.
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
    },
}

fn load_config(config: Option<PathBuf>, hidden: bool, no_ignore: bool) -> Result<CoreConfig> {
    let mut cfg: CoreConfig = match config {
        Some(p) => serde_json::from_slice(
            &std::fs::read(&p).with_context(|| format!("reading {}", p.display()))?,
        )?,
        None => CoreConfig::default(),
    };
    cfg.include_hidden_files |= hidden;
    cfg.respect_ignore_files &= !no_ignore;
    Ok(cfg)
}

fn canonical(roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    roots
        .iter()
        .map(|r| dunce::canonicalize(r).with_context(|| format!("root {}", r.display())))
        .collect()
}

/// The level named by a `CLIPPINGS_LOG` value: `error`, `warn`, `info`,
/// `debug`, `trace` or `off`, case-insensitively. Anything else, including
/// an empty or whitespace-only value, is `info` — note that
/// `LevelFilter::from_str("")` returns `Ok(ERROR)`, not a default, so this
/// cannot delegate to `FromStr`.
fn parse_log_level(v: &str) -> LevelFilter {
    match v.trim().to_ascii_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        "off" => LevelFilter::OFF,
        _ => LevelFilter::INFO,
    }
}

/// Sends log records to stderr at the level named by `CLIPPINGS_LOG`
/// (`error`, `warn`, `info`, `debug`, `trace` or `off`), `info` by default.
/// The extension shows stderr in its output channel (spec 10.3); stdout
/// carries JSON-RPC only.
fn init_logging() {
    let level = std::env::var("CLIPPINGS_LOG")
        .map(|v| parse_log_level(&v))
        .unwrap_or(LevelFilter::INFO);
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(level)
        .with_target(false)
        .init();
}

fn main() -> Result<()> {
    init_logging();
    match Cli::parse().command {
        Command::Scan {
            roots,
            config,
            hidden,
            no_ignore,
            json,
        } => {
            let cfg = load_config(config, hidden, no_ignore)?;
            let roots = canonical(&roots)?;
            let report = scan_report(&roots, &cfg, Arc::new(NativeFs))?;
            let mut out = std::io::stdout();
            if json {
                write_line(&mut out, &serde_json::to_string_pretty(&report)?)?;
            } else {
                for f in &report.files {
                    for t in &f.todos {
                        write_line(
                            &mut out,
                            &format!(
                                "{}:{}:{}: {} {}",
                                f.path,
                                t.start.line + 1,
                                t.start.character + 1,
                                t.tag,
                                t.after
                            ),
                        )?;
                    }
                }
            }
            flush(&mut out)?;
        }
        Command::Probe => {
            let info = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
                "protocolVersion": clippings_core::PROTOCOL_VERSION,
            });
            println!("{info}");
        }
        Command::Lsp => {
            clippings_core::server::main_loop::run_stdio().map_err(anyhow::Error::msg)?
        }
        Command::Watch {
            roots,
            config,
            hidden,
            no_ignore,
        } => watch::run(canonical(&roots)?, load_config(config, hidden, no_ignore)?)?,
    }
    Ok(())
}
