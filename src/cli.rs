use crate::error::{ProbeError, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "uiaprobe",
    version,
    about = "Read-only Windows GUI structure perception CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Dump interactive UI elements of a window as JSON to stdout
    Dump(DumpArgs),
    /// Run resident JSON-line interactive mode over stdin/stdout
    Stdio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TargetKind {
    Active,
}

#[derive(Debug, Args)]
pub struct DumpArgs {
    /// Target selection mode (defaults to active foreground window if --hwnd is omitted)
    #[arg(long, value_enum, default_value_t = TargetKind::Active)]
    pub target: TargetKind,

    /// Target a specific window handle (decimal or 0x-prefixed hex)
    #[arg(long)]
    pub hwnd: Option<String>,

    /// Enable browser DOM extraction (Chromium RootWebArea)
    #[arg(long, default_value_t = false)]
    pub dom: bool,

    /// Maximum UIA traversal depth
    #[arg(long, default_value_t = 16)]
    pub depth: usize,

    /// Maximum filtered interactive nodes to output
    #[arg(long, default_value_t = 300)]
    pub max_nodes: usize,

    /// Pretty-print JSON output to stdout
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Emit single-line minified JSON and omit default 'enabled' state
    #[arg(long, default_value_t = false)]
    pub compact: bool,

    /// Compare current nodes against a baseline DumpOutput JSON file
    #[arg(long)]
    pub diff: Option<PathBuf>,
}

/// Parses a decimal or `0x`-prefixed hexadecimal HWND string.
pub fn parse_hwnd(raw: &str) -> Result<isize> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProbeError::InvalidArgs("empty --hwnd value".to_string()));
    }
    let parsed = if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        isize::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<isize>()
    };
    match parsed {
        Ok(val) if val != 0 => Ok(val),
        _ => Err(ProbeError::InvalidArgs(format!(
            "invalid window handle: '{raw}'"
        ))),
    }
}
