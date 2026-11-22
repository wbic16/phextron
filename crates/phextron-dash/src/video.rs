//! Video Pipelines panel — the stage-4 live panel.
//!
//! Two live machine-readable sources, each read directly, each degrading
//! independently and by name — never a rendered page, never a build tree
//! walked at request time:
//!
//! - **The pipeline register**: `pipelines.phext`, the fleet register this
//!   stage created (house pattern: `pipelines-source.json` is the source
//!   of truth, `build_pipelines_phext.py` builds, `update_pipeline.py` is
//!   the writer path a pipeline updates its own entry through). One scroll
//!   per pipeline — name, kind (film / animation / training / other),
//!   state summary, last activity, the source its truth lives in, and the
//!   entry's updated stamp. Parsed here with libphext (`explode`, the
//!   reference parser). The generated Home scroll documents the register
//!   and is not a pipeline row.
//! - **ranch-batchq**: the standing queue on rock-mountain
//!   (`~/ranch-batchq`: pending/running/done/failed/outputs directories +
//!   `state/status.json`). Read live from the queue directories when they
//!   are readable from this host (`PHEXTRON_VIDEO_BATCHQ_DIR` — the case
//!   when the console is co-located with the queue); otherwise read from
//!   the snapshot document the VM-side sync writes beside the other
//!   phextron snapshots (`PHEXTRON_VIDEO_BATCHQ_JSON`), with the capture
//!   age on its face. With neither read path, the queue section degrades
//!   by name and the register rows still show — the two sources never
//!   take each other down.
//!
//! Locations come from the environment, so the console reads each source
//! where it lives:
//!   PHEXTRON_VIDEO_PIPELINES_PHEXT  register file (default
//!     /home/wbic16/phextron-data/phext-vm/pipelines.phext — the synced
//!     snapshot of ~/workspace/phext, the same corpus directory the
//!     phexts panel censuses; on the VM itself, point it at
//!     ~/workspace/phext/pipelines.phext)
//!   PHEXTRON_VIDEO_BATCHQ_DIR       live queue directory (no default —
//!     a guessed path on the wrong host would be a lie; set it where
//!     ~/ranch-batchq is actually readable)
//!   PHEXTRON_VIDEO_BATCHQ_JSON      queue snapshot document (default
//!     /home/wbic16/phextron-data/batchq.json)

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Same age shape the nodes and phexts panels use.
fn age_from_unix(ts: u64, now: u64) -> String {
    let s = now.saturating_sub(ts);
    if s < 90 {
        format!("{}s ago", s)
    } else if s < 5400 {
        format!("{}m ago", s / 60)
    } else if s < 172_800 {
        format!("{}h {}m ago", s / 3600, (s % 3600) / 60)
    } else {
        format!("{}d ago", s / 86_400)
    }
}

/// Days since the Unix epoch for a civil date (Howard Hinnant's
/// algorithm) — lets a `YYYY-MM-DD` last-activity date become an age
/// without a date library.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// The current civil day in America/Chicago, as days since the epoch.
/// Register dates are Chicago dates (the fleet runs on Chicago time), so
/// "today" must be Chicago's day, not UTC's — at 19:30 CST the UTC day
/// has already rolled over and a UTC age would call today's work
/// yesterday's. The US DST rule is implemented directly (CDT from the
/// second Sunday of March 08:00 UTC to the first Sunday of November
/// 07:00 UTC) so no tz database or crate is needed.
fn chicago_today(now: u64) -> i64 {
    let days = (now / 86_400) as i64;
    let year = {
        // Civil year of the UTC day (inverse of days_from_civil).
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        if m <= 2 { y + 1 } else { y }
    };
    // Day-of-week of a given month's 1st (0 = Sunday).
    let dow1 = |month: u32| {
        let d = days_from_civil(year, month, 1);
        (d + 4).rem_euclid(7) // 1970-01-01 was a Thursday
    };
    let nth_sunday = |month: u32, n: i64| {
        let first = 1 + (7 - dow1(month)) % 7;
        days_from_civil(year, month, (first + 7 * (n - 1)) as u32)
    };
    let cdt_start = nth_sunday(3, 2) * 86_400 + 8 * 3600;
    let cdt_end = nth_sunday(11, 1) * 86_400 + 7 * 3600;
    let offset: i64 = if (now as i64) >= cdt_start && (now as i64) < cdt_end {
        -5 * 3600
    } else {
        -6 * 3600
    };
    ((now as i64 + offset).div_euclid(86_400)) as i64
}

