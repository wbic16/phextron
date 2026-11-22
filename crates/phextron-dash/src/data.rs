//! Data panel — Phextron's visibility into everywhere Will has stashed
//! data (the panel half of task 25.7.9/8.7.10/7.7.10).
//!
//! The live source is the stash census register itself: `stash.phext`,
//! one scroll per stash, built on the Muse VM from `stash-source.json`
//! by `build_stash_phext.py`. The register IS the source — this panel
//! parses it with libphext (`explode`, the reference parser), never a
//! scrape of docs/RECON.md or of any rendered page. The Home scroll
//! carries the builder-computed totals (measured bytes over counted
//! entries only, stash count, gap count, by-kind rollup); the panel
//! renders those totals as written and never recomputes them
//! differently.
//!
//! Row semantics, straight from the register:
//! - `counted = yes` entries are the estate: one row each, in register
//!   (coordinate) order.
//! - `counted = no` entries WITH a measured size are drill-down detail
//!   inside a counted parent (their notes open with "DETAIL of
//!   <parent>."). They render nested under that parent and are never
//!   added into any total.
//! - Entries whose Size line reads `unmeasured` are gaps: stashes the
//!   census named but could not measure (Great White, the offline
//!   nodes, Neon's container corpus, Google Drive, …). A gap is a row
//!   with its reason on its face — never a zero, never an absence.
//!
//! Verification: the panel computes the SHA-256 of the register bytes
//! it read (a small std-only implementation below — the dependency
//! lineage stays libphext + serde_json) and puts it on its face, so
//! the read can be checked against the source register's known hash
//! (census build: 4dcd956e20c41fb13b42aab63b5b49f2829b791e44540c7c1
//! 593b54f001ef206).
//!
//! Per-entry re-probe: where an entry's host is THIS host, a stat is
//! cheap and safe, so the panel refreshes what a stat can honestly
//! refresh — a file's size and mtime, a directory's mtime (a directory
//! stat carries no recursive size; the register's du figure stands).
//! "This host" is never guessed: paths are host-relative (`/usr/share/
//! ollama` exists on half the fleet), so re-probing happens only for
//! entries whose Host names one of the hosts declared in
//! PHEXTRON_DATA_LOCAL_HOSTS. Everywhere else the row shows the
//! register value with its census stamp. A failed probe falls back to
//! the register value and says so on the row — it never blanks it.
//!
//! Locations come from the environment, so the console reads the
//! register where it lives:
//!   PHEXTRON_DATA_STASH_PHEXT  register file (default
//!     /home/wbic16/phextron-data/phext-vm/stash.phext — the synced
//!     snapshot of ~/workspace/phext, the same corpus directory the
//!     phexts and video panels read; on the VM itself, point it at
//!     ~/workspace/phext/stash.phext)
//!   PHEXTRON_DATA_LOCAL_HOSTS  comma-separated register Host names
//!     this console host answers for (e.g. "aletheia-core" when the
//!     console runs on the hub). Unset: no re-probing anywhere.

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::{json, Value};
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

/// SHA-256 (FIPS 180-4), std-only. The panel hashes the register bytes
/// it read so the read is verifiable against the source register's
/// known hash without growing the dependency lineage.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{:08x}", x)).collect()
}

#[derive(Clone, Debug, PartialEq)]
struct StashEntry {
    coordinate: String,
    name: String,
    kind: String,
    host: String,
    path: String,
    /// `None` iff the register's Size line reads `unmeasured` (a gap).
    size_bytes: Option<u64>,
    size_note: String,
    counted: bool,
    items: Option<String>,
    freshness: String,
    read_method: String,
    coverage: String,
    notes: String,
}

impl StashEntry {
    fn is_gap(&self) -> bool {
        self.size_bytes.is_none()
    }
    /// Detail = measured but not counted: drill-down inside a parent.
    fn is_detail(&self) -> bool {
        !self.counted && self.size_bytes.is_some()
    }
}

/// Parse the Size line the builder renders: `<n> bytes (<human>)` with
/// an optional ` — <note>`, or `unmeasured` with an optional
/// ` — <reason>`. Returns (size, note).
fn parse_size_line(v: &str) -> (Option<u64>, String) {
    if let Some(rest) = v.strip_prefix("unmeasured") {
        let note = rest.strip_prefix(" — ").unwrap_or("").trim().to_string();
        return (None, note);
    }
    let bytes = v.split(' ').next().and_then(|t| t.parse::<u64>().ok());
    let note = v
        .split_once(" — ")
        .map(|(_, n)| n.trim().to_string())
        .unwrap_or_default();
    (bytes, note)
}

