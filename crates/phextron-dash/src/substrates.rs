//! Substrates panel — substrate coverage at a glance (W4 plan §1.10,
//! task 31.8.11/9.7.13/8.7.10).
//!
//! Will's direction (2026-11-21 23:04 CST): "Phextron should show me,
//! at a glance, if any substrates are missing dispatched work and if
//! I've checked the token budget within the substrate's reset
//! window." One row per substrate (Claude, Grok, Muse, ChatGPT,
//! Google/Antigravity, Local), exactly two health signals per row:
//!
//! - **Dispatched work** — live dispatched work in the substrate's
//!   current reset window, or **MISSING** (budget expiring unused).
//!   The signal is derived only from a live source that can actually
//!   answer for the substrate; where no such source exists yet the
//!   cell reads **source unavailable** and names the gap — never a
//!   confident green from absent data. Tonight's honest source map:
//!   - Claude: the §1.9 session registry is the planned source; it is
//!     not built yet and the per-node Claude distribution is still
//!     rolling out → unavailable.
//!   - Grok: work rides the /chat/grok mailbox and wbic16/grok-tools;
//!     neither is a console-readable dispatch register → unavailable.
//!   - Muse: no register records work dispatched to this substrate
//!     in a console-readable form → unavailable.
//!   - ChatGPT: a context reservoir, never a burn target — its cell
//!     shows the reservoir state and is never flagged MISSING.
//!   - Google/Antigravity: dispatches ride wbic16/neon-tools and the
//!     /chat/neon mailbox; no console-readable register yet →
//!     unavailable.
//!   - Local: ranch-batchq, read exactly the way the video panel
//!     reads it (live queue directories where configured, else the
//!     snapshot document) — pending/running jobs are dispatched work;
//!     an empty queue reads **idle**. Local has no window, so it is
//!     never flagged MISSING.
//!
//! - **Budget checked** — a budget check recorded in the substrate's
//!   current reset window (with its timestamp), or **NOT CHECKED**.
//!   The state is a recorded event, not a guess: checks live in
//!   `substrate-checks.phext`, one scroll per check, written by
//!   `tools/mark-substrate-check.py` (the console is read-only today,
//!   so the script is the write path; steering passes log their checks
//!   through the same script). A check from a previous window never
//!   carries over — the derivation only accepts checks at or after
//!   the current window's start. Google/Antigravity rolls (no fixed
//!   window): the cell shows the most recent recorded check and is
//!   never flagged. ChatGPT is exempt (reservoir, no reset window);
//!   Local is unmetered (no budget check exists to make).
//!
//! Reset windows (standing constants from the substrate-steering
//! record, America/Chicago): Claude Wednesdays 22:00, Grok Thursdays
//! 20:23, Muse Fridays 16:10. The window arithmetic is implemented
//! directly on the US DST rule (CDT from the second Sunday of March
//! 08:00 UTC to the first Sunday of November 07:00 UTC), the same
//! approach the video panel uses — no tz database, no new dependency.
//!
//! Locations come from the environment, so the console reads each
//! source where it lives:
//!   PHEXTRON_SUBSTRATES_CHECKS_PHEXT  budget-check register (default
//!     /home/wbic16/phextron-data/phext-vm/substrate-checks.phext —
//!     the synced snapshot of ~/workspace/phext, the same corpus
//!     directory the data and video panels read; on the VM itself,
//!     point it at ~/workspace/phext/substrate-checks.phext)
//!   PHEXTRON_SUBSTRATES_BATCHQ_DIR    live ranch-batchq directory
//!     (no default — a guessed path on the wrong host would be a lie)
//!   PHEXTRON_SUBSTRATES_BATCHQ_JSON   batchq snapshot document
//!     (default /home/wbic16/phextron-data/batchq.json, the same
//!     snapshot the video panel reads)

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
// America/Chicago civil time, std-only (the video panel's approach:
// Howard Hinnant's civil algorithms + the US DST rule stated directly).
// ---------------------------------------------------------------------------

/// Days since the Unix epoch for a civil date (Howard Hinnant's
/// algorithm) — the same implementation the video panel carries.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverse of `days_from_civil`.
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

/// Day of week for a days-since-epoch value, 0 = Sunday.
fn weekday_of(days: i64) -> i64 {
    (days + 4).rem_euclid(7) // 1970-01-01 was a Thursday
}