fn activity_age(last_activity: &str, now: u64) -> String {
    let today = chicago_today(now);
    match parse_date(last_activity) {
        Some(days) => {
            let diff = today - days;
            if diff <= 0 {
                "today".to_string()
            } else if diff == 1 {
                "1d ago".to_string()
            } else {
                format!("{}d ago", diff)
            }
        }
        None => "unknown".to_string(),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PipelineEntry {
    coordinate: String,
    name: String,
    kind: String,
    state: String,
    last_activity: String,
    source: String,
    updated: String,
}

/// Parse one register scroll. An entry scroll carries a `# name` heading
/// and the field lines the builder renders (Coordinate / Kind / State /
/// Last activity / Source / Updated). The generated Home scroll has the
/// heading and prose but no Kind/State field lines, so it never parses
/// as an entry — that is what keeps it out of the rows.
fn parse_entry(text: &str) -> Option<PipelineEntry> {
    let mut name = None;
    let mut fields: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("# ") {
            if name.is_none() {
                name = Some(h.trim().to_string());
            }
            continue;
        }
        for key in ["Coordinate", "Kind", "State", "Last activity", "Source", "Updated"] {
            if let Some(v) = line.strip_prefix(&format!("{}: ", key)) {
                fields.insert(key, v.trim());
            }
        }
    }
    let name = name?;
    // Kind and State are the entry's signature; Home prose has neither.
    let kind = fields.get("Kind")?.to_string();
    let state = fields.get("State")?.to_string();
    Some(PipelineEntry {
        coordinate: fields.get("Coordinate").copied().unwrap_or("").to_string(),
        name,
        kind,
        state,
        last_activity: fields.get("Last activity").copied().unwrap_or("").to_string(),
        source: fields.get("Source").copied().unwrap_or("").to_string(),
        updated: fields.get("Updated").copied().unwrap_or("").to_string(),
    })
}

/// Read the pipeline register: every scroll libphext's `explode` returns
/// is offered to the entry parser; scrolls that do not parse as entries
/// (the Home scroll) are documentation, not rows. An unreadable or
/// non-UTF-8 register is an `Err` — the panel degrades rather than
/// guessing at a fleet list.
fn read_register(path: &Path) -> Result<Vec<PipelineEntry>, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "pipeline register {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "pipeline register {} is not valid UTF-8 — libphext parses text phexts; no rows are guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let mut entries: Vec<PipelineEntry> = scrolls
        .values()
        .filter_map(|s| parse_entry(s))
        .collect();
    // Most recent activity first; name breaks ties so the order is stable.
    entries.sort_by(|a, b| {
        b.last_activity
            .cmp(&a.last_activity)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

#[derive(Clone, Debug, PartialEq)]
struct BatchqCounts {
    pending: u64,
    running: u64,
    done: u64,
    failed: u64,
    outputs: u64,
}

#[derive(Clone, Debug)]
enum BatchqRead {
    LiveDir {
        counts: BatchqCounts,
        status_updated_at: Option<String>,
        reason: Option<String>,
    },
    Snapshot {
        counts: BatchqCounts,
        captured_at: String,
        status_updated_at: Option<String>,
        reason: Option<String>,
    },
    Degraded(String),
}

fn count_dir(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map(|rd| rd.count() as u64).unwrap_or(0)
}

/// Count the queue directories and read the runner's own status document.
fn read_batchq_dir(dir: &Path) -> Result<(BatchqCounts, Option<String>, Option<String>), String> {
    if !dir.is_dir() {
        return Err(format!("queue directory {} is not present on this host", dir.display()));
    }
    let counts = BatchqCounts {
        pending: count_dir(&dir.join("pending")),
        running: count_dir(&dir.join("running")),
        done: count_dir(&dir.join("done")),
        failed: count_dir(&dir.join("failed")),
        outputs: count_dir(&dir.join("outputs")),
    };
    let (mut updated, mut reason) = (None, None);
    if let Ok(raw) = std::fs::read_to_string(dir.join("state/status.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            updated = v
                .get("updated_at")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            reason = match (
                v.get("reason_code").and_then(|x| x.as_str()),
                v.get("reason_detail").and_then(|x| x.as_str()),
            ) {
                (Some(c), Some(d)) => Some(format!("{} — {}", c, d)),
                (Some(c), None) => Some(c.to_string()),
                _ => None,
            };
        }
    }
    Ok((counts, updated, reason))
}

fn counts_from_json(v: &Value) -> Option<BatchqCounts> {
    let c = v.get("counts")?;
    let get = |k: &str| c.get(k).and_then(|x| x.as_u64());
    Some(BatchqCounts {
        pending: get("pending")?,
        running: get("running")?,
        done: get("done")?,
        failed: get("failed")?,
        outputs: get("outputs").unwrap_or(0),
    })
}

/// The batchq read, in order: live queue directories where they are
/// readable, then the snapshot document, then degraded by name. The two
/// live shapes are kept distinct in the result — a snapshot is never
/// dressed up as a live read.
fn read_batchq(dir: Option<&Path>, snapshot: &Path) -> BatchqRead {
    if let Some(d) = dir {
        match read_batchq_dir(d) {
            Ok((counts, updated, reason)) => {
                return BatchqRead::LiveDir {
                    counts,
                    status_updated_at: updated,
                    reason,
                }
            }
            Err(e) => {
                return BatchqRead::Degraded(format!(
                    "ranch-batchq live read failed: {}",
                    e
                ))
            }
        }
    }
    match std::fs::read_to_string(snapshot) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(v) => match (counts_from_json(&v), v.get("captured_at").and_then(|x| x.as_str())) {
                (Some(counts), Some(captured)) => BatchqRead::Snapshot {
                    counts,
                    captured_at: captured.to_string(),
                    status_updated_at: v
                        .get("status_updated_at")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    reason: match (
                        v.get("reason_code").and_then(|x| x.as_str()),
                        v.get("reason_detail").and_then(|x| x.as_str()),
                    ) {
                        (Some(c), Some(d)) => Some(format!("{} — {}", c, d)),
                        (Some(c), None) => Some(c.to_string()),
                        _ => None,
                    },
                },
                _ => BatchqRead::Degraded(format!(
                    "ranch-batchq snapshot {} is present but does not carry counts/captured_at — nothing is guessed from it",
                    snapshot.display()
                )),
            },
            Err(e) => BatchqRead::Degraded(format!(
                "ranch-batchq snapshot {} is unparseable: {}",
                snapshot.display(),
                e
            )),
        },
        Err(_) => BatchqRead::Degraded(format!(
            "ranch-batchq on rock-mountain (~/ranch-batchq) is not readable from this host: no live queue directory is configured (PHEXTRON_VIDEO_BATCHQ_DIR) and no snapshot document is present at {} (PHEXTRON_VIDEO_BATCHQ_JSON) — queue counts are shown only where a real read path exists",
            snapshot.display()
        )),
    }
}

fn counts_json(c: &BatchqCounts) -> Value {
    json!({
        "pending": c.pending,
        "running": c.running,
        "done": c.done,
        "failed": c.failed,
        "outputs": c.outputs,
    })
}

fn batchq_json(read: &BatchqRead, now: u64) -> (Value, String) {
    match read {
        BatchqRead::LiveDir {
            counts,
            status_updated_at,
            reason,
        } => (
            json!({
                "mode": "live-dir",
                "node": "rock-mountain",
                "counts": counts_json(counts),
                "status_updated_at": status_updated_at,
                "reason": reason,
                "captured_age": Value::Null,
                "error": Value::Null,
            }),
            "batchq counts read live from the queue directories at request time".to_string(),
        ),
        BatchqRead::Snapshot {
            counts,
            captured_at,
            status_updated_at,
            reason,
        } => {
            // The snapshot's capture stamp is ISO-8601 with offset; the
            // file's own mtime is the honest age anchor on this host.
            let _ = now;
            (
                json!({
                    "mode": "snapshot",
                    "node": "rock-mountain",
                    "counts": counts_json(counts),
                    "status_updated_at": status_updated_at,
                    "reason": reason,
                    "captured_age": captured_at,
                    "error": Value::Null,
                }),
                format!(
                    "batchq counts from the snapshot captured {} (the queue itself lives on rock-mountain; the snapshot is the read path from this host)",
                    captured_at
                ),
            )
        }
        BatchqRead::Degraded(why) => (
            json!({
                "mode": "degraded",
                "node": "rock-mountain",
                "counts": Value::Null,
                "status_updated_at": Value::Null,
                "reason": Value::Null,
                "captured_age": Value::Null,
                "error": why,
            }),
            format!("batchq unavailable from here: {}", why),
        ),
    }
}

pub struct VideoPanel;

impl Panel for VideoPanel {
    fn id(&self) -> &'static str {
        "video"
    }
    fn title(&self) -> &'static str {
        "Video Pipelines"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let register_path = std::env::var("PHEXTRON_VIDEO_PIPELINES_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/phext-vm/pipelines.phext".to_string());
        let batchq_dir = std::env::var("PHEXTRON_VIDEO_BATCHQ_DIR")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from);
        let batchq_snapshot = std::env::var("PHEXTRON_VIDEO_BATCHQ_JSON")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/batchq.json".to_string());

        let batchq = read_batchq(batchq_dir.as_deref(), Path::new(&batchq_snapshot));
        let (batchq_body, batchq_freshness) = batchq_json(&batchq, now);

        let register_age = std::fs::metadata(&register_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| age_from_unix(d.as_secs(), now))
            .unwrap_or_else(|| "unreadable".to_string());

        let source_name = "pipelines.phext register via libphext + ranch-batchq queue state (rock-mountain)".to_string();
        let source_location = format!(
            "register {} · batchq {}",
            register_path,
            batchq_dir
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| format!("snapshot {}", batchq_snapshot))
        );

        match read_register(Path::new(&register_path)) {
            Ok(entries) => {
                let mut by_kind: std::collections::BTreeMap<String, u64> =
                    std::collections::BTreeMap::new();
                for e in &entries {
                    *by_kind.entry(e.kind.clone()).or_insert(0) += 1;
                }
                let newest_activity = entries
                    .iter()
                    .map(|e| e.last_activity.clone())
                    .max()
                    .unwrap_or_else(|| "none".to_string());
                let rows: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "coordinate": e.coordinate,
                            "name": e.name,
                            "kind": e.kind,
                            "state": e.state,
                            "last_activity": e.last_activity,
                            "activity_age": activity_age(&e.last_activity, now),
                            "source": e.source,
                            "updated": e.updated,
                        })
                    })
                    .collect();
                PanelData {
                    id: self.id(),
                    title: self.title(),
                    state: PanelState::Live,
                    source: SourceInfo {
                        name: source_name,
                        location: source_location,
                        freshness: format!(
                            "register read at request time · register file updated {} · {}",
                            register_age, batchq_freshness
                        ),
                    },
                    body: json!({
                        "register": {
                            "pipelines": entries.len(),
                            "by_kind": by_kind,
                            "newest_activity": newest_activity,
                        },
                        "rows": rows,
                        "batchq": batchq_body,
                    }),
                }
            }
            Err(e) => PanelData {
                id: self.id(),
                title: self.title(),
                state: PanelState::Degraded,
                source: SourceInfo {
                    name: source_name,
                    location: source_location,
                    freshness: format!(
                        "register unavailable from here · {}",
                        batchq_freshness
                    ),
                },
                body: json!({
                    "error": e,
                    "batchq": batchq_body,
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "phextron-video-test-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write(dir: &Path, rel: &str, bytes: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        let mut f = std::fs::File::create(&path).expect("create file");
        f.write_all(bytes).expect("write file");
    }

    const ENTRY: &str = "# Scrolltown animation\n\nCoordinate: 5.5.5/5.5.5/5.5.5\nKind: animation\nState: Active: exercises in gate validation.\nLast activity: 2026-11-21\nSource: ~/workspace/scrolltown-landing production tree\nUpdated: 2026-11-21 19:28 CST\n";

    const HOME: &str = "# pipelines.phext — the fleet video-pipeline register\n\nOne scroll per video pipeline: its name, kind (film / animation /\ntraining / other), state summary, last activity, the source its\ntruth lives in, and the timestamp of the entry's last update.\n\nPipelines recorded: 1\n";

    #[test]
    fn entry_scroll_parses_all_fields() {
        let e = parse_entry(ENTRY).expect("entry");
        assert_eq!(e.name, "Scrolltown animation");
        assert_eq!(e.coordinate, "5.5.5/5.5.5/5.5.5");
        assert_eq!(e.kind, "animation");
        assert_eq!(e.state, "Active: exercises in gate validation.");
        assert_eq!(e.last_activity, "2026-11-21");
        assert_eq!(e.updated, "2026-11-21 19:28 CST");
    }

    #[test]
    fn home_scroll_is_not_an_entry() {
        // The generated Home scroll documents the register; it carries no
        // Kind/State field lines and must never become a pipeline row.
        assert!(parse_entry(HOME).is_none());
    }

    #[test]
    fn register_reads_entries_and_skips_home() {
        let dir = temp_dir("register");
        // Home at 1.1.1/1.1.1/1.1.1, the entry one scroll break later.
        let text = format!("{}\u{17}{}", HOME, ENTRY);
        write(&dir, "pipelines.phext", text.as_bytes());
        let entries = read_register(&dir.join("pipelines.phext")).expect("register");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Scrolltown animation");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_register_is_an_error_that_names_the_path() {
        let dir = std::env::temp_dir().join("phextron-video-test-definitely-absent.phext");
        let err = read_register(&dir).expect_err("must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
        assert!(err.contains("phextron-video-test-definitely-absent"));
    }

    #[test]
    fn batchq_live_dir_counts_and_status() {
        let dir = temp_dir("batchq");
        for sub in ["pending", "running", "done", "failed", "outputs"] {
            std::fs::create_dir_all(dir.join(sub)).expect("queue dir");
        }
        write(&dir, "done/001-a.sh", b"#!/bin/sh\n");
        write(&dir, "done/002-b.sh", b"#!/bin/sh\n");
        write(&dir, "pending/003-c.sh", b"#!/bin/sh\n");
        write(&dir, "outputs/a.jsonl", b"{}\n");
        write(
            &dir,
            "state/status.json",
            br#"{"node":"rock-mountain","updated_at":"2026-11-19T01:37:15-06:00","reason_code":"no_jobs","reason_detail":"pending queue empty","running_job":""}"#,
        );
        match read_batchq(Some(&dir), &dir.join("absent.json")) {
            BatchqRead::LiveDir {
                counts,
                status_updated_at,
                reason,
            } => {
                assert_eq!(counts.pending, 1);
                assert_eq!(counts.running, 0);
                assert_eq!(counts.done, 2);
                assert_eq!(counts.failed, 0);
                assert_eq!(counts.outputs, 1);
                assert_eq!(
                    status_updated_at.as_deref(),
                    Some("2026-11-19T01:37:15-06:00")
                );
                assert_eq!(
                    reason.as_deref(),
                    Some("no_jobs — pending queue empty")
                );
            }
            other => panic!("expected LiveDir, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn batchq_snapshot_is_read_as_snapshot_not_live() {
        let dir = temp_dir("snapshot");
        write(
            &dir,
            "batchq.json",
            br#"{"node":"rock-mountain","captured_at":"2026-11-21T19:29:28-06:00","counts":{"pending":0,"running":0,"done":3,"failed":0,"outputs":3},"status_updated_at":"2026-11-19T01:37:15-06:00","reason_code":"no_jobs","reason_detail":"pending queue empty","running_job":""}"#,
        );
        match read_batchq(None, &dir.join("batchq.json")) {
            BatchqRead::Snapshot {
                counts,
                captured_at,
                ..
            } => {
                assert_eq!(counts.done, 3);
                assert_eq!(captured_at, "2026-11-21T19:29:28-06:00");
            }
            other => panic!("expected Snapshot, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn batchq_with_no_read_path_degrades_by_name() {
        let dir = temp_dir("degraded");
        match read_batchq(None, &dir.join("absent.json")) {
            BatchqRead::Degraded(why) => {
                assert!(why.contains("rock-mountain"), "names the host: {}", why);
                assert!(why.contains("ranch-batchq"), "names the queue: {}", why);
            }
            other => panic!("expected Degraded, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn activity_ages_compute_from_dates() {
        // 2026-11-21 12:00 UTC (06:00 CST) as the request instant.
        let now = days_from_civil(2026, 11, 21) as u64 * 86_400 + 43_200;
        assert_eq!(activity_age("2026-11-21", now), "today");
        assert_eq!(activity_age("2026-11-20", now), "1d ago");
        assert_eq!(activity_age("2026-11-09", now), "12d ago");
        assert_eq!(activity_age("not a date", now), "unknown");
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn activity_age_uses_chicago_not_utc_day() {
        // 2026-11-22 01:30 UTC is still 2026-11-21 19:30 in Chicago:
        // work dated 2026-11-21 is today's, not yesterday's.
        let now = days_from_civil(2026, 11, 22) as u64 * 86_400 + 5_400;
        assert_eq!(activity_age("2026-11-21", now), "today");
        assert_eq!(activity_age("2026-11-20", now), "1d ago");
        // And in CDT (July): 2026-07-02 04:30 UTC is 2026-07-01 23:30 CDT.
        let now_cdt = days_from_civil(2026, 7, 2) as u64 * 86_400 + 16_200;
        assert_eq!(activity_age("2026-07-01", now_cdt), "today");
    }
}
