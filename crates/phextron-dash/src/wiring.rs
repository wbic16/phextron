//! Wiring panel — the fleet topology from wiring.phext, clickable to
//! SSH (the panel half of task 24.5.6/6.5.9/5.5.12, W4 plan §1.11).
//!
//! The live source is the wiring declaration of record itself:
//! `wiring.phext`, Will's auditable declaration of the ranch network
//! (home of record: wbic16/human@exo). The register IS the source —
//! this panel parses it with libphext (`explode`, the reference
//! parser) and renders only what its machine-checked lines declare:
//! `node:`, `usb4:`, `xlink:`, `uplink:`, `proxy:`, `metric:`, and
//! `divergence:` lines, in the forms the register's own Conventions
//! scroll (1.1.1/1.1.1/1.1.2) specifies. That scroll states the
//! forms with `<placeholder>` tokens; a line carrying a placeholder
//! is a form, not a declaration, and is never parsed as one (the
//! live register's first panel read caught exactly those phantoms —
//! the regression test below keeps them out). Prose is never mined
//! for facts, and nothing is inferred from hostnames, the initials
//! map, or fleet knowledge outside the register.
//!
//! Node uplink classification, strictly from those lines:
//! - A node named by an `uplink:` line holds its row's 2.5G Ethernet
//!   uplink. The line's interface may itself read `unknown` — the
//!   uplink is declared, the interface name is unstated, and the row
//!   says which of the two it is.
//! - A node named by no `uplink:` line but standing as an endpoint
//!   of a declared `usb4:`/`xlink:` link rides USB4.
//! - Anything else is UNSTATED and renders as unstated.
//! - WiFi is assigned to NO node: the register declares no per-node
//!   WiFi uplink anywhere. WiFi exists in the register only as route
//!   policy (`metric: wifi 600`, the back-haul of last resort), so
//!   the legend carries the WiFi class with a count of zero and the
//!   panel says why on its face. The day the register declares a
//!   node's WiFi uplink in a machine-checked line, this panel will
//!   show it — not before.
//!
//! Divergences: the register's known-divergence scroll
//! (1.1.1/1.1.1/4.1.1) keys each entry (`node:DW`, `usb4:KO-MB`,
//! `uplink:NK`, …). An entry attaches to the element its key names
//! exactly; wildcard and policy keys (`uplink:*`, `proxy:*`,
//! `policy:lilly`) attach to nothing and render only in the panel's
//! divergence list. A declared element the register itself flags is
//! never quietly drawn as healthy.
//!
//! Double-click → SSH: §1.9's cluster session machinery (a
//! Windows-capable tmux with an in-console attach) is not built yet
//! — there is no session registry in this codebase — so the
//! double-click does the honest second thing the spec allows: it
//! yields the exact SSH command for the node, built from the
//! register's own tailnet address by the fleet's standing recipe
//! (TOOLS.md: the VM's Ed25519 key, IdentitiesOnly, via the tailnet
//! proxy), with the fleet-ssh.sh form alongside it, and the view
//! states plainly that it showed a command rather than attaching.
//! A node whose register line records `ssh -` has no address to
//! build a command from; its double-click says so, with its
//! declared state. No fake attach, no silent no-op.
//!
//! Verification: the panel computes the SHA-256 of the register
//! bytes it read (the std-only implementation shared from the Data
//! panel) and puts it on its face, so the read can be checked
//! against the source register's known hash.
//!
//! Locations come from the environment, so the console reads the
//! register where it lives:
//!   PHEXTRON_WIRING_PHEXT  register file (default
//!     /home/wbic16/phextron-data/phext-vm/wiring.phext — the synced
//!     snapshot of ~/workspace/phext, the same corpus directory the
//!     data, phexts and video panels read; on the VM itself, point
//!     it at ~/workspace/phext/wiring.phext)

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum UplinkClass {
    Ethernet,
    Usb4,
    /// Part of the legend's vocabulary (Will's direction names three
    /// uplink kinds), but assigned to no node today: the register
    /// declares no per-node WiFi uplink. Kept as a class so the day
    /// a machine-checked line declares one, the derivation has a
    /// place to put it.
    #[allow(dead_code)]
    Wifi,
    Unstated,
}

