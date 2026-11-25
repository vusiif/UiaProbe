use crate::cli::parse_hwnd;
use crate::diff::{compute_diff, DiffOutput};
use crate::error::{ProbeError, Result};
use crate::model::{DumpOutput, UiNode};
use crate::uia::browser::ChromiumDomExtractor;
use crate::uia::ia2::{extract_gecko_ia2_nodes, is_gecko_process};
use crate::uia::standard::{extract_window_nodes, ExtractOptions};
use crate::uia::{foreground_hwnd, read_window_info, UiaSession};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
enum StdioRequest {
    Ping,
    Quit,
    Dump {
        #[serde(default)]
        hwnd: Option<String>,
        #[serde(default)]
        dom: bool,
        #[serde(default = "default_depth")]
        depth: usize,
        #[serde(default = "default_max_nodes")]
        max_nodes: usize,
        #[serde(default)]
        compact: bool,
        #[serde(default)]
        diff: bool,
    },
}

const fn default_depth() -> usize {
    16
}

const fn default_max_nodes() -> usize {
    300
}

/// Runs the resident JSON-line interactive loop over `stdin`/`stdout`.
///
/// Reuses a single `UiaSession` across requests and caches previous snapshots
/// per `hwnd` for instant `diff: true` deltas.
pub fn run_stdio_loop() -> Result<()> {
    let session = UiaSession::new()?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut prev_snapshots: HashMap<isize, Vec<UiNode>> = HashMap::new();

    for line_res in stdin.lock().lines() {
        let line =
            line_res.map_err(|e| ProbeError::UiaFailure(format!("stdin read error: {e}")))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: StdioRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                writeln!(stdout, "{{\"ok\":false,\"error\":{:?}}}", e.to_string())
                    .map_err(|io_e| ProbeError::Serialization(io_e.to_string()))?;
                let _ = stdout.flush();
                continue;
            }
        };

        match req {
            StdioRequest::Ping => {
                writeln!(stdout, "{{\"ok\":true,\"pong\":true}}")
                    .map_err(|e| ProbeError::Serialization(e.to_string()))?;
                let _ = stdout.flush();
            }
            StdioRequest::Quit => {
                writeln!(stdout, "{{\"ok\":true,\"bye\":true}}")
                    .map_err(|e| ProbeError::Serialization(e.to_string()))?;
                let _ = stdout.flush();
                break;
            }
            StdioRequest::Dump {
                hwnd,
                dom,
                depth,
                max_nodes,
                compact,
                diff,
            } => {
                let resp = handle_stdio_dump(
                    &session,
                    hwnd.as_deref(),
                    dom,
                    depth,
                    max_nodes,
                    compact,
                    diff,
                    &mut prev_snapshots,
                );
                match resp {
                    Ok(json) => writeln!(stdout, "{json}")
                        .map_err(|e| ProbeError::Serialization(e.to_string()))?,
                    Err(err) => {
                        writeln!(stdout, "{{\"ok\":false,\"error\":{:?}}}", err.to_string())
                            .map_err(|e| ProbeError::Serialization(e.to_string()))?
                    }
                }
                let _ = stdout.flush();
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_stdio_dump(
    session: &UiaSession,
    hwnd_raw: Option<&str>,
    dom: bool,
    depth: usize,
    max_nodes: usize,
    compact: bool,
    diff: bool,
    prev_snapshots: &mut HashMap<isize, Vec<UiNode>>,
) -> Result<String> {
    let start = Instant::now();
    let hwnd = match hwnd_raw {
        Some(raw) => parse_hwnd(raw)?,
        None => foreground_hwnd()?,
    };
    let window = read_window_info(hwnd)?;
    let opts = ExtractOptions {
        depth,
        max_nodes,
        dom,
        compact,
    };

    let mut extracted = extract_window_nodes(session, hwnd, &opts, &ChromiumDomExtractor)?;
    if dom && is_gecko_process(&window.process) && extracted.nodes.len() < max_nodes {
        let remaining = max_nodes.saturating_sub(extracted.nodes.len());
        if let Ok((ia2_nodes, ia2_trunc)) = extract_gecko_ia2_nodes(hwnd, depth, remaining, compact)
        {
            extracted.nodes.extend(ia2_nodes);
            extracted.truncated |= ia2_trunc;
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    if diff {
        let prev = prev_snapshots.get(&hwnd).map(Vec::as_slice).unwrap_or(&[]);
        let summary = compute_diff(prev, &extracted.nodes);
        prev_snapshots.insert(hwnd, extracted.nodes);
        let out = DiffOutput {
            timestamp,
            elapsed_ms,
            truncated: extracted.truncated,
            window,
            diff: summary,
        };
        serde_json::to_string(&out).map_err(|e| ProbeError::Serialization(e.to_string()))
    } else {
        prev_snapshots.insert(hwnd, extracted.nodes.clone());
        let out = DumpOutput {
            timestamp,
            elapsed_ms,
            truncated: extracted.truncated,
            window,
            nodes: extracted.nodes,
        };
        serde_json::to_string(&out).map_err(|e| ProbeError::Serialization(e.to_string()))
    }
}
