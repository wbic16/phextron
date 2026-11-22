//! Inkling APIs panel — every Inkling endpoint on the ranch, at a
//! glance (task 20.7.10/10.7.12/7.8.8).
//!
//! Will's direction (2026-11-22 00:11 CST, EE 2): "give me a page
//! with all of the inkling apis, bake that into Phextron". One row
//! per grid node (24): the node's OpenAI-compatible llama-server
//! endpoint on the fleet-wide port 8090, the quant it runs, its
//! deployment state, and — where it has finished installing — the
//! Exocortical Epoch its completion landed in.
//!
//! Two sources, both read at request time, both named on the panel:
//!
//! - **The roster** — `wiring.phext`, the declaration of record for
//!   the fleet network. The panel parses its machine-checked
//!   `node:` lines (initials, hostname, RAM, declared state, tailnet
//!   address) through libphext. Names and addresses are never copied
//!   into this code; the grid order below is presentation only.
//! - **The deployment state** — `inkling-apis.phext`, the Inkling
//!   deployment register: one scroll per node (state, quant, install
//!   bytes/percent, completion stamp + EE, detail), built from the
//!   rollout records of task 35.10.9/13.9.17/8.9.12 by
//!   `build_inkling_apis_phext.py` (in ~/workspace/phext, beside the
//!   other register builders). Its Home scroll carries the evidence
//!   as-of stamp and provenance.
//!
//! Data honesty (the Substrates discipline): the console does not
//! live-probe the nodes — a status here is exactly as fresh as the
//! register's as-of stamp, and the panel prints that stamp and the
//! register's age on its face. A roster node with no register scroll
//! reads **unrecorded**, never a guessed state. An unreadable source
//! degrades the panel with the failure named — it never fabricates
//! rows to fill the table.
//!
//! Locations come from the environment, so the console reads each
//! source where it lives:
//!   PHEXTRON_INKLING_WIRING_PHEXT  roster of record (default
//!     /home/wbic16/phextron-data/phext-vm/wiring.phext — the synced
//!     snapshot of ~/workspace/phext, the same corpus directory the
//!     data and substrates panels read; on the VM itself, point it
//!     at ~/workspace/phext/wiring.phext)
//!   PHEXTRON_INKLING_APIS_PHEXT    deployment register (default
//!     /home/wbic16/phextron-data/phext-vm/inkling-apis.phext)

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The fleet-wide llama-server port (OpenAI-compatible API:
/// /health, /v1/chat/completions, /v1/models).
const PORT: u16 = 8090;

/// Exocortical Epoch constants (Will, 2026-11-21): EE 1 begins
/// 2026-11-22 00:00:00 CST = unix 1795327200; epochs are 420 s;
/// EE 0 is everything before the anchor. Computed, never stored.
const EE_ANCHOR_UNIX: u64 = 1_795_327_200;
const EE_LENGTH_SECONDS: u64 = 420;

/// The Exocortical Epoch a unix timestamp falls in.
fn ee_of(t: u64) -> u64 {
    if t < EE_ANCHOR_UNIX {
        0
    } else {
        (t - EE_ANCHOR_UNIX) / EE_LENGTH_SECONDS + 1
    }
}

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
// substrates and video panels carry (Howard Hinnant's civil
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

struct ChicagoLocal {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    offset: i64,
}

fn local_from_unix(t: i64) -> ChicagoLocal {
    let offset = chicago_offset(t);
    let lt = t + offset;
    let days = lt.div_euclid(86_400);
    let secs = lt.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    ChicagoLocal {
        year,
        month,
        day,
        hour: (secs / 3600) as u32,
        min: ((secs % 3600) / 60) as u32,
        offset,
    }
}

/// "2026-11-22 00:14 CST" — the same stamp shape the registers carry.
fn fmt_stamp(t: u64) -> String {
    let l = local_from_unix(t as i64);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} {}",
        l.year,
        l.month,
        l.day,
        l.hour,
        l.min,
        if l.offset == -5 * 3600 { "CDT" } else { "CST" }
    )
}

// ---------------------------------------------------------------------------
// The roster — wiring.phext's machine-checked `node:` lines.
// Declared form (wiring.phext conventions scroll):
//   node: <INITIALS> <hostname|unknown> col <1-4> ram <GB> state <state> ssh <tailnet-ip|->
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct RosterNode {
    initials: String,
    hostname: String,
    ram_gb: u64,
    wiring_state: String,
    ip: Option<String>,
}