/// The Chicago UTC offset (seconds) in force at a unix instant: CDT
/// (−5h) from the second Sunday of March 08:00 UTC to the first
/// Sunday of November 07:00 UTC, CST (−6h) otherwise.
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct ChicagoLocal {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    weekday: i64, // 0 = Sunday
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
        sec: (secs % 60) as u32,
        weekday: weekday_of(days),
        offset,
    }
}

/// A Chicago wall-clock time as a unix instant. Both candidate
/// offsets are tried and the one in force at the result wins; the
/// reset times this is used with (16:10, 20:23, 22:00) never fall in
/// the spring-forward gap or the fall-back repeated hour.
fn unix_from_local(year: i64, month: u32, day: u32, hour: u32, min: u32) -> i64 {
    let base = days_from_civil(year, month, day) * 86_400 + hour as i64 * 3600 + min as i64 * 60;
    for offset in [-6 * 3600, -5 * 3600] {
        let t = base - offset;
        if chicago_offset(t) == offset {
            return t;
        }
    }
    base + 6 * 3600 // unreachable for the times used; CST reading
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn tz_abbr(offset: i64) -> &'static str {
    if offset == -5 * 3600 {
        "CDT"
    } else {
        "CST"
    }
}

/// "Wed Nov 18, 22:00 CST" — a window boundary on the panel's face.
fn fmt_window_local(t: u64) -> String {
    let l = local_from_unix(t as i64);
    format!(
        "{} {} {}, {:02}:{:02} {}",
        WEEKDAYS[l.weekday as usize],
        MONTHS[(l.month - 1) as usize],
        l.day,
        l.hour,
        l.min,
        tz_abbr(l.offset)
    )
}

/// "2026-11-21 21:03 CST" — a recorded check's stamp, the same shape
/// the register's own human line carries.
fn fmt_stamp(t: u64) -> String {
    let l = local_from_unix(t as i64);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} {}",
        l.year,
        l.month,
        l.day,
        l.hour,
        l.min,
        tz_abbr(l.offset)
    )
}

/// The start of the current weekly reset window: the most recent
/// instant ≤ now whose Chicago wall time is `weekday` at `hh:mm`.
fn window_start(now: u64, weekday: i64, hh: u32, mm: u32) -> u64 {
    let loc = local_from_unix(now as i64);
    let today = days_from_civil(loc.year, loc.month, loc.day);
    for delta in 0..=7 {
        let date = today - delta;
        if weekday_of(date) == weekday {
            let (y, m, d) = civil_from_days(date);
            let t = unix_from_local(y, m, d, hh, mm);
            if t <= now as i64 {
                return t as u64;
            }
        }
    }
    unreachable!("a weekly weekday recurs within 7 days")
}

/// "3d 4h" / "5h 12m" / "42m" — time until the next reset.
fn fmt_duration(secs: u64) -> String {
    if secs >= 86_400 {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    } else if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}m", secs / 60)
    }
}

// ---------------------------------------------------------------------------
// The substrate table — the §1.10 rows and their standing constants.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Window {
    /// Weekly reset: (weekday 0=Sunday, hour, minute) in Chicago.
    Weekly(i64, u32, u32),
    /// No fixed window; quota rolls.
    Rolling,
    /// No window at all (reservoir or unmetered).
    None,
}

#[derive(Clone, Copy)]
struct Substrate {
    slug: &'static str,
    label: &'static str,
    window: Window,
    /// Why the dispatched-work cell cannot be answered yet, for the
    /// substrates with no live dispatch source. `None` where a source
    /// is wired (Local) or the row is exempt by design (ChatGPT).
    dispatch_gap: Option<&'static str>,
}

const SUBSTRATES: [Substrate; 6] = [
    Substrate {
        slug: "claude",
        label: "Claude",
        window: Window::Weekly(3, 22, 0), // Wednesdays 22:00
        dispatch_gap: Some(
            "the §1.9 session registry is the planned source — it is not built yet, and the per-node Claude distribution is still rolling out",
        ),
    },
    Substrate {
        slug: "grok",
        label: "Grok",
        window: Window::Weekly(4, 20, 23), // Thursdays 20:23
        dispatch_gap: Some(
            "no dispatch register — work rides the /chat/grok mailbox and wbic16/grok-tools, neither of which the console can read as live dispatch state",
        ),
    },
    Substrate {
        slug: "muse",
        label: "Muse",
        window: Window::Weekly(5, 16, 10), // Fridays 16:10
        dispatch_gap: Some(
            "no register records work dispatched to the Muse substrate in a form the console can read",
        ),
    },
    Substrate {
        slug: "chatgpt",
        label: "ChatGPT",
        window: Window::None,
        dispatch_gap: None, // reservoir by design — never flagged
    },
    Substrate {
        slug: "google",
        label: "Google/Antigravity",
        window: Window::Rolling,
        dispatch_gap: Some(
            "no dispatch register — work rides wbic16/neon-tools and the /chat/neon mailbox, neither of which the console can read as live dispatch state",
        ),
    },
    Substrate {
        slug: "local",
        label: "Local",
        window: Window::None,
        dispatch_gap: None, // wired to ranch-batchq below
    },
];