impl UplinkClass {
    fn as_str(&self) -> &'static str {
        match self {
            UplinkClass::Ethernet => "ethernet",
            UplinkClass::Usb4 => "usb4",
            UplinkClass::Wifi => "wifi",
            UplinkClass::Unstated => "unstated",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct WireNode {
    initials: String,
    /// The hostname exactly as the register writes it; the register
    /// writes `unknown` for nodes it cannot name, and that is a
    /// fact about the register, rendered as written.
    hostname: String,
    row: u32,
    col: u32,
    ram_gb: u32,
    state: String,
    ssh_ip: Option<String>,
    uplink: UplinkClass,
    /// The uplink interface from the node's `uplink:` line; `None`
    /// when the register leaves it `unknown` or the node holds no
    /// uplink line.
    uplink_iface: Option<String>,
    divergences: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct WireLink {
    /// "usb4" (in-row interlink) or "xlink" (cross-row/off-diagram).
    kind: String,
    a: String,
    b: String,
    subnet: String,
    a_ip: String,
    b_ip: String,
    expect_gbit: String,
    /// xlink lines carry a state token; usb4 lines do not.
    state: Option<String>,
    divergences: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct RowUplink {
    row: u32,
    side: String,
    node: String,
    iface: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Divergence {
    id: String,
    keys: Vec<String>,
    statement: String,
}

#[derive(Debug)]
struct Register {
    nodes: Vec<WireNode>,
    links: Vec<WireLink>,
    uplinks: Vec<RowUplink>,
    /// (row, host initials, port) per `proxy:` line.
    proxies: Vec<(u32, String, u32)>,
    metrics: Vec<(String, u32)>,
    divergences: Vec<Divergence>,
    sha256: String,
}

/// The register's Conventions scroll states each line form with
/// `<placeholder>` tokens (`node: <INITIALS> <hostname|unknown>
/// col <1-4> …`). Those are forms, not declarations: a token
/// carrying a placeholder bracket marks the whole line as a
/// template, and no parser below accepts it. (The register's own
/// checker reads the same file; a form drawn as a node would be a
/// phantom.) Free-text divergence statements are exempt — the guard
/// there covers only the id and key fields.
fn has_placeholder(tokens: &[&str]) -> bool {
    tokens.iter().any(|t| t.contains('<') || t.contains('>'))
}

/// Parse a `node:` line:
/// `node: <INITIALS> <hostname|unknown> col <1-4> ram <GB> state <state> ssh <tailnet-ip|->`
fn parse_node(line: &str) -> Option<(String, String, u32, u32, String, Option<String>)> {
    let t: Vec<&str> = line.split_whitespace().collect();
    if has_placeholder(&t) {
        return None;
    }
    if t.len() != 10 || t[2] != "col" || t[4] != "ram" || t[6] != "state" || t[8] != "ssh" {
        return None;
    }
    let col = t[3].parse::<u32>().ok()?;
    let ram = t[5].parse::<u32>().ok()?;
    let ssh = if t[9] == "-" {
        None
    } else {
        Some(t[9].to_string())
    };
    Some((
        t[0].to_string(),
        t[1].to_string(),
        col,
        ram,
        t[7].to_string(),
        ssh,
    ))
}

/// Parse a `usb4:` or `xlink:` line:
/// `<kind>: <A>-<B> subnet <cidr> <A> <ip> <B> <ip> expect-gbit <lo>-<hi> [state <s>]`
/// The endpoint tokens must agree with the pair token; a line whose
/// parts disagree is not a declaration this panel will draw.
fn parse_link(kind: &str, line: &str) -> Option<WireLink> {
    let t: Vec<&str> = line.split_whitespace().collect();
    if has_placeholder(&t) {
        return None;
    }
    if t.len() < 9 || t[1] != "subnet" || t[7] != "expect-gbit" {
        return None;
    }
    let (a, b) = t[0].split_once('-')?;
    if t[3] != a || t[5] != b {
        return None;
    }
    let state = if t.len() == 11 && t[9] == "state" {
        Some(t[10].to_string())
    } else if t.len() == 9 {
        None
    } else {
        return None;
    };
    Some(WireLink {
        kind: kind.to_string(),
        a: a.to_string(),
        b: b.to_string(),
        subnet: t[2].to_string(),
        a_ip: t[4].to_string(),
        b_ip: t[6].to_string(),
        expect_gbit: t[8].to_string(),
        state,
        divergences: Vec::new(),
    })
}

/// Parse an `uplink:` line: `uplink: <left|right> <INITIALS> <iface|unknown>`.
fn parse_uplink(line: &str) -> Option<(String, String, Option<String>)> {
    let t: Vec<&str> = line.split_whitespace().collect();
    if has_placeholder(&t) {
        return None;
    }
    if t.len() != 3 {
        return None;
    }
    let iface = if t[2] == "unknown" {
        None
    } else {
        Some(t[2].to_string())
    };
    Some((t[0].to_string(), t[1].to_string(), iface))
}

/// Parse a `proxy:` line: `proxy: host <INITIALS> port <n>`.
fn parse_proxy(line: &str) -> Option<(String, u32)> {
    let t: Vec<&str> = line.split_whitespace().collect();
    if has_placeholder(&t) {
        return None;
    }
    if t.len() != 4 || t[0] != "host" || t[2] != "port" {
        return None;
    }
    Some((t[1].to_string(), t[3].parse::<u32>().ok()?))
}

/// Parse a `divergence:` line:
/// `divergence: <D#> keys <key[,...]> - <statement and reason>`.
fn parse_divergence(line: &str) -> Option<Divergence> {
    let (id, rest) = line.split_once(' ')?;
    if id.contains('<') || id.contains('>') {
        return None;
    }
    let rest = rest.strip_prefix("keys ")?;
    let (keys, statement) = rest.split_once(" - ")?;
    if keys.contains('<') || keys.contains('>') {
        return None;
    }
    Some(Divergence {
        id: id.to_string(),
        keys: keys.split(',').map(|k| k.trim().to_string()).collect(),
        statement: statement.trim().to_string(),
    })
}

/// The grid row a scroll declares, from its coordinate: row scrolls
/// live in chapter 2 (1.1.1/1.1.1/2.<row>.1). Prose fallback: the
/// row scrolls open with "Grid row <n> of <m>".
fn scroll_row(coord: &str, text: &str) -> Option<u32> {
    let third = coord.split('/').nth(2)?;
    let mut parts = third.split('.');
    if parts.next() == Some("2") {
        if let Some(r) = parts.next().and_then(|p| p.parse::<u32>().ok()) {
            return Some(r);
        }
    }
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Grid row ") {
            if let Some(n) = rest
                .split_whitespace()
                .next()
                .and_then(|p| p.parse::<u32>().ok())
            {
                return Some(n);
            }
        }
    }
    None
}

/// A coordinate string as a sortable numeric key (same shape the
/// Data panel uses), so scrolls are read in register order.
fn coord_key(coord: &str) -> Vec<u64> {
    coord
        .split(|c| c == '/' || c == '.')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Read the wiring register: explode with libphext, walk the scrolls
/// in coordinate order, and extract only the machine-checked lines.
/// A register with no parseable `node:` line degrades rather than
/// letting the panel draw an empty fleet as if it were the truth.
fn read_register(path: &Path) -> Result<Register, String> {
    let raw = std::fs::read(path).map_err(|e| {
        format!(
            "wiring register {} is not readable from here: {}",
            path.display(),
            e
        )
    })?;
    let sha256 = crate::data::sha256_hex(&raw);
    let text = String::from_utf8(raw).map_err(|_| {
        format!(
            "wiring register {} is not valid UTF-8 — libphext parses text phexts; no topology is guessed",
            path.display()
        )
    })?;
    let scrolls = libphext::phext::explode(&text);
    let mut ordered: Vec<(String, String)> = scrolls
        .iter()
        .map(|(c, s)| (c.to_string(), s.clone()))
        .collect();
    ordered.sort_by_key(|(c, _)| coord_key(c));

    let mut nodes: Vec<WireNode> = Vec::new();
    let mut links: Vec<WireLink> = Vec::new();
    let mut uplinks: Vec<RowUplink> = Vec::new();
    let mut proxies: Vec<(u32, String, u32)> = Vec::new();
    let mut metrics: Vec<(String, u32)> = Vec::new();
    let mut divergences: Vec<Divergence> = Vec::new();

    for (coord, scroll) in &ordered {
        let row = scroll_row(coord, scroll);
        for line in scroll.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("node: ") {
                if let Some((ini, host, col, ram, state, ssh)) = parse_node(rest) {
                    nodes.push(WireNode {
                        initials: ini,
                        hostname: host,
                        row: row.unwrap_or(0),
                        col,
                        ram_gb: ram,
                        state,
                        ssh_ip: ssh,
                        uplink: UplinkClass::Unstated,
                        uplink_iface: None,
                        divergences: Vec::new(),
                    });
                }
            } else if let Some(rest) = line.strip_prefix("usb4: ") {
                if let Some(l) = parse_link("usb4", rest) {
                    links.push(l);
                }
            } else if let Some(rest) = line.strip_prefix("xlink: ") {
                if let Some(l) = parse_link("xlink", rest) {
                    links.push(l);
                }
            } else if let Some(rest) = line.strip_prefix("uplink: ") {
                if let Some((side, ini, iface)) = parse_uplink(rest) {
                    uplinks.push(RowUplink {
                        row: row.unwrap_or(0),
                        side,
                        node: ini,
                        iface,
                    });
                }
            } else if let Some(rest) = line.strip_prefix("proxy: ") {
                if let Some((host, port)) = parse_proxy(rest) {
                    proxies.push((row.unwrap_or(0), host, port));
                }
            } else if let Some(rest) = line.strip_prefix("metric: ") {
                let t: Vec<&str> = rest.split_whitespace().collect();
                if t.len() == 2 {
                    if let Ok(v) = t[1].parse::<u32>() {
                        metrics.push((t[0].to_string(), v));
                    }
                }
            } else if let Some(rest) = line.strip_prefix("divergence: ") {
                if let Some(d) = parse_divergence(rest) {
                    divergences.push(d);
                }
            }
        }
    }

    if nodes.is_empty() {
        return Err(format!(
            "wiring register {} yielded no node: lines — the fleet is never drawn from prose or from memory",
            path.display()
        ));
    }

    // Uplink classification, from the declared lines only (see the
    // module doc): an `uplink:` line means the row's 2.5G Ethernet
    // uplink; otherwise a declared link endpoint means USB4;
    // otherwise unstated. WiFi is never assigned.
    for u in &uplinks {
        if let Some(n) = nodes.iter_mut().find(|n| n.initials == u.node) {
            n.uplink = UplinkClass::Ethernet;
            n.uplink_iface = u.iface.clone();
        }
    }
    for l in &links {
        for ini in [&l.a, &l.b] {
            if let Some(n) = nodes.iter_mut().find(|n| &n.initials == ini) {
                if n.uplink == UplinkClass::Unstated {
                    n.uplink = UplinkClass::Usb4;
                }
            }
        }
    }

    // Divergence attachment by exact key. A key names its element as
    // the register writes it (`node:DW`, `usb4:KO-MB`, `uplink:NK`,
    // `xlink:QH-LO`); anything else (wildcards, policy keys, keys
    // naming undeclared elements like the KO-SC jumper) attaches to
    // nothing and stays in the panel's divergence list only.
    for d in &divergences {
        for key in &d.keys {
            if let Some(ini) = key.strip_prefix("node:") {
                if let Some(n) = nodes.iter_mut().find(|n| n.initials == ini) {
                    n.divergences.push(d.id.clone());
                }
            } else if let Some(pair) = key
                .strip_prefix("usb4:")
                .or_else(|| key.strip_prefix("xlink:"))
            {
                let kind = if key.starts_with("usb4:") {
                    "usb4"
                } else {
                    "xlink"
                };
                if let Some(l) = links
                    .iter_mut()
                    .find(|l| l.kind == kind && format!("{}-{}", l.a, l.b) == pair)
                {
                    l.divergences.push(d.id.clone());
                }
            } else if let Some(ini) = key.strip_prefix("uplink:") {
                if ini != "*" {
                    if let Some(n) = nodes.iter_mut().find(|n| n.initials == ini) {
                        n.divergences.push(d.id.clone());
                    }
                }
            }
        }
    }

    nodes.sort_by_key(|n| (n.row, n.col));
    Ok(Register {
        nodes,
        links,
        uplinks,
        proxies,
        metrics,
        divergences,
        sha256,
    })
}

/// The exact SSH command for a node, from the register's own tailnet
/// address, by the fleet's standing recipe (TOOLS.md): this VM's
/// Ed25519 key, offered alone, through the tailnet proxy. `None`
/// when the register records no address (`ssh -`) — there is no
/// honest command to show for such a node.
fn ssh_command(n: &WireNode) -> Option<String> {
    n.ssh_ip.as_ref().map(|ip| {
        format!(
            "ssh -i ~/.ssh/id_ed25519 -o IdentitiesOnly=yes -o \"ProxyCommand=nc -X connect -x hatch-egress-proxy:3130 %h %p\" wbic16@{}",
            ip
        )
    })
}

/// The fleet-ssh.sh form of the same hop, by the node's declared
/// hostname (fleet-ssh.sh routes by short name). `None` when the
/// register names no hostname or no address.
fn fleet_command(n: &WireNode) -> Option<String> {
    if n.ssh_ip.is_some() && n.hostname != "unknown" {
        Some(format!(
            "~/workspace/bickford-llama-ranch/deploy/fleet-ssh.sh {}",
            n.hostname
        ))
    } else {
        None
    }
}

pub struct WiringPanel;

impl Panel for WiringPanel {
    fn id(&self) -> &'static str {
        "wiring"
    }
    fn title(&self) -> &'static str {
        "Wiring"
    }

    fn data(&self) -> PanelData {
        let now = now_unix();
        let register_path = std::env::var("PHEXTRON_WIRING_PHEXT")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "/home/wbic16/phextron-data/phext-vm/wiring.phext".to_string());

        let file_age = std::fs::metadata(&register_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| age_from_unix(d.as_secs(), now))
            .unwrap_or_else(|| "unreadable".to_string());

        let source_name =
            "wiring.phext — the ranch network declaration of record (one scroll per row), parsed with libphext"
                .to_string();
        let source_location = format!("register {}", register_path);

        match read_register(Path::new(&register_path)) {
            Ok(reg) => {
                let count = |c: UplinkClass| reg.nodes.iter().filter(|n| n.uplink == c).count();
                let rows_max = reg.nodes.iter().map(|n| n.row).max().unwrap_or(0);
                let nodes: Vec<Value> = reg
                    .nodes
                    .iter()
                    .map(|n| {
                        json!({
                            "initials": n.initials,
                            "hostname": n.hostname,
                            "hostname_known": n.hostname != "unknown",
                            "row": n.row,
                            "col": n.col,
                            "ram_gb": n.ram_gb,
                            "state": n.state,
                            "ssh_ip": n.ssh_ip,
                            "uplink": {
                                "class": n.uplink.as_str(),
                                "iface": n.uplink_iface,
                            },
                            "divergences": n.divergences,
                            "ssh": match (ssh_command(n), fleet_command(n)) {
                                (Some(cmd), fleet) => json!({
                                    "command": cmd,
                                    "fleet_command": fleet,
                                }),
                                _ => Value::Null,
                            },
                        })
                    })
                    .collect();
                let links: Vec<Value> = reg
                    .links
                    .iter()
                    .map(|l| {
                        json!({
                            "kind": l.kind,
                            "a": l.a,
                            "b": l.b,
                            "subnet": l.subnet,
                            "a_ip": l.a_ip,
                            "b_ip": l.b_ip,
                            "expect_gbit": l.expect_gbit,
                            "state": l.state,
                            "divergences": l.divergences,
                        })
                    })
                    .collect();
                let metric = |name: &str| {
                    reg.metrics
                        .iter()
                        .find(|(k, _)| k == name)
                        .map(|(_, v)| json!(v))
                        .unwrap_or(Value::Null)
                };
                PanelData {
                    id: self.id(),
                    title: self.title(),
                    state: PanelState::Live,
                    source: SourceInfo {
                        name: source_name,
                        location: source_location,
                        freshness: format!(
                            "register read at request time · snapshot file updated {} · sha256 {}…",
                            file_age,
                            &reg.sha256[..12]
                        ),
                    },
                    body: json!({
                        "register": {
                            "nodes_recorded": reg.nodes.len(),
                            "links_recorded": reg.links.len(),
                            "rows": rows_max,
                            "counts": {
                                "ethernet": count(UplinkClass::Ethernet),
                                "usb4": count(UplinkClass::Usb4),
                                "wifi": count(UplinkClass::Wifi),
                                "unstated": count(UplinkClass::Unstated),
                            },
                            "metrics": {
                                "ethernet": metric("ethernet"),
                                "usb4": metric("usb4"),
                                "wifi": metric("wifi"),
                            },
                            "sha256": reg.sha256,
                            "file_age": file_age,
                            "double_click": "command",
                            "double_click_note": "§1.9 cluster session attach is not built yet — double-clicking a node shows its exact SSH command instead of attaching, and says so.",
                        },
                        "nodes": nodes,
                        "links": links,
                        "uplinks": reg.uplinks.iter().map(|u| json!({
                            "row": u.row, "side": u.side, "node": u.node, "iface": u.iface,
                        })).collect::<Vec<Value>>(),
                        "proxies": reg.proxies.iter().map(|(r, h, p)| json!({
                            "row": r, "host": h, "port": p,
                        })).collect::<Vec<Value>>(),
                        "divergences": reg.divergences.iter().map(|d| json!({
                            "id": d.id, "keys": d.keys, "statement": d.statement,
                        })).collect::<Vec<Value>>(),
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
            "phextron-wiring-test-{}-{}",
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

    // Two rows in the register's own declared forms, trimmed from
    // the real wiring.phext (rows 1 and 5), plus the divergence
    // scroll. The row-5 prose keeps its "QH acts as this row's
    // proxy host" line and row 1 keeps a WiFi prose mention — prose
    // must never move a classification.
    const ROW1: &str = "Row 1 - NK AH LP EP\n===================\nGrid row 1 of 6 (diagram coordinates (1,1) to (4,1)). A USB4\nchain of four with a 2.5G Ethernet uplink at each edge.\n\nnode: NK ni-knight col 1 ram 48 state online ssh 100.93.140.59\nnode: AH ashfall-haven col 2 ram 32 state online ssh 100.102.119.123\nnode: LP logos-prime col 3 ram 16 state online ssh 100.107.204.62\nnode: EP elven-path col 4 ram 32 state online ssh 100.120.211.72\n\nusb4: NK-AH subnet 10.54.11.0/24 NK 10.54.11.1 AH 10.54.11.2 expect-gbit 10-40\nusb4: AH-LP subnet 10.54.12.0/24 AH 10.54.12.1 LP 10.54.12.2 expect-gbit 10-40\nusb4: LP-EP subnet 10.54.13.0/24 LP 10.54.13.1 EP 10.54.13.2 expect-gbit 10-40\n\nuplink: left NK enp3s0\nuplink: right EP enp3s0\n\nproxy: host NK port 3129\n\nNotes: the legacy proxy is reached over WiFi today; NK is on\nWiFi in prose only, which must not classify it.\n";

    const ROW5: &str = "Row 5 - AM JK GW QH\n===================\nGrid row 5 of 6 (diagram coordinates (1,5) to (4,5)). A USB4\nchain of four with a 2.5G Ethernet uplink at each edge.\n\nnode: AM unknown col 1 ram 16 state unlocated ssh -\nnode: JK jacked-kite col 2 ram 32 state online ssh 100.82.52.27\nnode: GW great-white col 3 ram 32 state key-refused ssh -\nnode: QH quacked-hill col 4 ram 48 state online ssh 100.109.77.116\n\nusb4: AM-JK subnet 10.54.51.0/24 AM 10.54.51.1 JK 10.54.51.2 expect-gbit 10-40\nusb4: JK-GW subnet 10.54.52.0/24 JK 10.54.52.1 GW 10.54.52.2 expect-gbit 10-40\nusb4: GW-QH subnet 10.54.53.0/24 GW 10.54.53.1 QH 10.54.53.2 expect-gbit 10-40\n\nxlink: QH-LO subnet 10.54.90.0/24 QH 10.54.90.1 LO 10.54.90.2 expect-gbit 10-40 state provisional-offdiagram\n\nuplink: left AM unknown\nuplink: right QH enp2s0\n\nproxy: host QH port 3129\n";

    const DIVERGENCES: &str = "Known-divergence register\n=========================\n\ndivergence: D3 keys node:GW - great-white refuses this VM's SSH key (publickey denied).\ndivergence: D6 keys node:AM - AM-16 is unlocated: drawn at (1,5).\ndivergence: D9 keys xlink:QH-LO - an off-diagram USB4 cable joins quacked-hill and lighthouse-omen.\ndivergence: D10 keys uplink:NK - ni-knight's ethernet cable lands somewhere that is not the LAN.\ndivergence: D15 keys uplink:* - every declared uplink except NK's is uncabled as of the census.\ndivergence: D7 keys usb4:KO-SC,usb4:KO-MB - moon-beam is bypassed by a KO-SC jumper.\n";

    const METRICS: &str = "Policy - routing and route metrics\n==================================\n\nmetric: ethernet 100\nmetric: usb4 200\nmetric: wifi 600\n";

    fn fixture_text() -> String {
        // Scroll order in a phext: home, row 1 (2.1.1), row 5
        // (2.5.1), metrics (3.1.1), divergences (4.1.1) — separated
        // by the phext scroll delimiter (\u{17}, as in the Data
        // panel's fixtures).
        let home = "wiring.phext - the ranch network, declared\n";
        format!(
            "{}\u{17}{}\u{17}{}\u{17}{}\u{17}{}",
            home, ROW1, ROW5, METRICS, DIVERGENCES
        )
    }

    fn fixture_register(tag: &str) -> (Register, PathBuf) {
        let dir = temp_dir(tag);
        write(&dir, "wiring.phext", fixture_text().as_bytes());
        let reg = read_register(&dir.join("wiring.phext")).expect("register");
        (reg, dir)
    }

    fn node<'a>(reg: &'a Register, ini: &str) -> &'a WireNode {
        reg.nodes.iter().find(|n| n.initials == ini).expect("node")
    }

    #[test]
    fn sha256_shared_impl_matches_published_vectors() {
        // The panel puts this hash on its face; it is the Data
        // panel's shared std-only implementation.
        assert_eq!(
            crate::data::sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn nodes_parse_with_all_fields() {
        let (reg, dir) = fixture_register("nodes");
        assert_eq!(reg.nodes.len(), 8);
        let nk = node(&reg, "NK");
        assert_eq!(nk.hostname, "ni-knight");
        assert_eq!(nk.row, 1);
        assert_eq!(nk.col, 1);
        assert_eq!(nk.ram_gb, 48);
        assert_eq!(nk.state, "online");
        assert_eq!(nk.ssh_ip.as_deref(), Some("100.93.140.59"));
        let am = node(&reg, "AM");
        assert_eq!(am.row, 5);
        assert_eq!(am.hostname, "unknown");
        assert_eq!(am.state, "unlocated");
        assert_eq!(am.ssh_ip, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uplink_classification_comes_from_declared_lines_only() {
        let (reg, dir) = fixture_register("classify");
        // Edge nodes named by uplink: lines are Ethernet — NK even
        // though its scroll's prose talks about WiFi, AM even
        // though its interface is unstated.
        assert_eq!(node(&reg, "NK").uplink, UplinkClass::Ethernet);
        assert_eq!(node(&reg, "NK").uplink_iface.as_deref(), Some("enp3s0"));
        assert_eq!(node(&reg, "AM").uplink, UplinkClass::Ethernet);
        assert_eq!(node(&reg, "AM").uplink_iface, None);
        assert_eq!(node(&reg, "QH").uplink, UplinkClass::Ethernet);
        // Middle nodes ride their declared USB4 links.
        assert_eq!(node(&reg, "AH").uplink, UplinkClass::Usb4);
        assert_eq!(node(&reg, "GW").uplink, UplinkClass::Usb4);
        // WiFi is assigned to no node: the register declares no
        // per-node WiFi uplink, whatever the prose says.
        assert!(reg.nodes.iter().all(|n| n.uplink != UplinkClass::Wifi));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_with_no_declared_path_is_unstated() {
        // A node in no uplink: line and no link stays UNSTATED —
        // never promoted to a guessed medium.
        let text = "Grid row 2 of 6\n\nnode: ZZ zeta-zed col 2 ram 16 state online ssh 100.1.2.3\n";
        let dir = temp_dir("unstated");
        write(&dir, "wiring.phext", text.as_bytes());
        let reg = read_register(&dir.join("wiring.phext")).expect("register");
        assert_eq!(reg.nodes[0].uplink, UplinkClass::Unstated);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn links_parse_in_row_and_cross_row() {
        let (reg, dir) = fixture_register("links");
        assert_eq!(reg.links.len(), 7);
        let chain = reg
            .links
            .iter()
            .find(|l| l.a == "NK" && l.b == "AH")
            .expect("chain link");
        assert_eq!(chain.kind, "usb4");
        assert_eq!(chain.subnet, "10.54.11.0/24");
        assert_eq!(chain.a_ip, "10.54.11.1");
        assert_eq!(chain.b_ip, "10.54.11.2");
        assert_eq!(chain.expect_gbit, "10-40");
        assert_eq!(chain.state, None);
        let x = reg.links.iter().find(|l| l.kind == "xlink").expect("xlink");
        assert_eq!((x.a.as_str(), x.b.as_str()), ("QH", "LO"));
        assert_eq!(x.state.as_deref(), Some("provisional-offdiagram"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_link_lines_are_not_drawn() {
        // Endpoints that disagree with the pair token: not a
        // declaration, not drawn.
        assert!(parse_link(
            "usb4",
            "NK-AH subnet 10.54.11.0/24 NK 10.54.11.1 LP 10.54.11.2 expect-gbit 10-40"
        )
        .is_none());
        assert!(parse_link("usb4", "NK-AH subnet 10.54.11.0/24 NK 10.54.11.1").is_none());
    }

    #[test]
    fn divergences_attach_by_exact_key_only() {
        let (reg, dir) = fixture_register("divergences");
        assert_eq!(node(&reg, "GW").divergences, vec!["D3".to_string()]);
        assert_eq!(node(&reg, "AM").divergences, vec!["D6".to_string()]);
        // uplink:NK attaches to NK; the wildcard uplink:* (D15)
        // attaches to no node.
        assert_eq!(node(&reg, "NK").divergences, vec!["D10".to_string()]);
        assert!(reg
            .nodes
            .iter()
            .all(|n| !n.divergences.contains(&"D15".to_string())));
        // xlink:QH-LO attaches to the xlink.
        let x = reg.links.iter().find(|l| l.kind == "xlink").expect("xlink");
        assert_eq!(x.divergences, vec!["D9".to_string()]);
        // D7 keys usb4:KO-SC / usb4:KO-MB name links this fixture
        // does not declare: they attach to nothing.
        assert!(reg
            .links
            .iter()
            .all(|l| !l.divergences.contains(&"D7".to_string())));
        // All six stay in the register list regardless.
        assert_eq!(reg.divergences.len(), 6);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn metrics_and_proxies_parse() {
        let (reg, dir) = fixture_register("metrics");
        assert!(reg.metrics.contains(&("ethernet".to_string(), 100)));
        assert!(reg.metrics.contains(&("usb4".to_string(), 200)));
        assert!(reg.metrics.contains(&("wifi".to_string(), 600)));
        assert!(reg.proxies.contains(&(1, "NK".to_string(), 3129)));
        assert!(reg.proxies.contains(&(5, "QH".to_string(), 3129)));
        assert_eq!(reg.uplinks.len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ssh_command_built_only_from_a_declared_address() {
        let (reg, dir) = fixture_register("ssh");
        let cmd = ssh_command(node(&reg, "NK")).expect("command");
        assert_eq!(
            cmd,
            "ssh -i ~/.ssh/id_ed25519 -o IdentitiesOnly=yes -o \"ProxyCommand=nc -X connect -x hatch-egress-proxy:3130 %h %p\" wbic16@100.93.140.59"
        );
        assert_eq!(
            fleet_command(node(&reg, "NK")).as_deref(),
            Some("~/workspace/bickford-llama-ranch/deploy/fleet-ssh.sh ni-knight")
        );
        // ssh - in the register: no command, no fleet form.
        assert_eq!(ssh_command(node(&reg, "AM")), None);
        assert_eq!(fleet_command(node(&reg, "AM")), None);
        assert_eq!(ssh_command(node(&reg, "GW")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn register_hash_is_the_hash_of_what_was_read() {
        let (reg, dir) = fixture_register("hash");
        assert_eq!(reg.sha256, crate::data::sha256_hex(fixture_text().as_bytes()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_register_is_an_error_that_names_the_path() {
        let dir = std::env::temp_dir().join("phextron-wiring-test-definitely-absent.phext");
        let err = read_register(&dir).expect_err("must fail");
        assert!(err.contains("not readable"), "error names the failure: {}", err);
        assert!(err.contains("phextron-wiring-test-definitely-absent"));
    }

    #[test]
    fn convention_form_lines_are_not_declarations() {
        // The register's Conventions scroll states every form with
        // <placeholder> tokens. Fed the real forms verbatim, the
        // parsers must yield nothing — a form drawn as a node, a
        // link, or a divergence is a phantom. (Regression: the
        // first live read of the real register produced exactly
        // these phantoms before the placeholder guard.)
        assert!(parse_node("<INITIALS> <hostname|unknown> col <1-4> ram <GB> state <state> ssh <tailnet-ip|->").is_none());
        assert!(parse_link("usb4", "<A>-<B> subnet <10.54.x.0/24> <A> <ip> <B> <ip> expect-gbit 10-40").is_none());
        assert!(parse_link("xlink", "<A>-<B> subnet <10.54.9x.0/24> <A> <ip> <B> <ip> expect-gbit 10-40 state <status>").is_none());
        assert!(parse_uplink("<left|right> <INITIALS> <iface|unknown>").is_none());
        assert!(parse_proxy("host <INITIALS> port 3129").is_none());
        assert!(parse_divergence("<D#> keys <key[,...]> - <statement and reason>").is_none());
    }

    #[test]
    fn prose_only_register_degrades_by_name() {
        // A file full of wiring prose but no machine-checked lines
        // must not become a fleet.
        let dir = temp_dir("prose");
        write(
            &dir,
            "wiring.phext",
            b"Everything is wired to everything else, trust me.\n",
        );
        let err = read_register(&dir.join("wiring.phext")).expect_err("must fail");
        assert!(err.contains("no node: lines"), "names the gap: {}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