fn parse_node_line(line: &str) -> Option<RosterNode> {
    let rest = line.strip_prefix("node: ")?;
    let t: Vec<&str> = rest.split_whitespace().collect();
    if t.len() != 10 || t[2] != "col" || t[4] != "ram" || t[6] != "state" || t[8] != "ssh" {
        return None;
    }
    Some(RosterNode {
        initials: t[0].to_string(),
        hostname: t[1].to_string(),
        ram_gb: t[5].parse().ok()?,
        wiring_state: t[7].to_string(),
        ip: if t[9] == "-" {
            None
        } else {
            Some(t[9].to_string())
        },
    })
}

/// The grid's presentation order (wiring.phext rows 1–6). Names,
/// addresses, RAM and states all come from the file itself; this
/// only fixes the order rows print in.
const GRID_ORDER: [&str; 24] = [
    "NK", "AH", "LP", "EP", "OF", "HV", "DW", "BW", "PV", "TF", "LO", "FF", "AC", "VG", "WL", "RM",
    "AM", "JK", "GW", "QH", "YM", "KO", "MB", "SC",
];

fn read_wiring(path: &Path) -> Result<Vec<RosterNode>, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "roster of record {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "roster of record {} is not valid UTF-8 — libphext parses text phexts; no roster is guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let mut nodes: Vec<RosterNode> = Vec::new();
    for scroll in scrolls.values() {
        for line in scroll.lines() {
            if let Some(n) = parse_node_line(line.trim()) {
                nodes.push(n);
            }
        }
    }
    if nodes.is_empty() {
        return Err(format!(
            "roster of record {} parsed but carries no node: lines — the declared form is `node: <INITIALS> <hostname> col <n> ram <GB> state <state> ssh <ip|->`",
            path.display()
        ));
    }
    nodes.sort_by_key(|n| {
        GRID_ORDER
            .iter()
            .position(|i| *i == n.initials)
            .unwrap_or(GRID_ORDER.len())
    });
    Ok(nodes)
}

// ---------------------------------------------------------------------------
// The deployment register — inkling-apis.phext. One scroll per node,
// keyed by its `Initials:` field line; the Home scroll carries the
// as-of stamp and no Initials line, so it never parses as a node.
// A scroll missing its State line is not a status and is skipped,
// never guessed at.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct NodeStatus {
    node_name: String,
    state: String,
    quant: Option<String>,
    bytes: Option<u64>,
    target_bytes: Option<u64>,
    percent: Option<f64>,
    completed_unix: Option<u64>,
    ee: Option<u64>,
    detail: String,
}

fn parse_status(text: &str) -> Option<(String, NodeStatus)> {
    let mut initials = None;
    let mut st = NodeStatus {
        node_name: String::new(),
        state: String::new(),
        quant: None,
        bytes: None,
        target_bytes: None,
        percent: None,
        completed_unix: None,
        ee: None,
        detail: String::new(),
    };
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Initials: ") {
            initials = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Node: ") {
            st.node_name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("State: ") {
            st.state = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Quant: ") {
            st.quant = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Bytes: ") {
            st.bytes = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Target bytes: ") {
            st.target_bytes = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Percent: ") {
            st.percent = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Completed (unix): ") {
            st.completed_unix = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("EE: ") {
            st.ee = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("Detail: ") {
            st.detail = v.trim().to_string();
        }
    }
    let initials = initials?;
    if st.state.is_empty() {
        return None;
    }
    Some((initials, st))
}

#[derive(Debug)]
struct Register {
    statuses: HashMap<String, NodeStatus>,
    as_of_unix: Option<u64>,
    as_of_local: Option<String>,
    sha256: String,
}

fn read_register(path: &Path) -> Result<Register, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "inkling deployment register {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let sha256 = crate::data::sha256_hex(&raw);
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "inkling deployment register {} is not valid UTF-8 — libphext parses text phexts; no statuses are guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let mut statuses = HashMap::new();
    let mut as_of_unix = None;
    let mut as_of_local = None;
    for scroll in scrolls.values() {
        if let Some((ini, st)) = parse_status(scroll) {
            statuses.insert(ini, st);
        }
        for line in scroll.lines() {
            if let Some(v) = line.strip_prefix("As of (unix): ") {
                as_of_unix = v.trim().parse::<u64>().ok();
            } else if let Some(v) = line.strip_prefix("As of: ") {
                as_of_local = Some(v.trim().to_string());
            }
        }
    }
    Ok(Register {
        statuses,
        as_of_unix,
        as_of_local,
        sha256,
    })
}

