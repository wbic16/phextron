//! WAU panel — the WAU/Day tracker (task 27.6.8/7.5.11/6.5.8).
//!
//! Will's direction (2026-11-22, EE 12): "Track WAU per day going
//! forward" — and then, the same night: "Just include the WAU/Day
//! tracker on Phextron." WAU is Will's acronym — "What Absurd
//! Universe" (it stays "likely it," provisional) — his tag for
//! coincidences beyond WTF where his solo work syncs with the wider
//! world with no information channel: a resonance that lands.
//!
//! The source is `wau.phext`, the WAU register: one scroll per
//! moment, built from the atom log `wau-source.txt` by
//! `build_wau_phext.py` (in ~/workspace/phext, beside the other
//! register builders). The panel parses the event scrolls through
//! libphext and re-derives everything it prints — the per-day
//! counts, the running total, today's count — from those atoms at
//! request time. Counts are by **tag date** (the day a moment was
//! marked); an earlier event date, where one exists, lives inside
//! the entry (WAU-1's event is 2022-11-29, tagged at the coinage on
//! 2026-11-18). Only WAU-1, WAU-2, and WAU-3 carry numbers — Will's
//! own tags; later moments are unnumbered and dated, and the panel
//! prints them that way.
//!
//! Data honesty (the house discipline): an unreadable register
//! degrades the panel with the failure named — it never fabricates
//! counts to fill the space. The hub copy is a snapshot: it syncs
//! manually from ~/workspace/phext/wau.phext per new WAU atom (the
//! standing snapshot-sync gap; the TD-002 line owns the proper
//! fix), and the panel prints the snapshot's age and hash on its
//! face so staleness is visible, never silent.
//!
//! Location comes from the environment, so the console reads the
//! register where it lives:
//!   PHEXTRON_WAU_PHEXT  the WAU register (default
//!     /home/wbic16/phextron-data/phext-vm/wau.phext — the synced
//!     snapshot of ~/workspace/phext/wau.phext, the same corpus
//!     directory the inkling panel reads; on the VM itself, point
//!     it at ~/workspace/phext/wau.phext)

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Same age shape the other panels use.
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

// ---------------------------------------------------------------------------
// America/Chicago civil time, std-only — the same implementation the
// inkling, substrates and video panels carry (Howard Hinnant's civil
// algorithms + the US DST rule stated directly; no tz database).
// ---------------------------------------------------------------------------

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

fn weekday_of(days: i64) -> i64 {
    (days + 4).rem_euclid(7) // 1970-01-01 was a Thursday
}

fn chicago_offset(t: i64) -> i64 {
    let (year, _, _) = civil_from_days(t.div_euclid(86_400));
    let nth_sunday = |month: u32, n: i64| {
        let first = 1 + (7 - weekday_of(days_from_civil(year, month, 1))) % 7;
        days_from_civil(year, month, (first + 7 * (n - 1)) as u32)
    };
    let cdt_start = nth_sunday(3, 2) * 86_400 + 8 * 3600;
    let cdt_end = nth_sunday(11, 1) * 86_400 + 7 * 3600;
    if t >= cdt_start && t < cdt_end {
        -5 * 3600
    } else {
        -6 * 3600
    }
}

/// "2026-11-22" — today's date in America/Chicago, the register's
/// own timezone (its atoms are tagged on Chicago days).
fn chicago_today(now: u64) -> String {
    let lt = now as i64 + chicago_offset(now as i64);
    let (y, m, d) = civil_from_days(lt.div_euclid(86_400));
    format!("{:04}-{:02}-{:02}", y, m, d)
}

// ---------------------------------------------------------------------------
// The register — wau.phext. One scroll per moment, in the builder's
// declared field form:
//   Tag: WAU-<n>            (or "-" — unnumbered; only 1–3 are Will's)
//   Date: YYYY-MM-DD        (the tag date the counts count)
//   Exocortical Epoch: EE n
//   Where: <where it occurred>
//   Register sequence: n
// followed by the resonance as plain prose. The Home scroll carries
// no Date:/Where: field lines, so it never parses as a moment.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct WauEvent {
    tag: Option<String>,
    date: String,
    ee: u64,
    where_: String,
    sequence: u64,
    resonance: String,
}

