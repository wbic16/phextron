//! All-phexts panel — the stage-3 live panel.
//!
//! A census over the two live phext corpora, read directly and parsed with
//! libphext (the reference parser) — never a rendered page, never a listing
//! copied from somewhere else:
//! - the register corpus from the Muse VM: `~/workspace/phext/*.phext`
//!   (incipit, emise, tasks, skills, state, accounting, costs, life, …),
//! - the edit.phext.io corpus on awsbox: the hosted files in `/source/human`
//!   that phext-edit serves.
//!
//! Every row is computed from the file itself at request time: bytes from
//! the filesystem, scroll count from libphext's span index
//! (`libphext::phext::spans` — the reference parser's borrowed walk, whose
//! documented contract is exactly `explode`'s: a scroll exists iff it
//! holds at least one byte, and each span's text is the scroll `explode`
//! returns at that coordinate), freshness from mtime. A phext
//! that does not parse (not valid UTF-8 text) keeps its row and says so —
//! it never disappears, and its scroll count is never guessed.
//!
//! The census is cached per corpus directory behind a stat-only
//! fingerprint (file name, length, mtime at nanosecond precision): a
//! request re-stats the directory and re-parses only when a file has
//! actually changed. That keeps the panel honest at request time without
//! re-walking the half-gigabyte ietf.phext on every tab fetch.
//!
//! Corpus locations come from the environment, so the console reads each
//! corpus where it lives:
//!   PHEXTRON_PHEXTS_VM_DIR     register corpus directory
//!                              (default /home/wbic16/phextron-data/phext-vm —
//!                              the snapshot the ranch node carries, synced
//!                              from the VM with mtimes preserved; on the VM
//!                              itself, point it at ~/workspace/phext)
//!   PHEXTRON_PHEXTS_AWSBOX_DIR edit.phext.io corpus directory — no default,
//!                              deliberately. It must be set on the host
//!                              that holds the files (on awsbox:
//!                              /source/human). A guessed default is a
//!                              lie waiting to happen: aletheia-core itself
//!                              carries a /source/human — a wbic16/human
//!                              checkout from 2026-03-15, 23 phexts,
//!                              untouched for 251 days — which a defaulted
//!                              path would have served up as this corpus.
//!
//! Why the awsbox corpus degrades from the ranch nodes today, named rather
//! than papered over: the hosted files live on awsbox's disk; phext-edit
//! binds 127.0.0.1:8080 on awsbox, and its only corpus-wide API
//! (/api/phexts) carries names and byte sizes but no scroll counts; the
//! public route is HTTPS-only (the port-80 vhost 301-redirects) and this
//! std-only reader speaks plain HTTP on the tailnet. So from a ranch node
//! the corpus is not configured and reports degraded with that reason on
//! its face; on awsbox, with PHEXTRON_PHEXTS_AWSBOX_DIR=/source/human,
//! the same reader goes live unchanged. The SQ
//! tenant hosts on awsbox (ports 1337/1338/1339/8931) are key-gated and
//! host other tenants' phexts, not this corpus — they are not a source
//! for it. (docs/RECON.md §3.)

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

/// Same age shape the nodes and Jenkins panels use.
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

struct CorpusSpec {
    id: &'static str,
    label: &'static str,
    host: &'static str,
    env_var: &'static str,
    /// The directory used when the env var is unset — `None` means the
    /// corpus is only ever read where its location is explicitly
    /// configured; it is never guessed.
    default_dir: Option<&'static str>,
    /// Extra truth appended to the error when this corpus cannot be read.
    degraded_hint: Option<&'static str>,
}

fn corpus_specs() -> Vec<CorpusSpec> {
    vec![
        CorpusSpec {
            id: "vm",
            label: "Register corpus",
            host: "Muse VM",
            env_var: "PHEXTRON_PHEXTS_VM_DIR",
            default_dir: Some("/home/wbic16/phextron-data/phext-vm"),
            degraded_hint: None,
        },
        CorpusSpec {
            id: "awsbox",
            label: "edit.phext.io corpus",
            host: "awsbox",
            env_var: "PHEXTRON_PHEXTS_AWSBOX_DIR",
            default_dir: None,
            degraded_hint: Some(
                "the hosted files live on awsbox's disk: phext-edit binds 127.0.0.1:8080 there, \
                 its corpus-wide /api/phexts carries names and byte sizes but no scroll counts, \
                 and the public route is HTTPS-only — this reader goes live where the files \
                 themselves are readable (on awsbox), and shows no rows rather than stale ones here",
            ),
        },
    ]
}

