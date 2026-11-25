use clap::Parser;
use std::fs;
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uiaprobe::cli::{parse_hwnd, Cli, Commands, DumpArgs};
use uiaprobe::coords::init_dpi_awareness;
use uiaprobe::diff::{compute_diff, DiffOutput};
use uiaprobe::model::DumpOutput;
use uiaprobe::stdio::run_stdio_loop;
use uiaprobe::uia::browser::ChromiumDomExtractor;
use uiaprobe::uia::ia2::{extract_gecko_ia2_nodes, is_gecko_process};
use uiaprobe::uia::standard::{extract_window_nodes, ExtractOptions};
use uiaprobe::uia::{foreground_hwnd, read_window_info, UiaSession};
use uiaprobe::{ProbeError, Result};

fn run_dump(args: DumpArgs) -> Result<()> {
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
    let mut extracted = extract_window_nodes(&session, hwnd, &opts, &browser)?;

    if args.dom && is_gecko_process(&window.process) && extracted.nodes.len() < args.max_nodes {
        let remaining = args.max_nodes.saturating_sub(extracted.nodes.len());
        match extract_gecko_ia2_nodes(hwnd, args.depth, remaining, args.compact) {
            Ok((ia2_nodes, ia2_trunc)) => {
                extracted.nodes.extend(ia2_nodes);
                extracted.truncated |= ia2_trunc;
            }
            Err(err) => {
                eprintln!("warning: Firefox IA2 fallback failed: {err}");
            }
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let json = if let Some(baseline_path) = args.diff {
        let raw_prev = fs::read_to_string(&baseline_path).map_err(|e| {
            ProbeError::InvalidArgs(format!(
                "failed to read --diff baseline '{}': {e}",
                baseline_path.display()
            ))
        })?;
        let prev_dump: DumpOutput = serde_json::from_str(&raw_prev).map_err(|e| {
            ProbeError::InvalidArgs(format!(
                "invalid --diff baseline JSON '{}': {e}",
                baseline_path.display()
            ))
        })?;
        let diff_out = DiffOutput {
            timestamp,
            elapsed_ms,
            truncated: extracted.truncated,
            window,
            diff: compute_diff(&prev_dump.nodes, &extracted.nodes),
        };
        if args.json && !args.compact {
            serde_json::to_string_pretty(&diff_out)
        } else {
            serde_json::to_string(&diff_out)
        }
    } else {
        let output = DumpOutput {
            timestamp,
            elapsed_ms,
            truncated: extracted.truncated,
            window,
            nodes: extracted.nodes,
        };
        if args.json && !args.compact {
            serde_json::to_string_pretty(&output)
        } else {
            serde_json::to_string(&output)
        }
    }
    .map_err(|e| ProbeError::Serialization(e.to_string()))?;

    println!("{json}");
    Ok(())
}

fn main() -> ExitCode {
    // Must be the very first call before any Win32/UIA API usage.
    init_dpi_awareness();

    let cli = Cli::parse();
    let res = match cli.command {
        Commands::Dump(args) => run_dump(args),
        Commands::Stdio => run_stdio_loop(),
    };
    if let Err(err) = res {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