/// Parse one stash scroll. An entry scroll carries a `# name` heading
/// and the field lines the builder renders (Coordinate / Kind / Host /
/// Path / Size / Counted in estate total / Items / Freshness / Read
/// method / Coverage) followed by free notes. The generated Home
/// scroll has the heading and prose but no Kind/Counted field lines,
/// so it never parses as an entry — that is what keeps it out of the
/// rows.
fn parse_entry(text: &str) -> Option<StashEntry> {
    let mut name = None;
    let mut fields: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut notes_lines: Vec<&str> = Vec::new();
    const KEYS: [&str; 10] = [
        "Coordinate",
        "Kind",
        "Host",
        "Path",
        "Size",
        "Counted in estate total",
        "Items",
        "Freshness",
        "Read method",
        "Coverage",
    ];
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("# ") {
            if name.is_none() {
                name = Some(h.trim().to_string());
            }
            continue;
        }
        let mut is_field = false;
        for key in KEYS {
            if let Some(v) = line.strip_prefix(&format!("{}: ", key)) {
                fields.insert(key, v.trim());
                is_field = true;
                break;
            }
        }
        if !is_field {
            notes_lines.push(line);
        }
    }
    let name = name?;
    // Kind and Counted are the entry's signature; Home prose has neither.
    let kind = fields.get("Kind")?.to_string();
    let counted_raw = fields.get("Counted in estate total")?;
    let (size_bytes, size_note) = parse_size_line(fields.get("Size").copied().unwrap_or(""));
    Some(StashEntry {
        coordinate: fields.get("Coordinate").copied().unwrap_or("").to_string(),
        name,
        kind,
        host: fields.get("Host").copied().unwrap_or("").to_string(),
        path: fields.get("Path").copied().unwrap_or("").to_string(),
        size_bytes,
        size_note,
        counted: *counted_raw == "yes",
        items: fields.get("Items").map(|s| s.to_string()),
        freshness: fields.get("Freshness").copied().unwrap_or("").to_string(),
        read_method: fields.get("Read method").copied().unwrap_or("").to_string(),
        coverage: fields.get("Coverage").copied().unwrap_or("").to_string(),
        notes: notes_lines.join("\n").trim().to_string(),
    })
}

#[derive(Clone, Debug, PartialEq)]
struct HomeTotals {
    census_date: String,
    stashes_recorded: u64,
    total_bytes: u64,
    /// The builder's own human rendering of the total, carried verbatim
    /// so the panel never shows a differently-rounded figure.
    total_human: String,
    unmeasured: u64,
    by_kind: Vec<(String, u64, String)>, // kind, count, counted-bytes human (as written)
}

/// Parse the Home scroll's builder-computed totals. The header figures
/// come from here and nowhere else — the panel renders them, it does
/// not recompute them from the rows.
fn parse_home(text: &str) -> Option<HomeTotals> {
    let mut census_date = None;
    let mut stashes_recorded = None;
    let mut total = None;
    let mut unmeasured = None;
    let mut by_kind: Vec<(String, u64, String)> = Vec::new();
    let mut in_by_kind = false;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Census date: ") {
            census_date = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Stashes recorded: ") {
            stashes_recorded = v.trim().parse::<u64>().ok();
        } else if let Some(v) = line.strip_prefix("Total measured (counted entries only): ") {
            let bytes = v.split(' ').next().and_then(|t| t.parse::<u64>().ok());
            let human = v
                .split_once('(')
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .map(|s| s.to_string());
            total = match (bytes, human) {
                (Some(b), Some(h)) => Some((b, h)),
                _ => None,
            };
        } else if let Some(v) = line.strip_prefix("Unmeasured stashes (gaps, named honestly): ") {
            unmeasured = v.trim().parse::<u64>().ok();
        } else if line.starts_with("By kind (") {
            in_by_kind = true;
        } else if in_by_kind {
            if let Some(item) = line.strip_prefix("- ") {
                // "- <kind>: <count> — <counted bytes human>"
                if let Some((kind, rest)) = item.split_once(": ") {
                    if let Some((count, human)) = rest.split_once(" — ") {
                        if let Ok(n) = count.trim().parse::<u64>() {
                            by_kind.push((kind.to_string(), n, human.trim().to_string()));
                        }
                    }
                }
            } else if line.trim().is_empty() {
                in_by_kind = false;
            }
        }
    }
    let (total_bytes, total_human) = total?;
    Some(HomeTotals {
        census_date: census_date?,
        stashes_recorded: stashes_recorded?,
        total_bytes,
        total_human,
        unmeasured: unmeasured?,
        by_kind,
    })
}