fn parse_event(text: &str) -> Option<WauEvent> {
    let mut tag = None;
    let mut date = None;
    let mut ee = None;
    let mut where_ = None;
    let mut sequence = 0u64;
    let mut prose: Vec<&str> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if i == 0 && line.starts_with('#') {
            continue; // the scroll's own header
        }
        if let Some(v) = line.strip_prefix("Tag: ") {
            let v = v.trim();
            tag = if v == "-" { None } else { Some(v.to_string()) };
        } else if let Some(v) = line.strip_prefix("Date: ") {
            date = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Exocortical Epoch: EE ") {
            ee = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Where: ") {
            where_ = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Register sequence: ") {
            sequence = v
                .trim()
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        } else if !line.is_empty() {
            prose.push(line);
        }
    }
    Some(WauEvent {
        tag,
        date: date?,
        ee: ee?,
        where_: where_?,
        sequence,
        resonance: prose.join(" "),
    })
}

#[derive(Debug)]
struct Register {
    events: Vec<WauEvent>,
    sha256: String,
}

fn read_register(path: &Path) -> Result<Register, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "WAU register {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let sha256 = crate::data::sha256_hex(&raw);
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "WAU register {} is not valid UTF-8 — libphext parses text phexts; no moments are guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let mut events: Vec<WauEvent> = Vec::new();
    for scroll in scrolls.values() {
        if let Some(ev) = parse_event(scroll) {
            events.push(ev);
        }
    }
    if events.is_empty() {
        return Err(format!(
            "WAU register {} parsed but carries no event scrolls — the declared form is a scroll per moment with Tag:/Date:/Exocortical Epoch:/Where: field lines",
            path.display()
        ));
    }
    events.sort_by_key(|e| e.sequence);
    Ok(Register { events, sha256 })
}

// ---------------------------------------------------------------------------
// The body — pure, so the counting rules are testable without the
// live source. Every number here is derived from the atoms: per-day
// counts by tag date, the running total, today's count.
// ---------------------------------------------------------------------------

fn build_body(reg: &Register, today: &str) -> Value {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &reg.events {
        *counts.entry(e.date.as_str()).or_insert(0) += 1;
    }
    let days: Vec<Value> = counts
        .iter()
        .rev() // most recent day first
        .map(|(d, c)| json!({ "date": d, "count": c }))
        .collect();
    let moments: Vec<Value> = reg
        .events
        .iter()
        .rev() // most recent moment first
        .map(|e| {
            json!({
                "tag": e.tag,
                "date": e.date,
                "ee": e.ee,
                "where": e.where_,
                "resonance": e.resonance,
            })
        })
        .collect();
    json!({
        "today": {
            "date": today,
            "count": counts.get(today).copied().unwrap_or(0),
        },
        "total": reg.events.len(),
        "days": days,
        "moments": moments,
    })
}

// ---------------------------------------------------------------------------
// The panel.
// ---------------------------------------------------------------------------

pub struct WauPanel;