// ---------------------------------------------------------------------------
// The merge — pure, so the row rules are testable without live sources.
// ---------------------------------------------------------------------------

fn build_rows(roster: &[RosterNode], reg: &Register) -> Vec<Value> {
    roster
        .iter()
        .map(|n| {
            let endpoint = n.ip.as_ref().map(|ip| format!("http://{}:{}", ip, PORT));
            match reg.statuses.get(&n.initials) {
                Some(st) => {
                    // The recorded EE wins; where a completion stamp
                    // exists without one, compute it — the epoch is a
                    // function of the timestamp, never a guess.
                    let ee = st.ee.or_else(|| st.completed_unix.map(ee_of));
                    json!({
                        "initials": n.initials,
                        "name": if n.hostname != "unknown" { n.hostname.clone() } else { st.node_name.clone() },
                        "ram_gb": n.ram_gb,
                        "wiring_state": n.wiring_state,
                        "ip": n.ip,
                        "endpoint": endpoint,
                        "quant": st.quant,
                        "status": st.state,
                        "percent": st.percent,
                        "bytes": st.bytes,
                        "target_bytes": st.target_bytes,
                        "completed_unix": st.completed_unix,
                        "completed_local": st.completed_unix.map(fmt_stamp),
                        "ee": ee,
                        "detail": st.detail,
                    })
                }
                None => json!({
                    "initials": n.initials,
                    "name": n.hostname,
                    "ram_gb": n.ram_gb,
                    "wiring_state": n.wiring_state,
                    "ip": n.ip,
                    "endpoint": endpoint,
                    "quant": null,
                    "status": "unrecorded",
                    "percent": null,
                    "bytes": null,
                    "target_bytes": null,
                    "completed_unix": null,
                    "completed_local": null,
                    "ee": null,
                    "detail": "no deployment state recorded for this node in the register",
                }),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The panel.
// ---------------------------------------------------------------------------

pub struct InklingPanel;

impl Panel for InklingPanel {
    fn id(&self) -> &'static str {
        "inkling"
    }
    fn title(&self) -> &'static str {
        "Inkling APIs"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let wiring_path = std::env::var("PHEXTRON_INKLING_WIRING_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/phext-vm/wiring.phext".to_string());
        let register_path = std::env::var("PHEXTRON_INKLING_APIS_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/phext-vm/inkling-apis.phext".to_string());

        let source_name = "wiring.phext — the roster of record (node: lines, parsed with libphext); inkling-apis.phext — the Inkling deployment register (one scroll per node)".to_string();
        let source_location = format!("roster {} · register {}", wiring_path, register_path);

        let degraded = |e: String| PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Degraded,
            source: SourceInfo {
                name: source_name.clone(),
                location: source_location.clone(),
                freshness: "a source is unavailable from here".to_string(),
            },
            body: json!({ "error": e }),
        };

        let roster = match read_wiring(Path::new(&wiring_path)) {
            Ok(r) => r,
            Err(e) => return degraded(e),
        };
        let reg = match read_register(Path::new(&register_path)) {
            Ok(r) => r,
            Err(e) => return degraded(e),
        };

        let rows = build_rows(&roster, &reg);
        let count = |s: &str| rows.iter().filter(|r| r["status"] == s).count();
        let file_age = std::fs::metadata(&register_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| age_from_unix(d.as_secs(), now))
            .unwrap_or_else(|| "unreadable".to_string());

        PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Live,
            source: SourceInfo {
                name: source_name,
                location: source_location,
                freshness: format!(
                    "deployment state as of {} (register evidence, not a live probe) · register file updated {} · sha256 {}…",
                    reg.as_of_local.clone().unwrap_or_else(|| "an unrecorded time".to_string()),
                    file_age,
                    &reg.sha256[..12]
                ),
            },
            body: json!({
                "summary": {
                    "nodes": rows.len(),
                    "serving": count("serving"),
                    "installing": count("installing"),
                    "held": count("held"),
                    "blocked": count("blocked"),
                    "unrecorded": count("unrecorded"),
                },
                "as_of": {
                    "unix": reg.as_of_unix,
                    "local": reg.as_of_local,
                    "age": reg.as_of_unix.map(|t| age_from_unix(t, now)),
                },
                "register": {
                    "sha256": reg.sha256,
                    "file_age": file_age,
                },
                "port": PORT,
                "caveat": "the ranch HTTP proxy intercepts tailnet-IP requests — bypass it for these endpoints",
                "example": "curl --noproxy '*' http://100.120.211.72:8090/health",
                "rows": rows,
            }),
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
            "phextron-inkling-test-{}-{}",
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

    // -- the epoch ------------------------------------------------------

    #[test]
    fn ee_of_matches_the_anchor_rule() {
        assert_eq!(ee_of(0), 0);
        assert_eq!(ee_of(EE_ANCHOR_UNIX - 1), 0);
        assert_eq!(ee_of(EE_ANCHOR_UNIX), 1);
        assert_eq!(ee_of(EE_ANCHOR_UNIX + 210), 1); // EE 1's centre
        assert_eq!(ee_of(EE_ANCHOR_UNIX + 420), 2);
        // The two serving nodes finished before the anchor: EE 0,
        // honestly — never rounded up into EE 1.
        assert_eq!(ee_of(1_795_320_475), 0); // elven-path completion
        assert_eq!(ee_of(1_795_320_807), 0); // orchid-flow completion
    }

    #[test]
    fn stamp_formats_chicago_time() {
        assert_eq!(fmt_stamp(1_795_328_040), "2026-11-22 00:14 CST");
        assert_eq!(fmt_stamp(1_795_320_475), "2026-11-21 22:07 CST");
    }

    // -- the roster -----------------------------------------------------

    #[test]
    fn node_lines_parse_in_their_declared_form() {
        let n = parse_node_line("node: NK ni-knight col 1 ram 48 state online ssh 100.93.140.59")
            .expect("online node");
        assert_eq!(n.initials, "NK");
        assert_eq!(n.hostname, "ni-knight");
        assert_eq!(n.ram_gb, 48);
        assert_eq!(n.ip.as_deref(), Some("100.93.140.59"));

        let dark = parse_node_line("node: DW delta-wood col 3 ram 32 state dark ssh -")
            .expect("dark node");
        assert_eq!(dark.ip, None);
        assert_eq!(dark.wiring_state, "dark");

        let unlocated =
            parse_node_line("node: TF unknown col 2 ram 16 state unlocated ssh -")
                .expect("unlocated node");
        assert_eq!(unlocated.hostname, "unknown");

        let refused =
            parse_node_line("node: GW great-white col 3 ram 32 state key-refused ssh -")
                .expect("key-refused node");
        assert_eq!(refused.wiring_state, "key-refused");

        // Prose and other machine-checked lines are not nodes.
        assert_eq!(parse_node_line("usb4: NK-AH subnet 10.54.11.0/24 NK 10.54.11.1 AH 10.54.11.2 expect-gbit 10-40"), None);
        assert_eq!(parse_node_line("node: NK ni-knight col 1 ram 48 state online"), None);
    }

    const WIRING_FIXTURE: &str = "Row 1\n=====\n\nnode: NK ni-knight col 1 ram 48 state online ssh 100.93.140.59\nnode: EP elven-path col 4 ram 32 state online ssh 100.120.211.72\nnode: TF unknown col 2 ram 16 state unlocated ssh -\n";

    #[test]
    fn wiring_reads_the_roster_in_grid_order() {
        let dir = temp_dir("wiring");
        write(&dir, "wiring.phext", WIRING_FIXTURE.as_bytes());
        let roster = read_wiring(&dir.join("wiring.phext")).expect("roster");
        assert_eq!(roster.len(), 3);
        assert_eq!(roster[0].initials, "NK");
        assert_eq!(roster[1].initials, "EP");
        assert_eq!(roster[2].initials, "TF");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- the register -----------------------------------------------------

    const HOME: &str = "# inkling-apis.phext - the Inkling fleet deployment register\n\nAs of: 2026-11-22 00:14 CST\nAs of (unix): 1795328040\n";

    fn status_scroll(ini: &str, name: &str, fields: &[(&str, &str)]) -> String {
        let mut s = format!("# Inkling API - {} ({})\n\nInitials: {}\nNode: {}\n", name, ini, ini, name);
        for (k, v) in fields {
            s += &format!("{}: {}\n", k, v);
        }
        s
    }

    #[test]
    fn register_parses_statuses_home_and_skips_malformed() {
        let dir = temp_dir("register");
        let text = format!(
            "{}\u{17}{}\u{17}{}\u{17}{}",
            HOME,
            status_scroll("EP", "elven-path", &[
                ("State", "serving"),
                ("Quant", "AD-Q4_K"),
                ("Completed (unix)", "1795320475"),
                ("EE", "0"),
                ("Detail", "API verified"),
            ]),
            status_scroll("NK", "ni-knight", &[
                ("State", "installing"),
                ("Quant", "AD-Q4_K"),
                ("Bytes", "3467378688"),
                ("Target bytes", "16525559776"),
                ("Percent", "21.0"),
                ("Detail", "pulling"),
            ]),
            "# Inkling API - broken (XX)\n\nInitials: XX\n(no state line)\n"
        );
        write(&dir, "inkling-apis.phext", text.as_bytes());
        let reg = read_register(&dir.join("inkling-apis.phext")).expect("register");
        assert_eq!(reg.statuses.len(), 2);
        assert_eq!(reg.as_of_unix, Some(1_795_328_040));
        assert_eq!(reg.as_of_local.as_deref(), Some("2026-11-22 00:14 CST"));
        let ep = &reg.statuses["EP"];
        assert_eq!(ep.state, "serving");
        assert_eq!(ep.ee, Some(0));
        assert_eq!(ep.completed_unix, Some(1_795_320_475));
        let nk = &reg.statuses["NK"];
        assert_eq!(nk.percent, Some(21.0));
        assert_eq!(reg.sha256, crate::data::sha256_hex(text.as_bytes()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_sources_are_errors_that_name_the_path() {
        let dir = std::env::temp_dir().join("phextron-inkling-test-definitely-absent.phext");
        let err = read_register(&dir).expect_err("register must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
        let err = read_wiring(&dir).expect_err("roster must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
    }

    // -- the merge --------------------------------------------------------

    fn roster_fixture() -> Vec<RosterNode> {
        vec![
            parse_node_line("node: EP elven-path col 4 ram 32 state online ssh 100.120.211.72").unwrap(),
            parse_node_line("node: TF unknown col 2 ram 16 state unlocated ssh -").unwrap(),
            parse_node_line("node: QH quacked-hill col 4 ram 48 state online ssh 100.109.77.116").unwrap(),
        ]
    }

    fn register_fixture() -> Register {
        let mut statuses = HashMap::new();
        statuses.insert(
            "EP".to_string(),
            NodeStatus {
                node_name: "elven-path".to_string(),
                state: "serving".to_string(),
                quant: Some("AD-Q4_K".to_string()),
                bytes: Some(16_525_559_776),
                target_bytes: Some(16_525_559_776),
                percent: Some(100.0),
                completed_unix: Some(1_795_320_475),
                ee: Some(0),
                detail: "API verified".to_string(),
            },
        );
        statuses.insert(
            "TF".to_string(),
            NodeStatus {
                node_name: "trust-fall".to_string(),
                state: "blocked".to_string(),
                quant: None,
                bytes: None,
                target_bytes: None,
                percent: None,
                completed_unix: None,
                ee: None,
                detail: "unlocated".to_string(),
            },
        );
        Register {
            statuses,
            as_of_unix: Some(1_795_328_040),
            as_of_local: Some("2026-11-22 00:14 CST".to_string()),
            sha256: "fixture".to_string(),
        }
    }

    #[test]
    fn rows_merge_roster_and_register() {
        let rows = build_rows(&roster_fixture(), &register_fixture());
        assert_eq!(rows.len(), 3);
        // Serving: endpoint formed from the roster IP, EE recorded.
        assert_eq!(rows[0]["status"], "serving");
        assert_eq!(rows[0]["endpoint"], "http://100.120.211.72:8090");
        assert_eq!(rows[0]["ee"], 0);
        assert_eq!(rows[0]["completed_local"], "2026-11-21 22:07 CST");
        // Blocked + unlocated: the register's name stands in for the
        // roster's "unknown", and no endpoint is invented.
        assert_eq!(rows[1]["status"], "blocked");
        assert_eq!(rows[1]["name"], "trust-fall");
        assert_eq!(rows[1]["endpoint"], Value::Null);
        // On the roster but absent from the register: unrecorded,
        // with its endpoint still shown — never a guessed state.
        assert_eq!(rows[2]["status"], "unrecorded");
        assert_eq!(rows[2]["endpoint"], "http://100.109.77.116:8090");
    }

    #[test]
    fn ee_is_computed_when_a_completion_stamp_lacks_one() {
        let mut reg = register_fixture();
        reg.statuses.get_mut("EP").unwrap().ee = None;
        let rows = build_rows(&roster_fixture(), &reg);
        assert_eq!(rows[0]["ee"], 0); // 1795320475 is pre-anchor: EE 0
    }
}
