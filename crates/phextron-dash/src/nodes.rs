//! Ranch-nodes panel — the stage-1 live panel.
//!
//! Reads three live machine-readable sources, and only those:
//! - the fleet roster: ranch-proxy `nodes.json` (every ranch node; a node is
//!   never dropped from the roster, down or not),
//! - the sampler's latest pull: `last-pull.json` from the ranch-state
//!   distillation pipeline (per-node status, load, Tctl, GPU edge, and the
//!   ollama / render / gateway markers, measured by the ranch fanout),
//! - the distilled register: `state.phext`, parsed with libphext (the
//!   reference phext implementation) — its newest entry rides the panel as
//!   the fleet's distilled state line.
//!
//! Source paths come from the environment so the console can be co-located
//! with the sampler's outputs wherever they live:
//!   PHEXTRON_NODES_JSON, PHEXTRON_LAST_PULL, PHEXTRON_STATE_PHEXT.

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn env_path(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_string())
}

fn file_age_s(path: &str) -> Option<u64> {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(now.saturating_sub(mtime))
}

fn age_line(path: &str) -> String {
    match file_age_s(path) {
        Some(s) if s < 90 => format!("updated {}s ago", s),
        Some(s) if s < 5400 => format!("updated {}m ago", s / 60),
        Some(s) => format!("updated {}h {}m ago", s / 3600, (s % 3600) / 60),
        None => "source file not readable".to_string(),
    }
}

/// One distilled entry line from state.phext. Two shapes occur:
/// `logical-coord | YYYY-MM-DD HH:MM TZ | summary` (a displaced write) and
/// `YYYY-MM-DD HH:MM TZ | summary` (a write at its own coordinate).
/// Returns (timestamp, summary) — timestamps compare correctly as strings.
fn parse_entry_line(line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = line.splitn(4, " | ").collect();
    let looks_like_ts = |s: &str| {
        s.len() >= 16
            && s.as_bytes().get(4) == Some(&b'-')
            && s.as_bytes().get(7) == Some(&b'-')
            && s.as_bytes().get(10) == Some(&b' ')
    };
    match parts.len() {
        3 if looks_like_ts(parts[1]) => Some((parts[1].to_string(), parts[2].to_string())),
        2 if looks_like_ts(parts[0]) => Some((parts[0].to_string(), parts[1].to_string())),
        _ => None,
    }
}

/// The newest distilled entry in state.phext, plus register statistics.
/// Parsed through libphext's `explode` — the reference parser, never a
/// hand-rolled scroll splitter.
fn latest_distilled(path: &str) -> Option<(usize, usize, String, String)> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let scrolls = libphext::phext::explode(&text);
    let scroll_count = scrolls.len();
    let mut entries = 0usize;
    let mut best: Option<(String, String)> = None;
    for scroll in scrolls.values() {
        for line in scroll.lines() {
            if let Some((ts, summary)) = parse_entry_line(line) {
                entries += 1;
                if best.as_ref().map_or(true, |(bts, _)| ts > *bts) {
                    best = Some((ts, summary));
                }
            }
        }
    }
    let (ts, summary) = best?;
    Some((scroll_count, entries, ts, summary))
}

pub struct NodesPanel;

