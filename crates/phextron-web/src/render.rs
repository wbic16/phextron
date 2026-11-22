//! Panel renderers for the web UI — the console's rendering logic in Rust.
//!
//! Stage 1 ports ONE panel: ranch nodes. `render_nodes` is a line-for-line
//! port of the desktop shell's `renderNodes` (crates/phextron-dash/web/
//! shell.html) — same HTML, same classes, same escaping — so the WASM build
//! and the desktop shell render the same panel identically from the same
//! `/api/panels/nodes` JSON. The remaining panels port onto this crate in
//! stage 2, one at a time, the same way.
//!
//! Degraded panels render through the port of the shell's `renderDegraded`:
//! a panel whose live source is unavailable says so on its face, in the web
//! UI exactly as on the desktop.

use crate::json::Json;

/// The shell's `esc()`: HTML-escape a display string.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a JSON value's display form (the JS renderers esc() everything).
fn escv(v: &Json) -> String {
    esc(&v.display())
}

/// Escape an optional field's display form; absent renders empty, exactly
/// as the JS renderers render `undefined` through esc().
fn escf(v: Option<&Json>) -> String {
    match v {
        Some(j) => escv(j),
        None => String::new(),
    }
}

/// The shell's `num()`: null/missing renders the em dash, else the value.
fn num(v: Option<&Json>) -> String {
    match v {
        None => "—".to_string(),
        Some(j) if j.is_null() => "—".to_string(),
        Some(j) => escv(j),
    }
}

/// A string field with the JS `||` fallback: null, missing, or empty
/// yields the fallback.
fn str_or<'a>(v: Option<&'a Json>, fallback: &'a str) -> &'a str {
    match v.and_then(|j| j.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => fallback,
    }
}

fn source_line(d: &Json, freshness: bool) -> String {
    let src = d.get("source");
    let name = src.and_then(|s| s.get("name")).map(|v| v.display()).unwrap_or_default();
    let location = src
        .and_then(|s| s.get("location"))
        .map(|v| v.display())
        .unwrap_or_default();
    let mut out = format!(
        "<p class=\"src\">Source: <b>{}</b> · {}",
        esc(&name),
        esc(&location)
    );
    if freshness {
        let f = src
            .and_then(|s| s.get("freshness"))
            .map(|v| v.display())
            .unwrap_or_default();
        out.push_str(&format!("<br>Freshness: {}</p>", esc(&f)));
    } else {
        out.push_str("</p>");
    }
    out
}

/// Port of the shell's `renderNodes`.
fn render_nodes(d: &Json) -> String {
    let b = match d.get("body") {
        Some(body) => body,
        None => return render_degraded(d),
    };
    let mut out = source_line(d, true);

    if let Some(dist) = b.get("distilled") {
        if !dist.is_null() {
            out.push_str(&format!(
                "<div class=\"banner\"><div>{}</div><div class=\"coord\">state.phext · newest distilled entry {} · register: {} scrolls, {} entries · {}</div></div>",
                escf(dist.get("summary")),
                escf(dist.get("ts")),
                escf(dist.get("scrolls")),
                escf(dist.get("entries")),
                escf(dist.get("register_age")),
            ));
        }
    }

    let counts = b.get("counts");
    out.push_str("<div class=\"chips\">");
    out.push_str(&format!(
        "<span class=\"chip\">roster <b>{}</b></span>",
        num(b.get("roster_count"))
    ));
    for key in ["up", "thermal-hold", "no-access", "down", "not-in-pull"] {
        if let Some(c) = counts.and_then(|c| c.get(key)) {
            // JS: `if (b.counts[k])` — zero counts are falsy and skipped.
            if c.as_f64().map(|n| n != 0.0).unwrap_or(false) {
                out.push_str(&format!(
                    "<span class=\"chip\">{} <b>{}</b></span>",
                    esc(key),
                    escv(c)
                ));
            }
        }
    }
    out.push_str("</div>");

    out.push_str(
        "<table><thead><tr><th>Node</th><th>Status</th><th>Load</th><th>Cores</th>\
         <th>Tctl °C</th><th>GPU °C</th><th>Ollama</th><th>Render</th><th>Gateway</th><th>Top</th></tr></thead><tbody>",
    );
    if let Some(rows) = b.get("rows").and_then(|r| r.as_arr()) {
        for r in rows {
            let load = r.get("load");
            let ncpu = r.get("ncpu");
            let per = match (load.and_then(|v| v.as_f64()), ncpu.and_then(|v| v.as_f64())) {
                (Some(l), Some(n)) if n != 0.0 => format!(" ({:.2}/core)", l / n),
                _ => String::new(),
            };
            let enabled = r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
            let status = str_or(r.get("status"), "");
            let top = str_or(r.get("top"), "");
            out.push_str(&format!(
                "<tr><td><b>{}</b>{}<br><span class=\"top\">{}</span></td>\
                 <td class=\"st st-{}\">{}</td>\
                 <td class=\"num\">{}{}</td>\
                 <td class=\"num\">{}</td>\
                 <td class=\"num\">{}</td>\
                 <td class=\"num\">{}</td>\
                 <td>{}</td><td>{}</td><td>{}</td>\
                 <td class=\"top\" title=\"{}\">{}</td></tr>",
                esc(str_or(r.get("display"), "")),
                if enabled {
                    ""
                } else {
                    " <span class=\"badge\">disabled</span>"
                },
                esc(str_or(r.get("ip"), "")),
                esc(status),
                esc(status),
                num(load),
                per,
                num(ncpu),
                num(r.get("tctl_c")),
                num(r.get("gpu_c")),
                esc(str_or(r.get("ollama"), "—")),
                esc(str_or(r.get("render"), "—")),
                esc(str_or(r.get("gateway"), "—")),
                esc(top),
                esc(top),
            ));
        }
    }
    out.push_str("</tbody></table>");
    out
}

/// Port of the shell's `renderDegraded`.
fn render_degraded(d: &Json) -> String {
    let mut out = source_line(d, false);
    let title = d.get("title").map(|v| v.display()).unwrap_or_default();
    let err = d
        .get("body")
        .and_then(|b| b.get("error"))
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            d.get("source")
                .and_then(|s| s.get("freshness"))
                .map(|v| v.display())
                .unwrap_or_default()
        });
    out.push_str(&format!(
        "<div class=\"card\"><h2>{}<span class=\"badge\">degraded</span></h2><p class=\"err\">{}</p></div>",
        esc(&title),
        esc(&err)
    ));
    out
}

/// A live panel that has not been ported to the web UI yet. The web shell
/// only requests the nodes panel in stage 1, so this is a defensive path —
/// and it names the gap on its face rather than rendering nothing.
fn render_not_ported(d: &Json) -> String {
    let mut out = source_line(d, true);
    let title = d.get("title").map(|v| v.display()).unwrap_or_default();
    out.push_str(&format!(
        "<div class=\"card\"><h2>{}<span class=\"badge\">web stage 2</span></h2>\
         <p>This panel has not been ported to the WASM web UI yet — it ports in stage 2. \
         The desktop shell renders it today.</p></div>",
        esc(&title)
    ));
    out
}

/// Render one panel's JSON (the exact document `/api/panels/<id>` serves)
/// to the HTML the shell places in its panel element.
pub fn render_panel(d: &Json) -> String {
    let state = d.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let id = d.get("id").and_then(|v| v.as_str()).unwrap_or("");
    match state {
        "degraded" => render_degraded(d),
        "live" if id == "nodes" => render_nodes(d),
        "live" => render_not_ported(d),
        _ => render_degraded(d),
    }
}
