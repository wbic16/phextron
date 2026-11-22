//! Host-side tests for the web UI's parser and renderers. The WASM module
//! is a pure function from panel JSON to HTML, so the whole rendering path
//! is testable without a browser; the browser pass then only has to prove
//! the module loads and runs (see verify_browser.py).

use phextron_web::json::{parse, Json};
use phextron_web::render_panel_json;

#[test]
fn parser_reads_the_console_shapes() {
    let doc = parse(
        r#"{"a": 23, "b": 2.50, "c": null, "d": true, "e": [1, "x"], "f": {"g": "hi\nthere"}, "h": "🔥"}"#,
    )
    .expect("parse");
    assert_eq!(doc.get("a"), Some(&Json::Num("23".to_string())));
    // The lexeme is kept verbatim — display never reformats a number.
    assert_eq!(doc.get("b"), Some(&Json::Num("2.50".to_string())));
    assert_eq!(doc.get("c"), Some(&Json::Null));
    assert_eq!(doc.get("d"), Some(&Json::Bool(true)));
    assert_eq!(doc.get("e").and_then(|v| v.as_arr()).map(|a| a.len()), Some(2));
    assert_eq!(
        doc.get("f").and_then(|f| f.get("g")).and_then(|g| g.as_str()),
        Some("hi\nthere")
    );
    assert_eq!(doc.get("h").and_then(|h| h.as_str()), Some("🔥"));
}

#[test]
fn parser_decodes_surrogate_pairs() {
    let doc = parse(r#"{"x": "\ud83d\udd25"}"#).expect("parse");
    assert_eq!(doc.get("x").and_then(|x| x.as_str()), Some("🔥"));
}

#[test]
fn parser_rejects_garbage() {
    assert!(parse("{").is_err());
    assert!(parse("[1,").is_err());
    assert!(parse("{} extra").is_err());
    assert!(parse("{\"a\": tru}").is_err());
}

/// A fixture in the exact shape crates/phextron-dash's NodesPanel emits:
/// one up node with full telemetry, one down node with nulls and a hostile
/// `top` string, a distilled banner, and a partial counts map.
const NODES_JSON: &str = r#"{
  "id": "nodes", "title": "Ranch Nodes", "state": "live",
  "source": {"name": "ranch fanout pull", "location": "pull /x/last-pull.json", "freshness": "pull updated 42s ago"},
  "body": {
    "pull_ts": "2026-11-21T23:00:00Z", "pull_coordinate": null,
    "counts": {"up": 1, "down": 1},
    "roster_count": 2,
    "distilled": {"summary": "1/2 nodes up", "ts": "2026-11-21 17:53 CST", "scrolls": 12, "entries": 34, "register_age": "updated 5m ago"},
    "rows": [
      {"name": "alpha", "display": "Alpha", "dns": "alpha.lan", "ip": "100.0.0.1", "enabled": true,
       "note": "", "status": "up", "load": 8.0, "ncpu": 16, "tctl_c": 61.5, "gpu_c": 55,
       "ollama": "up", "render": "idle", "gateway": "up", "top": "ollama serve"},
      {"name": "beta", "display": "Beta", "dns": "beta.lan", "ip": "100.0.0.2", "enabled": false,
       "note": "", "status": "down", "load": null, "ncpu": null, "tctl_c": null, "gpu_c": null,
       "ollama": null, "render": null, "gateway": null, "top": "<script>alert(1)</script>"}
    ]
  }
}"#;

#[test]
fn nodes_panel_renders_rows_chips_and_banner() {
    let html = render_panel_json(NODES_JSON);
    // Banner from the distilled register entry.
    assert!(html.contains("1/2 nodes up"), "banner summary: {}", html);
    assert!(html.contains("12 scrolls, 34 entries"), "banner register: {}", html);
    // Chips: roster + only the counts present, in the shell's fixed order.
    assert!(html.contains("roster <b>2</b>"), "roster chip: {}", html);
    assert!(html.contains("up <b>1</b>"), "up chip: {}", html);
    assert!(html.contains("down <b>1</b>"), "down chip: {}", html);
    assert!(!html.contains("no-access <b>"), "absent counts stay absent: {}", html);
    // Both rows render; per-core load is computed like the JS does.
    assert_eq!(html.matches("<tr><td>").count(), 2, "two body rows: {}", html);
    assert!(html.contains("st-up"), "status class: {}", html);
    assert!(html.contains("8 (0.50/core)"), "per-core load: {}", html);
    // Nulls render the em dash, and the disabled badge shows.
    assert!(html.contains("<td class=\"num\">—</td>"), "null dash: {}", html);
    assert!(html.contains("<span class=\"badge\">disabled</span>"), "badge: {}", html);
}

#[test]
fn nodes_panel_escapes_like_the_shell() {
    let html = render_panel_json(NODES_JSON);
    assert!(
        html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
        "top is escaped: {}",
        html
    );
    assert!(!html.contains("<script>alert"), "no raw injection: {}", html);
}

#[test]
fn degraded_panel_renders_its_error_on_its_face() {
    let html = render_panel_json(
        r#"{"id": "nodes", "title": "Ranch Nodes", "state": "degraded",
            "source": {"name": "ranch fanout pull", "location": "pull /x", "freshness": "pull unreadable"},
            "body": {"error": "pull unreadable: No such file or directory"}}"#,
    );
    assert!(html.contains("degraded"), "badge: {}", html);
    assert!(html.contains("pull unreadable: No such file or directory"), "error: {}", html);
}

#[test]
fn unparseable_input_renders_an_error_card_not_a_panic() {
    let html = render_panel_json("{not json");
    assert!(html.contains("Panel data unreadable"), "error card: {}", html);
    assert!(html.contains("json parse error"), "names the failure: {}", html);
}