impl Panel for NodesPanel {
    fn id(&self) -> &'static str {
        "nodes"
    }
    fn title(&self) -> &'static str {
        "Ranch Nodes"
    }

    fn data(&self) -> PanelData {
        let roster_path = env_path(
            "PHEXTRON_NODES_JSON",
            "/home/wbic16/phextron-data/nodes.json",
        );
        let pull_path = env_path(
            "PHEXTRON_LAST_PULL",
            "/home/wbic16/phextron-data/last-pull.json",
        );
        let state_path = env_path(
            "PHEXTRON_STATE_PHEXT",
            "/home/wbic16/phextron-data/state.phext",
        );

        let degraded = |why: String| PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Degraded,
            source: SourceInfo {
                name: "ranch fanout pull + state.phext register".into(),
                location: format!("roster {}, pull {}, register {}", roster_path, pull_path, state_path),
                freshness: why.clone(),
            },
            body: json!({ "error": why }),
        };

        let roster_raw = match std::fs::read_to_string(&roster_path) {
            Ok(s) => s,
            Err(e) => return degraded(format!("roster unreadable: {}", e)),
        };
        let pull_raw = match std::fs::read_to_string(&pull_path) {
            Ok(s) => s,
            Err(e) => return degraded(format!("pull unreadable: {}", e)),
        };
        let roster: Value = match serde_json::from_str(&roster_raw) {
            Ok(v) => v,
            Err(e) => return degraded(format!("roster unparseable: {}", e)),
        };
        let pull: Value = match serde_json::from_str(&pull_raw) {
            Ok(v) => v,
            Err(e) => return degraded(format!("pull unparseable: {}", e)),
        };

        // Per-node pull records, by name.
        let mut pull_by_name: HashMap<String, &Map<String, Value>> = HashMap::new();
        if let Some(nodes) = pull.get("nodes").and_then(|n| n.as_array()) {
            for n in nodes {
                if let (Some(name), Some(obj)) = (n.get("name").and_then(|v| v.as_str()), n.as_object()) {
                    pull_by_name.insert(name.to_string(), obj);
                }
            }
        }

        let mut rows: Vec<Value> = Vec::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        if let Some(nodes) = roster.get("nodes").and_then(|n| n.as_array()) {
            for n in nodes {
                let name = n.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let p = pull_by_name.get(name);
                let status = p
                    .and_then(|o| o.get("status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("not-in-pull")
                    .to_string();
                *counts.entry(status.clone()).or_insert(0) += 1;
                let getf = |k: &str| p.and_then(|o| o.get(k)).cloned().unwrap_or(Value::Null);
                rows.push(json!({
                    "name": name,
                    "display": n.get("display").and_then(|v| v.as_str()).unwrap_or(name),
                    "dns": n.get("dns").and_then(|v| v.as_str()).unwrap_or(""),
                    "ip": n.get("ip").and_then(|v| v.as_str()).unwrap_or(""),
                    "enabled": n.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                    "note": n.get("note").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": status,
                    "load": getf("load"),
                    "ncpu": getf("ncpu"),
                    "tctl_c": getf("tctl"),
                    "gpu_c": getf("gpu"),
                    "ollama": getf("ollama"),
                    "render": getf("render"),
                    "gateway": getf("gateway"),
                    "top": getf("top"),
                }));
            }
        }

        let distilled = latest_distilled(&state_path).map(|(scrolls, entries, ts, summary)| {
            json!({
                "scrolls": scrolls,
                "entries": entries,
                "ts": ts,
                "summary": summary,
                "register_age": age_line(&state_path),
            })
        });

        PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Live,
            source: SourceInfo {
                name: "ranch fanout pull (last-pull.json) + fleet roster (nodes.json) + state.phext via libphext".into(),
                location: format!("pull {}, roster {}, register {}", pull_path, roster_path, state_path),
                freshness: format!(
                    "pull {} · register {} · pull ts {}",
                    age_line(&pull_path),
                    age_line(&state_path),
                    pull.get("ts").and_then(|v| v.as_str()).unwrap_or("unknown")
                ),
            },
            body: json!({
                "pull_ts": pull.get("ts").cloned().unwrap_or(Value::Null),
                "pull_coordinate": pull.get("coordinate").cloned().unwrap_or(Value::Null),
                "counts": counts,
                "roster_count": rows.len(),
                "distilled": distilled,
                "rows": rows,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_entry_line;

    #[test]
    fn displaced_entry_line_parses() {
        let (ts, summary) = parse_entry_line(
            "28.8.9/9.8.12/10.7.11 | 2026-11-21 17:53 CST | 16/23 nodes up; busy: inference on free-fall.",
        )
        .expect("entry");
        assert_eq!(ts, "2026-11-21 17:53 CST");
        assert!(summary.starts_with("16/23 nodes up"));
    }

    #[test]
    fn own_coordinate_entry_line_parses() {
        let (ts, _) = parse_entry_line("2026-11-21 17:48 CST | 16/23 nodes up.").expect("entry");
        assert_eq!(ts, "2026-11-21 17:48 CST");
    }

    #[test]
    fn prose_is_not_an_entry() {
        assert!(parse_entry_line("Home documents the layout and entry format.").is_none());
        assert!(parse_entry_line("COSTS — the taxicab ledger | with a pipe").is_none());
    }
}