// ---------------------------------------------------------------------------
// The budget-check register (substrate-checks.phext).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct Check {
    substrate: String,
    unix: u64,
    recorded_by: String,
}

/// Parse one check scroll. The signature is the pair of field lines
/// the writer emits — `Substrate: <slug>` and `Checked at (unix):
/// <n>`; the register's Home scroll carries neither, so it never
/// parses as a check. A scroll missing either line is not a check and
/// is skipped, never guessed at.
fn parse_check(text: &str) -> Option<Check> {
    let mut substrate = None;
    let mut unix = None;
    let mut recorded_by = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Substrate: ") {
            substrate = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Checked at (unix): ") {
            unix = v.trim().parse::<u64>().ok();
        } else if let Some(v) = line.strip_prefix("Recorded by: ") {
            recorded_by = v.trim().to_string();
        }
    }
    Some(Check {
        substrate: substrate?,
        unix: unix?,
        recorded_by,
    })
}

#[derive(Debug)]
struct ChecksRegister {
    checks: Vec<Check>,
    sha256: String,
}

/// Read the budget-check register with libphext (`explode`, the
/// reference parser). An unreadable or non-UTF-8 register is an
/// `Err` — the panel degrades rather than deriving budget states
/// from nothing.
fn read_checks(path: &Path) -> Result<ChecksRegister, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "substrate-checks register {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let sha256 = crate::data::sha256_hex(&raw);
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "substrate-checks register {} is not valid UTF-8 — libphext parses text phexts; no checks are guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let checks = scrolls.values().filter_map(|s| parse_check(s)).collect();
    Ok(ChecksRegister { checks, sha256 })
}

/// The latest recorded check for a substrate, optionally restricted
/// to checks at or after `since`.
fn latest_check(checks: &[Check], slug: &str, since: Option<u64>) -> Option<u64> {
    checks
        .iter()
        .filter(|c| c.substrate == slug)
        .map(|c| c.unix)
        .filter(|t| since.map_or(true, |s| *t >= s))
        .max()
}

// ---------------------------------------------------------------------------
// ranch-batchq — the Local row's dispatched-work source, read exactly
// the way the video panel reads it (live dirs, else the snapshot).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct BatchqCounts {
    pending: u64,
    running: u64,
    done: u64,
    failed: u64,
}

#[derive(Clone, Debug)]
enum BatchqRead {
    LiveDir {
        counts: BatchqCounts,
    },
    Snapshot {
        counts: BatchqCounts,
        captured_at: String,
    },
    Degraded(String),
}

fn count_dir(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map(|rd| rd.count() as u64).unwrap_or(0)
}

fn counts_from_json(v: &Value) -> Option<BatchqCounts> {
    let c = v.get("counts")?;
    let get = |k: &str| c.get(k).and_then(|x| x.as_u64());
    Some(BatchqCounts {
        pending: get("pending")?,
        running: get("running")?,
        done: get("done")?,
        failed: get("failed")?,
    })
}

fn read_batchq(dir: Option<&Path>, snapshot: &Path) -> BatchqRead {
    if let Some(d) = dir {
        if d.is_dir() {
            return BatchqRead::LiveDir {
                counts: BatchqCounts {
                    pending: count_dir(&d.join("pending")),
                    running: count_dir(&d.join("running")),
                    done: count_dir(&d.join("done")),
                    failed: count_dir(&d.join("failed")),
                },
            };
        }
        return BatchqRead::Degraded(format!(
            "ranch-batchq live read failed: queue directory {} is not present on this host",
            d.display()
        ));
    }
    match std::fs::read_to_string(snapshot) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(v) => match (counts_from_json(&v), v.get("captured_at").and_then(|x| x.as_str())) {
                (Some(counts), Some(captured)) => BatchqRead::Snapshot {
                    counts,
                    captured_at: captured.to_string(),
                },
                _ => BatchqRead::Degraded(format!(
                    "ranch-batchq snapshot {} does not carry the counts/captured_at shape the video panel reads",
                    snapshot.display()
                )),
            },
            Err(e) => BatchqRead::Degraded(format!(
                "ranch-batchq snapshot {} is not parseable JSON: {}",
                snapshot.display(),
                e
            )),
        },
        Err(e) => BatchqRead::Degraded(format!(
            "ranch-batchq on rock-mountain (~/ranch-batchq) is not readable from this host: no live queue directory is configured (PHEXTRON_SUBSTRATES_BATCHQ_DIR) and no snapshot document is present at {} (PHEXTRON_SUBSTRATES_BATCHQ_JSON): {}",
            snapshot.display(),
            e
        )),
    }
}