#[derive(Clone, Debug)]
struct PhextRow {
    name: String,
    file: String,
    bytes: u64,
    scrolls: Option<usize>,
    parse_error: Option<String>,
    modified_unix: u64,
}

#[derive(Clone, Debug)]
struct CorpusCensus {
    rows: Vec<PhextRow>,
    total_bytes: u64,
    total_scrolls: u64,
    parse_failures: usize,
    newest_modified: Option<u64>,
}

/// Census one corpus directory. An unreadable directory is an `Err` (the
/// corpus degrades); a readable one always censuses, even if some phexts
/// inside it fail to parse — those keep their rows with the failure named.
fn census_dir(dir: &Path) -> Result<CorpusCensus, String> {
    let rd = std::fs::read_dir(dir)
        .map_err(|e| format!("corpus directory {} is not readable from here: {}", dir.display(), e))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| format!("corpus directory {} could not be listed: {}", dir.display(), e))?;
        let path = ent.path();
        let is_file = ent.file_type().map(|t| t.is_file()).unwrap_or(false);
        if is_file && path.extension().map_or(false, |x| x == "phext") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut census = CorpusCensus {
        rows: Vec::new(),
        total_bytes: 0,
        total_scrolls: 0,
        parse_failures: 0,
        newest_modified: None,
    };
    for path in paths {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file.clone());
        let md = std::fs::metadata(&path).ok();
        let bytes = md.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified_unix = md
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (scrolls, parse_error) = match std::fs::read(&path) {
            Ok(raw) => match String::from_utf8(raw) {
                Ok(text) => (Some(libphext::phext::spans(&text).len()), None),
                Err(_) => (
                    None,
                    Some(
                        "not valid UTF-8 — libphext parses text phexts; scroll count not guessed"
                            .to_string(),
                    ),
                ),
            },
            Err(e) => (None, Some(format!("unreadable: {}", e))),
        };
        census.total_bytes += bytes;
        match scrolls {
            Some(n) => census.total_scrolls += n as u64,
            None => census.parse_failures += 1,
        }
        if modified_unix > 0 {
            census.newest_modified = Some(match census.newest_modified {
                Some(cur) => cur.max(modified_unix),
                None => modified_unix,
            });
        }
        census.rows.push(PhextRow {
            name,
            file,
            bytes,
            scrolls,
            parse_error,
            modified_unix,
        });
    }
    Ok(census)
}

/// Stat-only fingerprint of a corpus directory: every .phext file's name,
/// length, and mtime (nanosecond precision). Any content change moves at
/// least one of the three, so a matching fingerprint means the cached
/// census is still the directory's truth.
fn fingerprint(dir: &Path) -> Result<String, String> {
    let rd = std::fs::read_dir(dir)
        .map_err(|e| format!("corpus directory {} is not readable from here: {}", dir.display(), e))?;
    let mut parts: Vec<String> = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| format!("corpus directory {} could not be listed: {}", dir.display(), e))?;
        let path = ent.path();
        let is_file = ent.file_type().map(|t| t.is_file()).unwrap_or(false);
        if is_file && path.extension().map_or(false, |x| x == "phext") {
            let md = ent.metadata().map_err(|e| format!("could not stat {}: {}", path.display(), e))?;
            let (secs, nanos) = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| (d.as_secs(), d.subsec_nanos()))
                .unwrap_or((0, 0));
            parts.push(format!(
                "{}:{}:{}.{}",
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                md.len(),
                secs,
                nanos
            ));
        }
    }
    parts.sort();
    Ok(parts.join("\n"))
}

static CENSUS_CACHE: std::sync::Mutex<
    std::collections::BTreeMap<String, (String, CorpusCensus)>,
> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The census the panel serves: re-stat the directory on every call and
/// re-parse only when the fingerprint moved. Keyed per directory — the
/// two corpora alternate through here on every shell load, and a
/// single-slot cache would thrash between them.
fn census_cached(dir: &Path) -> Result<CorpusCensus, String> {
    let fp = fingerprint(dir)?;
    let key = dir.to_string_lossy().to_string();
    if let Ok(guard) = CENSUS_CACHE.lock() {
        if let Some((f, c)) = guard.get(&key) {
            if *f == fp {
                return Ok(c.clone());
            }
        }
    }
    let census = census_dir(dir)?;
    if let Ok(mut guard) = CENSUS_CACHE.lock() {
        guard.insert(key, (fp, census.clone()));
    }
    Ok(census)
}

pub struct PhextsPanel;