#[derive(Debug)]
struct Register {
    home: HomeTotals,
    entries: Vec<StashEntry>,
    sha256: String,
}

/// Read the stash register: explode with libphext, sort entries by the
/// coordinate each scroll carries, and require the Home scroll — its
/// totals are builder-computed there and are never recomputed here, so
/// a register without a parseable Home degrades rather than letting
/// the panel invent a header.
fn read_register(path: &Path) -> Result<Register, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "stash register {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let sha256 = sha256_hex(&raw);
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "stash register {} is not valid UTF-8 — libphext parses text phexts; no rows are guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let mut home = None;
    let mut entries: Vec<StashEntry> = Vec::new();
    for s in scrolls.values() {
        if home.is_none() {
            if let Some(h) = parse_home(s) {
                home = Some(h);
                continue;
            }
        }
        if let Some(e) = parse_entry(s) {
            entries.push(e);
        }
    }
    let home = home.ok_or_else(|| {
        format!(
            "stash register {} has no parseable Home scroll — the estate totals are builder-computed there and are never recomputed from the rows",
            path.display()
        )
    })?;
    entries.sort_by_key(|e| coord_key(&e.coordinate));
    Ok(Register {
        home,
        entries,
        sha256,
    })
}

/// A coordinate string as a sortable numeric key ("1.1.1/1.1.1/1.1.10"
/// sorts after "1.1.1/1.1.1/1.1.9", which a string sort would invert).
fn coord_key(coord: &str) -> Vec<u64> {
    coord
        .split(|c| c == '/' || c == '.')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Resolve a detail entry's counted parent. Primary: the notes open
/// with "DETAIL of <label>." — the label is the parent's short name
/// ("Hub home", "GitHub account") and the parent's full name is the
/// label or the label plus a parenthetical ("Hub home (wbic16)").
/// Fallback: the longest counted entry on the same host whose path is
/// a proper prefix of the detail's path. Unresolvable details keep
/// their row at top level — grouped placement is a rendering, never a
/// reason to drop a stash.
fn parent_name(e: &StashEntry, entries: &[StashEntry]) -> Option<String> {
    if let Some(rest) = e.notes.strip_prefix("DETAIL of ") {
        if let Some(label) = rest.split('.').next() {
            let label = label.trim();
            if !label.is_empty() {
                for c in entries {
                    if c.counted && (c.name == label || c.name.starts_with(&format!("{} (", label)))
                    {
                        return Some(c.name.clone());
                    }
                }
            }
        }
    }
    let mut best: Option<&StashEntry> = None;
    for c in entries {
        if c.counted && c.host == e.host && c.path.len() > 1 {
            let prefix = format!("{}/", c.path.trim_end_matches('/'));
            if e.path.starts_with(&prefix) && best.map_or(true, |b| c.path.len() > b.path.len()) {
                best = Some(c);
            }
        }
    }
    best.map(|c| c.name.clone())
}

/// Display order: top-level rows (counted entries and gaps) in
/// register order, each followed immediately by its detail children in
/// register order. Returns (entry index, parent name) pairs.
fn display_order(entries: &[StashEntry]) -> Vec<(usize, Option<String>)> {
    let mut children: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    let mut parent_of: Vec<Option<String>> = vec![None; entries.len()];
    for (i, e) in entries.iter().enumerate() {
        if e.is_detail() {
            if let Some(p) = parent_name(e, entries) {
                children.entry(p.clone()).or_default().push(i);
                parent_of[i] = Some(p);
            }
        }
    }
    let mut out: Vec<(usize, Option<String>)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if e.is_detail() && parent_of[i].is_some() {
            continue; // emitted under its parent
        }
        out.push((i, None));
        if let Some(kids) = children.get(&e.name) {
            for &k in kids {
                out.push((k, Some(e.name.clone())));
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Probe {
    /// No re-probe: the row carries the register value + census stamp.
    Census,
    /// A file stat: size and mtime are live.
    LiveFile { size: u64, mtime: u64 },
    /// A directory stat: mtime is live; size stays the register's.
    LiveDir { mtime: u64 },
    /// The probe was attempted and failed; the register value stands.
    Failed(String),
}

/// The per-entry re-probe, deliberately narrow: only entries whose
/// Host is declared local to this console host, only plain absolute
/// paths (multi-path entries like "a + b" and brace expansions are
/// register prose, not paths), and only a stat — no walks, no reads.
fn probe(e: &StashEntry, local_hosts: &[String]) -> Probe {
    if !local_hosts.iter().any(|h| h == &e.host) {
        return Probe::Census;
    }
    if !e.path.starts_with('/') || e.path.contains(" + ") || e.path.contains('{') {
        return Probe::Census;
    }
    match std::fs::metadata(&e.path) {
        Ok(md) => {
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if md.is_file() {
                Probe::LiveFile {
                    size: md.len(),
                    mtime,
                }
            } else {
                Probe::LiveDir { mtime }
            }
        }
        Err(err) => Probe::Failed(format!("stat {} failed: {}", e.path, err)),
    }
}

fn probe_json(p: &Probe, now: u64) -> Value {
    match p {
        Probe::Census => json!({
            "mode": "census", "size_bytes": Value::Null, "age": Value::Null, "note": Value::Null,
        }),
        Probe::LiveFile { size, mtime } => json!({
            "mode": "live-file", "size_bytes": size,
            "age": age_from_unix(*mtime, now), "note": Value::Null,
        }),
        Probe::LiveDir { mtime } => json!({
            "mode": "live-dir", "size_bytes": Value::Null,
            "age": age_from_unix(*mtime, now), "note": Value::Null,
        }),
        Probe::Failed(why) => json!({
            "mode": "failed", "size_bytes": Value::Null, "age": Value::Null, "note": why,
        }),
    }
}

pub struct DataPanel;

impl Panel for DataPanel {
    fn id(&self) -> &'static str {
        "data"
    }
    fn title(&self) -> &'static str {
        "Data"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let register_path = std::env::var("PHEXTRON_DATA_STASH_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/phext-vm/stash.phext".to_string());
        let local_hosts: Vec<String> = std::env::var("PHEXTRON_DATA_LOCAL_HOSTS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let file_age = std::fs::metadata(&register_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| age_from_unix(d.as_secs(), now))
            .unwrap_or_else(|| "unreadable".to_string());

        let source_name =
            "stash.phext — the estate stash census register (one scroll per stash), parsed with libphext"
                .to_string();
        let source_location = format!("register {}", register_path);

        match read_register(Path::new(&register_path)) {
            Ok(reg) => {
                let order = display_order(&reg.entries);
                let rows: Vec<Value> = order
                    .iter()
                    .map(|(i, parent)| {
                        let e = &reg.entries[*i];
                        let p = probe(e, &local_hosts);
                        json!({
                            "coordinate": e.coordinate,
                            "name": e.name,
                            "kind": e.kind,
                            "host": e.host,
                            "path": e.path,
                            "size_bytes": e.size_bytes,
                            "size_note": e.size_note,
                            "counted": e.counted,
                            "detail": e.is_detail(),
                            "parent": parent,
                            "gap": e.is_gap(),
                            "items": e.items,
                            "freshness": e.freshness,
                            "read_method": e.read_method,
                            "coverage": e.coverage,
                            "notes": e.notes,
                            "probe": probe_json(&p, now),
                        })
                    })
                    .collect();
                let counted_entries = reg.entries.iter().filter(|e| e.counted).count();
                let detail_entries = reg.entries.iter().filter(|e| e.is_detail()).count();
                let gap_entries = reg.entries.iter().filter(|e| e.is_gap()).count();
                PanelData {
                    id: self.id(),
                    title: self.title(),
                    state: PanelState::Live,
                    source: SourceInfo {
                        name: source_name,
                        location: source_location,
                        freshness: format!(
                            "register read at request time · census {} · snapshot file updated {} · sha256 {}…",
                            reg.home.census_date,
                            file_age,
                            &reg.sha256[..12]
                        ),
                    },
                    body: json!({
                        "register": {
                            "census_date": reg.home.census_date,
                            "stashes_recorded": reg.home.stashes_recorded,
                            "entries_parsed": reg.entries.len(),
                            "counted_entries": counted_entries,
                            "detail_entries": detail_entries,
                            "gap_entries": gap_entries,
                            "total_measured_bytes": reg.home.total_bytes,
                            "total_measured_human": reg.home.total_human,
                            "unmeasured": reg.home.unmeasured,
                            "by_kind": reg.home.by_kind.iter().map(|(k, n, b)| json!({
                                "kind": k, "count": n, "bytes_human": b,
                            })).collect::<Vec<Value>>(),
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
                    freshness: "register unavailable from here".to_string(),
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
            "phextron-data-test-{}-{}",
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

    const HOME: &str = "# stash.phext — the stash census\n\nWill's directive (2026-11-21): \"make sure phextron has\nvisibility into everywhere I've stashed data.\"\n\nCensus date: 2026-11-21\nTask: 25.7.9/8.7.10/7.7.10 (parent program: the Phextron dashboards, 21.4.10/5.5.8/6.4.6)\nStashes recorded: 3\nTotal measured (counted entries only): 436246630400 bytes (436.25 GB)\nUnmeasured stashes (gaps, named honestly): 1\n\nBy kind (count — counted bytes):\n- other: 2 — 0 B\n- working trees: 1 — 436.25 GB\n\nUnmeasured stashes:\n- Great White (great-white) — unmeasured - SSH key refused\n";

    const PARENT: &str = "# Hub home (wbic16)\n\nCoordinate: 1.1.1/1.1.1/1.1.3\nKind: working trees\nHost: aletheia-core\nPath: /home/wbic16\nSize: 436246630400 bytes (436.25 GB)\nCounted in estate total: yes\nFreshness: 2026-11-21 (disk census, nodes.json)\nRead method: filesystem sampler (tailnet SSH df/du)\nCoverage: nodes panel (node only - data roots new)\n\nFull-home du from the 2026-11-21 disk census.\n";

    const DETAIL: &str = "# Hub chromium-census\n\nCoordinate: 1.1.1/1.1.1/1.1.2\nKind: other\nHost: aletheia-core\nPath: /home/wbic16/chromium-census\nSize: 239775534979 bytes (239.78 GB)\nCounted in estate total: no\nFreshness: 2026-11-21 19:30 CST (census probe)\nRead method: filesystem sampler (tailnet SSH df/du)\nCoverage: new\n\nDETAIL of Hub home. The largest single stash on the hub.\n";

    const GAP: &str = "# Great White\n\nCoordinate: 1.1.1/1.1.1/1.1.4\nKind: other\nHost: great-white\nPath: (whole node)\nSize: unmeasured — unmeasured - SSH key refused\nCounted in estate total: no\nFreshness: 2026-11-21 19:30 CST (census probe)\nRead method: filesystem sampler (tailnet SSH df/du)\nCoverage: new\n\nThe node refuses this key.\n";

    #[test]
    fn sha256_matches_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 56-byte vector: exercises the two-block padding path.
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn counted_entry_parses_all_fields() {
        let e = parse_entry(PARENT).expect("entry");
        assert_eq!(e.name, "Hub home (wbic16)");
        assert_eq!(e.kind, "working trees");
        assert_eq!(e.host, "aletheia-core");
        assert_eq!(e.size_bytes, Some(436246630400));
        assert!(e.counted);
        assert!(!e.is_gap());
        assert!(!e.is_detail());
        assert_eq!(e.items, None);
    }

    #[test]
    fn zero_size_is_measured_not_a_gap() {
        // The hub's SQ instances measure exactly 0 bytes — a measured
        // zero must not be misread as an unmeasured gap.
        let text = "# Hub SQ instances\n\nCoordinate: 1.1.1/1.1.1/1.1.5\nKind: phext corpus\nHost: aletheia-core\nPath: sq host on ports 1337 and 18745\nSize: 0 bytes (0 B)\nCounted in estate total: yes\nItems: 2 instances, 1 scroll each\nFreshness: 2026-11-21 19:30 CST (census probe)\nRead method: SQ API (/api/v2/toc)\nCoverage: new\n";
        let e = parse_entry(text).expect("entry");
        assert_eq!(e.size_bytes, Some(0));
        assert!(!e.is_gap());
        assert_eq!(e.items.as_deref(), Some("2 instances, 1 scroll each"));
    }

    #[test]
    fn gap_entry_parses_with_its_reason() {
        let e = parse_entry(GAP).expect("entry");
        assert_eq!(e.size_bytes, None);
        assert!(e.is_gap());
        assert!(!e.is_detail());
        assert_eq!(e.size_note, "unmeasured - SSH key refused");
    }

    #[test]
    fn home_scroll_is_not_an_entry_and_its_totals_parse() {
        assert!(parse_entry(HOME).is_none());
        let h = parse_home(HOME).expect("home");
        assert_eq!(h.census_date, "2026-11-21");
        assert_eq!(h.stashes_recorded, 3);
        assert_eq!(h.total_bytes, 436246630400);
        assert_eq!(h.total_human, "436.25 GB");
        assert_eq!(h.unmeasured, 1);
        assert_eq!(h.by_kind.len(), 2);
        assert_eq!(h.by_kind[1], ("working trees".to_string(), 1, "436.25 GB".to_string()));
    }

    #[test]
    fn register_groups_detail_under_parent_regardless_of_order() {
        let dir = temp_dir("register");
        // File order: home, detail (coord .2), parent (coord .3), gap
        // (coord .4) — the detail precedes its parent in the register
        // and must still render nested under it.
        let text = format!("{}\u{17}{}\u{17}{}\u{17}{}", HOME, DETAIL, PARENT, GAP);
        write(&dir, "stash.phext", text.as_bytes());
        let reg = read_register(&dir.join("stash.phext")).expect("register");
        assert_eq!(reg.entries.len(), 3);
        assert_eq!(reg.home.stashes_recorded, 3);
        assert_eq!(reg.sha256, sha256_hex(text.as_bytes()));
        let order = display_order(&reg.entries);
        let names: Vec<&str> = order.iter().map(|(i, _)| reg.entries[*i].name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Hub home (wbic16)", "Hub chromium-census", "Great White"]
        );
        assert_eq!(order[1].1.as_deref(), Some("Hub home (wbic16)"));
        assert_eq!(order[0].1, None);
        assert_eq!(order[2].1, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parent_falls_back_to_path_prefix() {
        // A detail whose notes lost the DETAIL label still nests under
        // the counted entry whose path contains it, same host only.
        let mut orphan = parse_entry(DETAIL).expect("detail");
        orphan.notes = "No label here.".to_string();
        let parent = parse_entry(PARENT).expect("parent");
        let entries = vec![parent, orphan];
        assert_eq!(
            parent_name(&entries[1], &entries).as_deref(),
            Some("Hub home (wbic16)")
        );
    }

    #[test]
    fn missing_register_is_an_error_that_names_the_path() {
        let dir = std::env::temp_dir().join("phextron-data-test-definitely-absent.phext");
        let err = read_register(&dir).expect_err("must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
        assert!(err.contains("phextron-data-test-definitely-absent"));
    }

    #[test]
    fn register_without_home_degrades_by_name() {
        let dir = temp_dir("nohome");
        write(&dir, "stash.phext", PARENT.as_bytes());
        let err = read_register(&dir.join("stash.phext")).expect_err("must fail");
        assert!(err.contains("no parseable Home scroll"), "names the gap: {}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_refreshes_only_declared_local_hosts() {
        let dir = temp_dir("probe");
        write(&dir, "blob.bin", b"12345");
        let mut e = parse_entry(PARENT).expect("entry");
        e.path = dir.join("blob.bin").to_string_lossy().to_string();
        // Host not declared local: census, even though the path exists.
        assert_eq!(probe(&e, &[]), Probe::Census);
        assert_eq!(probe(&e, &["some-other-host".to_string()]), Probe::Census);
        // Declared local + a file: live size.
        match probe(&e, &["aletheia-core".to_string()]) {
            Probe::LiveFile { size, .. } => assert_eq!(size, 5),
            other => panic!("expected LiveFile, got {:?}", other),
        }
        // Declared local + a directory: freshness only, no size claim.
        e.path = dir.to_string_lossy().to_string();
        match probe(&e, &["aletheia-core".to_string()]) {
            Probe::LiveDir { .. } => {}
            other => panic!("expected LiveDir, got {:?}", other),
        }
        // Declared local + a vanished path: failed probe, named.
        e.path = dir.join("gone").to_string_lossy().to_string();
        match probe(&e, &["aletheia-core".to_string()]) {
            Probe::Failed(why) => assert!(why.contains("gone"), "names the path: {}", why),
            other => panic!("expected Failed, got {:?}", other),
        }
        // Multi-path register prose is never stat'ed.
        e.path = "/home/wbic16/emise-v9 + emise-v10".to_string();
        assert_eq!(probe(&e, &["aletheia-core".to_string()]), Probe::Census);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn coordinate_keys_sort_numerically() {
        assert!(coord_key("1.1.1/1.1.1/1.1.9") < coord_key("1.1.1/1.1.1/1.1.10"));
        assert!(coord_key("1.1.1/1.1.1/1.1.2") < coord_key("1.1.1/1.1.1/1.1.10"));
    }
}