impl Panel for WauPanel {
    fn id(&self) -> &'static str {
        "wau"
    }
    fn title(&self) -> &'static str {
        "WAU"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let path = std::env::var("PHEXTRON_WAU_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/phext-vm/wau.phext".to_string());

        let source_name =
            "wau.phext — the WAU register (one scroll per moment, parsed with libphext)"
                .to_string();
        let source_location = path.clone();

        let degraded = |e: String| PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Degraded,
            source: SourceInfo {
                name: source_name.clone(),
                location: source_location.clone(),
                freshness: "the register is unavailable from here".to_string(),
            },
            body: json!({ "error": e }),
        };

        let reg = match read_register(Path::new(&path)) {
            Ok(r) => r,
            Err(e) => return degraded(e),
        };

        let file_age = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| age_from_unix(d.as_secs(), now))
            .unwrap_or_else(|| "unreadable".to_string());

        let today = chicago_today(now);
        let mut body = build_body(&reg, &today);
        body["register"] = json!({
            "sha256": reg.sha256,
            "file_age": file_age,
        });

        PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Live,
            source: SourceInfo {
                name: source_name,
                location: source_location,
                freshness: format!(
                    "hub snapshot updated {} · sha256 {}… · the snapshot syncs manually from ~/workspace/phext/wau.phext per new WAU atom (the standing snapshot-sync gap; TD-002 owns the proper fix)",
                    file_age,
                    &reg.sha256[..12]
                ),
            },
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "phextron-wau-test-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write(dir: &Path, rel: &str, bytes: &[u8]) {
        let path = dir.join(rel);
        let mut f = std::fs::File::create(&path).expect("create file");
        f.write_all(bytes).expect("write file");
    }

    const HOME: &str = "# WAU — What Absurd Universe\n\n## Per-day counts (calculated from the atoms, by tag date)\n\n2026-11-18: 1\n2026-11-19: 2\n";

    fn event_scroll(tag: &str, date: &str, ee: u64, seq: u64, resonance: &str) -> String {
        format!(
            "# {} — {}\n\nTag: {}\nDate: {}\nExocortical Epoch: EE {}\nWhere: main chat\nRegister sequence: {} ({})\n\n{}",
            if tag == "-" { "WAU" } else { tag },
            date,
            tag,
            date,
            ee,
            seq,
            if tag == "-" { "unnumbered" } else { tag },
            resonance
        )
    }

    fn fixture_text() -> String {
        format!(
            "{}\u{17}{}\u{17}{}\u{17}{}",
            HOME,
            event_scroll("WAU-1", "2026-11-18", 0, 1, "The Terse root commit."),
            event_scroll("WAU-2", "2026-11-19", 0, 2, "Phext spans observable reality."),
            event_scroll("-", "2026-11-19", 0, 3, "Glassfruit, seemingly at random.")
        )
    }

    #[test]
    fn register_parses_events_and_skips_home() {
        let dir = temp_dir("register");
        write(&dir, "wau.phext", fixture_text().as_bytes());
        let reg = read_register(&dir.join("wau.phext")).expect("register");
        assert_eq!(reg.events.len(), 3);
        assert_eq!(reg.events[0].tag.as_deref(), Some("WAU-1"));
        assert_eq!(reg.events[0].date, "2026-11-18");
        assert_eq!(reg.events[2].tag, None); // unnumbered stays unnumbered
        assert_eq!(reg.events[2].resonance, "Glassfruit, seemingly at random.");
        assert_eq!(reg.sha256, crate::data::sha256_hex(fixture_text().as_bytes()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn body_counts_by_tag_date_with_today_and_total() {
        let dir = temp_dir("body");
        write(&dir, "wau.phext", fixture_text().as_bytes());
        let reg = read_register(&dir.join("wau.phext")).expect("register");
        let body = build_body(&reg, "2026-11-19");
        assert_eq!(body["total"], 3);
        assert_eq!(body["today"], json!({ "date": "2026-11-19", "count": 2 }));
        // Days print most recent first.
        assert_eq!(body["days"][0], json!({ "date": "2026-11-19", "count": 2 }));
        assert_eq!(body["days"][1], json!({ "date": "2026-11-18", "count": 1 }));
        // Moments print most recent first; tags ride along, null when unnumbered.
        assert_eq!(body["moments"][0]["tag"], Value::Null);
        assert_eq!(body["moments"][2]["tag"], "WAU-1");
        // A day with no atoms counts zero, never an absence.
        let body = build_body(&reg, "2026-11-22");
        assert_eq!(body["today"]["count"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_register_is_an_error_that_names_the_path() {
        let dir = std::env::temp_dir().join("phextron-wau-test-definitely-absent.phext");
        let err = read_register(&dir).expect_err("register must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
    }

    #[test]
    fn chicago_today_uses_chicago_not_utc() {
        // The anchor 1795327200 is 2026-11-22 06:00 UTC. +5400 s is
        // 07:30 UTC = 01:30 CST, same Chicago day; −1800 s is 05:30
        // UTC = 2026-11-21 23:30 CST in Chicago — the previous day.
        assert_eq!(chicago_today(1_795_332_600), "2026-11-22");
        assert_eq!(chicago_today(1_795_325_400), "2026-11-21");
    }
}