impl Panel for PhextsPanel {
    fn id(&self) -> &'static str {
        "phexts"
    }
    fn title(&self) -> &'static str {
        "All Phexts"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let mut corpora: Vec<Value> = Vec::new();
        let mut rows: Vec<Value> = Vec::new();
        let mut live = 0usize;
        let mut errors: Vec<String> = Vec::new();
        let (mut t_phexts, mut t_bytes, mut t_scrolls, mut t_failures) = (0u64, 0u64, 0u64, 0usize);
        let mut freshness_parts: Vec<String> = Vec::new();
        let mut locations: Vec<String> = Vec::new();

        for spec in corpus_specs() {
            let env_dir = std::env::var(spec.env_var)
                .ok()
                .filter(|d| !d.trim().is_empty());
            let configured = env_dir.is_some() || spec.default_dir.is_some();
            let dir: String = env_dir
                .clone()
                .or_else(|| spec.default_dir.map(|d| d.to_string()))
                .unwrap_or_else(|| format!("(not configured — {} unset)", spec.env_var));
            locations.push(format!("{}: {}", spec.label, dir));
            // A corpus with no default is read only where its location is
            // explicitly configured — never guessed (see the module docs
            // on the stale namesake /source/human).
            let result = if configured {
                census_cached(Path::new(&dir))
            } else {
                Err("corpus location not configured on this host — set PHEXTRON_PHEXTS_AWSBOX_DIR to the hosted files' directory on the host that holds them (on awsbox: /source/human); no default path is guessed".to_string())
            };
            match result {
                Ok(c) => {
                    live += 1;
                    t_phexts += c.rows.len() as u64;
                    t_bytes += c.total_bytes;
                    t_scrolls += c.total_scrolls;
                    t_failures += c.parse_failures;
                    let newest_age = c
                        .newest_modified
                        .map(|ts| age_from_unix(ts, now))
                        .unwrap_or_else(|| "no files".to_string());
                    freshness_parts.push(format!("{} newest file {}", spec.label, newest_age));
                    corpora.push(json!({
                        "id": spec.id,
                        "label": spec.label,
                        "host": spec.host,
                        "location": dir,
                        "state": "live",
                        "phexts": c.rows.len(),
                        "bytes": c.total_bytes,
                        "scrolls": c.total_scrolls,
                        "parse_failures": c.parse_failures,
                        "newest_age": newest_age,
                        "error": Value::Null,
                    }));
                    for r in c.rows {
                        rows.push(json!({
                            "name": r.name,
                            "file": r.file,
                            "corpus": spec.id,
                            "corpus_label": spec.label,
                            "bytes": r.bytes,
                            "scrolls": r.scrolls,
                            "parse_error": r.parse_error,
                            "modified_unix": r.modified_unix,
                            "age": if r.modified_unix > 0 { age_from_unix(r.modified_unix, now) } else { "unknown".to_string() },
                        }));
                    }
                }
                Err(e) => {
                    let why = match spec.degraded_hint {
                        Some(hint) => format!("{} — {}", e, hint),
                        None => e,
                    };
                    errors.push(format!("{}: {}", spec.label, why));
                    freshness_parts.push(format!("{} unavailable from here", spec.label));
                    corpora.push(json!({
                        "id": spec.id,
                        "label": spec.label,
                        "host": spec.host,
                        "location": dir,
                        "state": "degraded",
                        "phexts": Value::Null,
                        "bytes": Value::Null,
                        "scrolls": Value::Null,
                        "parse_failures": Value::Null,
                        "newest_age": Value::Null,
                        "error": why,
                    }));
                }
            }
        }

        let source = SourceInfo {
            name: "the phext corpora themselves, parsed with libphext (span index) — bytes, scroll counts, and mtimes read from the files at request time".into(),
            location: locations.join(" · "),
            freshness: format!(
                "census read at request time · {}",
                freshness_parts.join(" · ")
            ),
        };

        if live == 0 {
            return PanelData {
                id: self.id(),
                title: self.title(),
                state: PanelState::Degraded,
                source,
                body: json!({
                    "error": errors.join(" · "),
                    "corpora": corpora,
                }),
            };
        }

        PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Live,
            source,
            body: json!({
                "totals": {
                    "phexts": t_phexts,
                    "bytes": t_bytes,
                    "scrolls": t_scrolls,
                    "parse_failures": t_failures,
                    "corpora_live": live,
                    "corpora_total": corpora.len(),
                },
                "corpora": corpora,
                "rows": rows,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "phextron-phexts-test-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        let mut f = std::fs::File::create(dir.join(name)).expect("create file");
        f.write_all(bytes).expect("write file");
    }

    #[test]
    fn census_counts_scrolls_bytes_and_keeps_parse_failures() {
        let dir = temp_dir("census");
        // \u{17} is the phext scroll break: three scrolls, one scroll, and
        // a phext that is not text at all. A non-phext file and a directory
        // wearing a .phext name are not part of the corpus.
        write(&dir, "alpha.phext", "one\u{17}two\u{17}three".as_bytes());
        write(&dir, "beta.phext", b"single scroll");
        write(&dir, "gamma.phext", &[0xff, 0xfe, 0x00, 0x01]);
        write(&dir, "notes.txt", b"not a phext");
        std::fs::create_dir_all(dir.join("delta.phext")).expect("dir named .phext");

        let c = census_dir(&dir).expect("census");
        assert_eq!(c.rows.len(), 3);
        assert_eq!(c.parse_failures, 1);
        assert_eq!(c.total_scrolls, 4);
        assert_eq!(
            c.total_bytes,
            ("one\u{17}two\u{17}three".len() + "single scroll".len() + 4) as u64
        );
        let by_name = |n: &str| c.rows.iter().find(|r| r.name == n).expect("row");
        assert_eq!(by_name("alpha").scrolls, Some(3));
        assert_eq!(by_name("beta").scrolls, Some(1));
        let gamma = by_name("gamma");
        assert_eq!(gamma.scrolls, None);
        assert!(gamma.parse_error.as_ref().unwrap().contains("UTF-8"));
        assert!(gamma.modified_unix > 0);
        // Rows are in file-name order within the corpus.
        assert_eq!(c.rows[0].name, "alpha");
        assert_eq!(c.rows[2].name, "gamma");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_scrolls_do_not_materialize() {
        // libphext existence semantics: delimiter runs addressing empty
        // scrolls are addressing, not content — explode counts only
        // scrolls holding at least one byte.
        let dir = temp_dir("empty");
        write(&dir, "sparse.phext", "a\u{17}\u{17}b".as_bytes());
        let c = census_dir(&dir).expect("census");
        assert_eq!(c.rows[0].scrolls, Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn readable_empty_dir_is_an_empty_census_not_an_error() {
        let dir = temp_dir("emptydir");
        let c = census_dir(&dir).expect("census");
        assert_eq!(c.rows.len(), 0);
        assert_eq!(c.total_bytes, 0);
        assert_eq!(c.newest_modified, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_dir_is_an_error_that_names_the_path() {
        let dir = std::env::temp_dir().join("phextron-phexts-test-definitely-absent");
        let err = census_dir(&dir).expect_err("must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
        assert!(err.contains("phextron-phexts-test-definitely-absent"));
    }

    #[test]
    fn span_count_matches_explode_on_a_mixed_document() {
        // The census counts with `spans`; pin its documented equivalence
        // with `explode` (the nodes panel's parser) so a libphext upgrade
        // that broke the contract would fail here, not silently drift.
        let text = "a\u{17}\u{17}b\u{18}c\u{19}\u{1A}d";
        assert_eq!(
            libphext::phext::spans(text).len(),
            libphext::phext::explode(text).len()
        );
        assert_eq!(libphext::phext::spans(text).len(), 4);
    }

    #[test]
    fn cached_census_refreshes_when_a_file_changes() {
        let dir = temp_dir("cache");
        write(&dir, "alpha.phext", b"one");
        let first = census_cached(&dir).expect("first census");
        assert_eq!(first.rows[0].scrolls, Some(1));
        // Same path, new content (different length): the fingerprint must
        // move and the served census must follow.
        write(&dir, "alpha.phext", "one\u{17}two".as_bytes());
        let second = census_cached(&dir).expect("second census");
        assert_eq!(second.rows[0].scrolls, Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_is_keyed_per_corpus_directory() {
        let a = temp_dir("cache-a");
        let b = temp_dir("cache-b");
        write(&a, "alpha.phext", b"one");
        write(&b, "beta.phext", "x\u{17}y".as_bytes());
        // The two corpora alternate through the cache on every shell load;
        // each directory must keep its own census.
        assert_eq!(census_cached(&a).expect("a1").rows[0].name, "alpha");
        assert_eq!(census_cached(&b).expect("b1").rows[0].name, "beta");
        assert_eq!(census_cached(&a).expect("a2").rows[0].scrolls, Some(1));
        assert_eq!(census_cached(&b).expect("b2").rows[0].scrolls, Some(2));
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn ages_humanize_like_the_other_panels() {
        assert_eq!(age_from_unix(0, 45), "45s ago");
        assert_eq!(age_from_unix(0, 3_900), "65m ago");
        assert_eq!(age_from_unix(0, 5_700), "1h 35m ago");
        assert_eq!(age_from_unix(0, 200_000), "2d ago");
        assert_eq!(age_from_unix(100, 50), "0s ago");
    }
}