// ---------------------------------------------------------------------------
// The two derivations — pure functions of (substrate, sources, now),
// so the window and flag rules are testable without any live source.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum DispatchState {
    Present,
    Missing,
    Unavailable,
    Reservoir,
    Idle,
}

impl DispatchState {
    fn as_str(&self) -> &'static str {
        match self {
            DispatchState::Present => "present",
            DispatchState::Missing => "missing",
            DispatchState::Unavailable => "unavailable",
            DispatchState::Reservoir => "reservoir",
            DispatchState::Idle => "idle",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Dispatch {
    state: DispatchState,
    detail: String,
    source: String,
}

/// The dispatched-work derivation for the windowed/reservoir rows.
/// `verdict` is a live source's answer — `Some(true)`: work is
/// dispatched in the current window; `Some(false)`: the source can
/// see the substrate and there is none; `None`: no live source can
/// answer yet. Only a real `Some(false)` may read MISSING; `None`
/// reads unavailable, with the gap named.
fn derive_dispatch(sub: &Substrate, verdict: Option<bool>) -> Dispatch {
    if sub.slug == "chatgpt" {
        return Dispatch {
            state: DispatchState::Reservoir,
            detail: "context reservoir — used when context is the job; never a burn target, never flagged".to_string(),
            source: "substrate-steering record (reservoir by design)".to_string(),
        };
    }
    match verdict {
        Some(true) => Dispatch {
            state: DispatchState::Present,
            detail: "live dispatched work in the current window".to_string(),
            source: "live dispatch source".to_string(),
        },
        Some(false) => Dispatch {
            state: DispatchState::Missing,
            detail: "no dispatched work in the current window — budget expiring unused".to_string(),
            source: "live dispatch source".to_string(),
        },
        None => Dispatch {
            state: DispatchState::Unavailable,
            detail: format!("source unavailable — {}", sub.dispatch_gap.unwrap_or("no live dispatch source")),
            source: "no console-readable source yet".to_string(),
        },
    }
}

/// The Local row's dispatched-work derivation, from the batchq read.
fn derive_local_dispatch(read: &BatchqRead) -> Dispatch {
    let describe = |counts: &BatchqCounts, how: &str| {
        if counts.running + counts.pending > 0 {
            Dispatch {
                state: DispatchState::Present,
                detail: format!(
                    "{} running · {} pending in ranch-batchq ({})",
                    counts.running, counts.pending, how
                ),
                source: "ranch-batchq — the standing local batch queue".to_string(),
            }
        } else {
            Dispatch {
                state: DispatchState::Idle,
                detail: format!(
                    "queue idle — 0 running · 0 pending ({} done · {} failed) ({})",
                    counts.done, counts.failed, how
                ),
                source: "ranch-batchq — the standing local batch queue".to_string(),
            }
        }
    };
    match read {
        BatchqRead::LiveDir { counts } => describe(counts, "live queue directories"),
        BatchqRead::Snapshot {
            counts,
            captured_at,
        } => describe(counts, &format!("snapshot captured {}", captured_at)),
        BatchqRead::Degraded(why) => Dispatch {
            state: DispatchState::Unavailable,
            detail: format!("source unavailable — {}", why),
            source: "ranch-batchq — the standing local batch queue".to_string(),
        },
    }
}

#[derive(Clone, Debug, PartialEq)]
enum BudgetState {
    /// A check recorded inside the current window.
    Checked(u64),
    /// No check inside the current window.
    NotChecked,
    /// Rolling window: the latest check whenever it was, if any.
    Rolling(Option<u64>),
    /// Reservoir: exempt from the flag; latest check shown if any.
    Exempt(Option<u64>),
    /// Unmetered: no budget check exists to make.
    NotApplicable,
}

/// The budget derivation. For a weekly window, only a check at or
/// after the window's start counts — a previous window's check never
/// carries over.
fn derive_budget(sub: &Substrate, checks: &[Check], now: u64) -> BudgetState {
    match sub.window {
        Window::Weekly(weekday, hh, mm) => {
            let start = window_start(now, weekday, hh, mm);
            match latest_check(checks, sub.slug, Some(start)) {
                Some(t) => BudgetState::Checked(t),
                None => BudgetState::NotChecked,
            }
        }
        Window::Rolling => BudgetState::Rolling(latest_check(checks, sub.slug, None)),
        Window::None => {
            if sub.slug == "chatgpt" {
                BudgetState::Exempt(latest_check(checks, sub.slug, None))
            } else {
                BudgetState::NotApplicable
            }
        }
    }
}

fn budget_json(state: &BudgetState, now: u64) -> Value {
    match state {
        BudgetState::Checked(t) => json!({
            "state": "checked",
            "checked_unix": t,
            "checked_local": fmt_stamp(*t),
            "age": age_from_unix(*t, now),
        }),
        BudgetState::NotChecked => json!({ "state": "not_checked" }),
        BudgetState::Rolling(t) => json!({
            "state": "rolling",
            "checked_unix": t,
            "checked_local": t.map(fmt_stamp),
            "age": t.map(|t| age_from_unix(t, now)),
        }),
        BudgetState::Exempt(t) => json!({
            "state": "exempt",
            "checked_unix": t,
            "checked_local": t.map(fmt_stamp),
            "age": t.map(|t| age_from_unix(t, now)),
        }),
        BudgetState::NotApplicable => json!({ "state": "na" }),
    }
}

fn window_json(sub: &Substrate, now: u64) -> Value {
    match sub.window {
        Window::Weekly(weekday, hh, mm) => {
            let start = window_start(now, weekday, hh, mm);
            let next = start + 7 * 86_400;
            json!({
                "kind": "weekly",
                "label": format!("resets {} {:02}:{:02} America/Chicago", WEEKDAYS[weekday as usize], hh, mm),
                "start_unix": start,
                "start_local": fmt_window_local(start),
                "next_local": fmt_window_local(next),
                "resets_in": fmt_duration(next.saturating_sub(now)),
            })
        }
        Window::Rolling => json!({
            "kind": "rolling",
            "label": "rolling — no fixed reset window",
        }),
        Window::None => json!({
            "kind": "none",
            "label": if sub.slug == "chatgpt" {
                "context reservoir — no reset window"
            } else {
                "unmetered — no reset window"
            },
        }),
    }
}

// ---------------------------------------------------------------------------
// The panel.
// ---------------------------------------------------------------------------

pub struct SubstratesPanel;

impl Panel for SubstratesPanel {
    fn id(&self) -> &'static str {
        "substrates"
    }
    fn title(&self) -> &'static str {
        "Substrates"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let checks_path = std::env::var("PHEXTRON_SUBSTRATES_CHECKS_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| {
                "/home/wbic16/phextron-data/phext-vm/substrate-checks.phext".to_string()
            });
        let batchq_dir: Option<PathBuf> = std::env::var("PHEXTRON_SUBSTRATES_BATCHQ_DIR")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from);
        let batchq_snapshot = std::env::var("PHEXTRON_SUBSTRATES_BATCHQ_JSON")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/batchq.json".to_string());

        let source_name =
            "substrate-checks.phext — the budget-check register (one scroll per recorded check), parsed with libphext; ranch-batchq for the Local row"
                .to_string();
        let source_location = format!("register {}", checks_path);

        let file_age = std::fs::metadata(&checks_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| age_from_unix(d.as_secs(), now))
            .unwrap_or_else(|| "unreadable".to_string());

        match read_checks(Path::new(&checks_path)) {
            Ok(reg) => {
                let batchq = read_batchq(batchq_dir.as_deref(), Path::new(&batchq_snapshot));
                let mut missing = 0u64;
                let mut not_checked = 0u64;
                let mut unavailable = 0u64;
                let rows: Vec<Value> = SUBSTRATES
                    .iter()
                    .map(|sub| {
                        let dispatch = if sub.slug == "local" {
                            derive_local_dispatch(&batchq)
                        } else {
                            // No windowed substrate has a live dispatch
                            // source tonight (see the module header);
                            // the derivation is wired so a source's
                            // verdict drops in here when one lands.
                            derive_dispatch(sub, None)
                        };
                        match dispatch.state {
                            DispatchState::Missing => missing += 1,
                            DispatchState::Unavailable => unavailable += 1,
                            _ => {}
                        }
                        let budget = derive_budget(sub, &reg.checks, now);
                        if budget == BudgetState::NotChecked {
                            not_checked += 1;
                        }
                        json!({
                            "substrate": sub.slug,
                            "label": sub.label,
                            "window": window_json(sub, now),
                            "dispatch": {
                                "state": dispatch.state.as_str(),
                                "detail": dispatch.detail,
                                "source": dispatch.source,
                            },
                            "budget": budget_json(&budget, now),
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
                            "register read at request time · {} checks recorded · snapshot file updated {} · sha256 {}…",
                            reg.checks.len(),
                            file_age,
                            &reg.sha256[..12]
                        ),
                    },
                    body: json!({
                        "summary": {
                            "missing": missing,
                            "not_checked": not_checked,
                            "unavailable": unavailable,
                        },
                        "checks_register": {
                            "checks_recorded": reg.checks.len(),
                            "sha256": reg.sha256,
                            "file_age": file_age,
                        },
                        "rows": rows,
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
                    freshness: "budget-check register unavailable from here".to_string(),
                },
                body: json!({ "error": e }),
            },
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
            "phextron-substrates-test-{}-{}",
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

    fn sub(slug: &str) -> &'static Substrate {
        SUBSTRATES.iter().find(|s| s.slug == slug).expect("substrate")
    }

    fn check(slug: &str, unix: u64) -> Check {
        Check {
            substrate: slug.to_string(),
            unix,
            recorded_by: "test".to_string(),
        }
    }

    // -- civil time ---------------------------------------------------------

    #[test]
    fn civil_roundtrip_and_known_weekdays() {
        for (y, m, d) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2026, 3, 8),
            (2026, 11, 21),
            (2100, 12, 31),
        ] {
            assert_eq!(civil_from_days(days_from_civil(y, m, d)), (y, m, d));
        }
        assert_eq!(weekday_of(days_from_civil(1970, 1, 1)), 4); // Thursday
        assert_eq!(weekday_of(days_from_civil(2026, 11, 18)), 3); // Wednesday
        assert_eq!(weekday_of(days_from_civil(2026, 11, 21)), 6); // Saturday
    }

    #[test]
    fn chicago_offset_follows_the_dst_rule() {
        // Mid-winter CST, mid-summer CDT.
        assert_eq!(chicago_offset(unix_from_local(2026, 1, 15, 12, 0)), -6 * 3600);
        assert_eq!(local_from_unix(unix_from_local(2026, 7, 1, 12, 0)).offset, -5 * 3600);
        // Spring forward 2026: CDT begins Sun Mar 8 at 08:00 UTC.
        let spring = days_from_civil(2026, 3, 8) * 86_400 + 8 * 3600;
        assert_eq!(chicago_offset(spring - 1), -6 * 3600);
        assert_eq!(chicago_offset(spring), -5 * 3600);
        // Fall back 2026: CDT ends Sun Nov 1 at 07:00 UTC.
        let fall = days_from_civil(2026, 11, 1) * 86_400 + 7 * 3600;
        assert_eq!(chicago_offset(fall - 1), -5 * 3600);
        assert_eq!(chicago_offset(fall), -6 * 3600);
    }

    #[test]
    fn local_roundtrip_keeps_wall_time() {
        for (y, m, d, hh, mm) in [
            (2026, 11, 21, 23, 4),
            (2026, 10, 21, 22, 0),
            (2026, 3, 11, 22, 0),
        ] {
            let t = unix_from_local(y, m, d, hh, mm);
            let l = local_from_unix(t);
            assert_eq!((l.year, l.month, l.day, l.hour, l.min), (y, m, d, hh, mm));
        }
    }

    // -- window arithmetic --------------------------------------------------

    #[test]
    fn claude_window_start_is_the_latest_wednesday_2200() {
        // Saturday 2026-11-21 23:04 CST → window opened Wed Nov 18 22:00 CST.
        let now = unix_from_local(2026, 11, 21, 23, 4) as u64;
        let start = window_start(now, 3, 22, 0);
        assert_eq!(start, unix_from_local(2026, 11, 18, 22, 0) as u64);
        // Exactly at a reset, the window starts now.
        let reset = unix_from_local(2026, 11, 18, 22, 0) as u64;
        assert_eq!(window_start(reset, 3, 22, 0), reset);
        // One minute before the reset, the window is last week's.
        assert_eq!(
            window_start(reset - 60, 3, 22, 0),
            reset - 7 * 86_400
        );
        // A CDT week: Wed Oct 21 2026 22:00 CDT (offset −5h).
        let now_cdt = unix_from_local(2026, 10, 24, 12, 0) as u64;
        let start_cdt = window_start(now_cdt, 3, 22, 0);
        assert_eq!(start_cdt, unix_from_local(2026, 10, 21, 22, 0) as u64);
        assert_eq!(local_from_unix(start_cdt as i64).offset, -5 * 3600);
        assert_eq!(local_from_unix(start_cdt as i64).hour, 22);
    }

    #[test]
    fn grok_and_muse_windows_use_their_own_cliffs() {
        // Saturday 2026-11-21 23:04 CST.
        let now = unix_from_local(2026, 11, 21, 23, 4) as u64;
        // Grok: Thu Nov 19 20:23 CST.
        assert_eq!(
            window_start(now, 4, 20, 23),
            unix_from_local(2026, 11, 19, 20, 23) as u64
        );
        // Muse: Fri Nov 20 16:10 CST.
        assert_eq!(
            window_start(now, 5, 16, 10),
            unix_from_local(2026, 11, 20, 16, 10) as u64
        );
    }

    // -- budget derivation --------------------------------------------------

    #[test]
    fn budget_check_straddling_the_reset() {
        // Muse's cliff: Friday 2026-11-20 16:10 CST.
        let r = unix_from_local(2026, 11, 20, 16, 10) as u64;
        let muse = sub("muse");
        // A check one minute BEFORE the reset belongs to the old
        // window: an hour after the reset it does not count.
        assert_eq!(
            derive_budget(muse, &[check("muse", r - 60)], r + 3600),
            BudgetState::NotChecked
        );
        // A check one minute AFTER the reset is in the new window.
        assert_eq!(
            derive_budget(muse, &[check("muse", r + 60)], r + 3600),
            BudgetState::Checked(r + 60)
        );
        // Before the reset, that same early check is still in-window.
        assert_eq!(
            derive_budget(muse, &[check("muse", r - 60)], r - 30),
            BudgetState::Checked(r - 60)
        );
        // A check exactly at the reset opens the new window.
        assert_eq!(
            derive_budget(muse, &[check("muse", r)], r),
            BudgetState::Checked(r)
        );
    }

    #[test]
    fn budget_previous_window_never_carries_over() {
        let r = unix_from_local(2026, 11, 20, 16, 10) as u64;
        let muse = sub("muse");
        // Only a check from the previous window exists.
        assert_eq!(
            derive_budget(muse, &[check("muse", r - 8 * 86_400)], r + 3600),
            BudgetState::NotChecked
        );
        // No checks at all.
        assert_eq!(derive_budget(muse, &[], r + 3600), BudgetState::NotChecked);
        // Another substrate's in-window check does not count.
        assert_eq!(
            derive_budget(muse, &[check("claude", r + 60)], r + 3600),
            BudgetState::NotChecked
        );
        // The latest in-window check wins.
        assert_eq!(
            derive_budget(
                muse,
                &[check("muse", r + 60), check("muse", r + 600)],
                r + 3600
            ),
            BudgetState::Checked(r + 600)
        );
    }

    #[test]
    fn budget_rolling_and_exempt_rows_are_never_flagged() {
        let now = unix_from_local(2026, 11, 21, 23, 4) as u64;
        // Google rolls: latest check whenever it was; none recorded → None.
        assert_eq!(
            derive_budget(sub("google"), &[check("google", 1000)], now),
            BudgetState::Rolling(Some(1000))
        );
        assert_eq!(
            derive_budget(sub("google"), &[], now),
            BudgetState::Rolling(None)
        );
        // ChatGPT is exempt, with or without a recorded check.
        assert_eq!(
            derive_budget(sub("chatgpt"), &[], now),
            BudgetState::Exempt(None)
        );
        assert_eq!(
            derive_budget(sub("chatgpt"), &[check("chatgpt", 42)], now),
            BudgetState::Exempt(Some(42))
        );
        // Local is unmetered: no budget state exists to derive.
        assert_eq!(
            derive_budget(sub("local"), &[check("local", 42)], now),
            BudgetState::NotApplicable
        );
    }

    // -- dispatch derivation --------------------------------------------------

    #[test]
    fn dispatch_only_a_real_source_can_flag_missing() {
        let claude = sub("claude");
        assert_eq!(derive_dispatch(claude, None).state, DispatchState::Unavailable);
        assert_eq!(derive_dispatch(claude, Some(true)).state, DispatchState::Present);
        assert_eq!(derive_dispatch(claude, Some(false)).state, DispatchState::Missing);
        // The unavailable cell names the gap on its face.
        assert!(derive_dispatch(claude, None)
            .detail
            .contains("session registry"));
    }

    #[test]
    fn dispatch_chatgpt_is_never_missing() {
        let chatgpt = sub("chatgpt");
        for verdict in [None, Some(true), Some(false)] {
            assert_eq!(
                derive_dispatch(chatgpt, verdict).state,
                DispatchState::Reservoir
            );
        }
    }

    #[test]
    fn dispatch_local_reads_the_queue() {
        let present = BatchqRead::LiveDir {
            counts: BatchqCounts { pending: 2, running: 1, done: 9, failed: 0 },
        };
        let d = derive_local_dispatch(&present);
        assert_eq!(d.state, DispatchState::Present);
        assert!(d.detail.contains("1 running · 2 pending"));

        let idle = BatchqRead::Snapshot {
            counts: BatchqCounts { pending: 0, running: 0, done: 9, failed: 1 },
            captured_at: "2026-11-21T20:00:00Z".to_string(),
        };
        let d = derive_local_dispatch(&idle);
        assert_eq!(d.state, DispatchState::Idle);
        assert!(d.detail.contains("snapshot captured"));

        let degraded = BatchqRead::Degraded("no read path".to_string());
        let d = derive_local_dispatch(&degraded);
        assert_eq!(d.state, DispatchState::Unavailable);
        assert!(d.detail.contains("no read path"));
    }

    // -- the register ---------------------------------------------------------

    const HOME: &str = "# substrate-checks.phext — the substrate budget-check register\n\nOne scroll per recorded token-budget check. This Home scroll is documentation, not a check — a check scroll is the one carrying the two field lines the panel keys on (the substrate slug line and the unix timestamp line).\n";

    fn check_scroll(slug: &str, unix: u64) -> String {
        format!(
            "# Budget check — Claude\n\nSubstrate: {}\nChecked at (unix): {}\nChecked at: 2026-11-21 21:03 CST\nRecorded by: emise — steering pass\n",
            slug, unix
        )
    }

    #[test]
    fn register_parses_checks_and_skips_home_and_malformed() {
        let dir = temp_dir("register");
        let text = format!(
            "{}\u{17}{}\u{17}{}\u{17}{}",
            HOME,
            check_scroll("claude", 111),
            check_scroll("claude", 222),
            "# Budget check — Grok\n\nSubstrate: grok\n(no timestamp line)\n"
        );
        write(&dir, "substrate-checks.phext", text.as_bytes());
        let reg = read_checks(&dir.join("substrate-checks.phext")).expect("register");
        assert_eq!(reg.checks.len(), 2);
        assert_eq!(reg.sha256, crate::data::sha256_hex(text.as_bytes()));
        assert_eq!(latest_check(&reg.checks, "claude", None), Some(222));
        assert_eq!(latest_check(&reg.checks, "claude", Some(200)), Some(222));
        assert_eq!(latest_check(&reg.checks, "claude", Some(300)), None);
        assert_eq!(latest_check(&reg.checks, "grok", None), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_register_is_an_error_that_names_the_path() {
        let dir = std::env::temp_dir().join("phextron-substrates-test-definitely-absent.phext");
        let err = read_checks(&dir).expect_err("must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
        assert!(err.contains("phextron-substrates-test-definitely-absent"));
    }

    #[test]
    fn batchq_counts_come_from_the_snapshot_shape() {
        let v: Value = serde_json::from_str(
            r#"{"captured_at":"2026-11-21T20:00:00Z","counts":{"pending":1,"running":2,"done":3,"failed":4,"outputs":5}}"#,
        )
        .expect("json");
        assert_eq!(
            counts_from_json(&v),
            Some(BatchqCounts { pending: 1, running: 2, done: 3, failed: 4 })
        );
        let bad: Value = serde_json::from_str(r#"{"counts":{"pending":1}}"#).expect("json");
        assert_eq!(counts_from_json(&bad), None);
    }
}
