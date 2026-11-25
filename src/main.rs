use clap::Parser;
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uiaprobe::cli::{parse_hwnd, Cli, Commands, DumpArgs};
use uiaprobe::coords::init_dpi_awareness;
use uiaprobe::model::DumpOutput;
use uiaprobe::uia::browser::ChromiumDomExtractor;
use uiaprobe::uia::standard::{extract_window_nodes, ExtractOptions};
use uiaprobe::uia::{foreground_hwnd, read_window_info, UiaSession};
use uiaprobe::{ProbeError, Result};

fn run(args: DumpArgs) -> Result<()> {
    let start = Instant::now();
    let hwnd = match args.hwnd.as_deref() {
        Some(raw) => parse_hwnd(raw)?,
        None => foreground_hwnd()?,
    };

    let window = read_window_info(hwnd)?;
    let session = UiaSession::new()?;
    let opts = ExtractOptions {
        depth: args.depth,
        max_nodes: args.max_nodes,
        dom: args.dom,
        compact: args.compact,
    };
    let browser = ChromiumDomExtractor;
    let extracted = extract_window_nodes(&session, hwnd, &opts, &browser)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let output = DumpOutput {
        timestamp,
        elapsed_ms,
        truncated: extracted.truncated,
        window,
        nodes: extracted.nodes,
    };

    let json = if args.json && !args.compact {
        serde_json::to_string_pretty(&output)
    } else {
        serde_json::to_string(&output)
    }
    .map_err(|e| ProbeError::Serialization(e.to_string()))?;

    println!("{json}");
    Ok(())
}

fn main() -> ExitCode {
    // Must be the very first call before any Win32/UIA API usage.
    init_dpi_awareness();

    let cli = Cli::parse();
    match cli.command {
        Commands::Dump(args) => {
            if let Err(err) = run(args) {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
