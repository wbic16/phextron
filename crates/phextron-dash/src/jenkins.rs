//! Jenkins panel — the stage-2 live panel.
//!
//! Reads the fleet's one Jenkins controller through its own REST API —
//! `/api/json` (jobs + last builds), `/computer/api/json` (agents),
//! `/queue/api/json` (the build queue), `/whoAmI/api/json` (the identity
//! the reader runs as). Never the HTML UI: rendered pages are not data
//! plumbing (docs/RECON.md §2).
//!
//! The controller's realm is the house LDAP (the console auth ruling:
//! Desktop SSO for humans, MCP Auth for agents) and anonymous reads are
//! refused, so the reader authenticates with a read-only API token taken
//! from the environment — never from the repo, never printed:
//!   PHEXTRON_JENKINS_URL    controller base (default http://100.88.154.1:8080)
//!   PHEXTRON_JENKINS_USER   the read identity (e.g. a read-only service account)
//!   PHEXTRON_JENKINS_TOKEN  that identity's API token
//!
//! Until that identity is provisioned the panel degrades honestly: it
//! still asks the API, and reports the answer it got (anonymous reads are
//! refused with 403 today) instead of inventing job or agent state. The
//! provisioning steps live in docs/RECON.md §2.

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_URL: &str = "http://100.88.154.1:8080";
const ROOT_TREE: &str = "jobs[name,url,color,buildable,inQueue,lastBuild[number,url,result,timestamp,duration,building]],mode,numExecutors,quietingDown";
const COMPUTER_TREE: &str = "busyExecutors,totalExecutors,computer[displayName,offline,temporarilyOffline,offlineCauseReason,numExecutors,idle]";
const QUEUE_TREE: &str = "items[task[name,url],why,inQueueSince,blocked,buildable,stuck]";

// ---------------------------------------------------------------------------
// Minimal std HTTP client. The controller is plain HTTP on the tailnet, so
// no TLS stack is needed; an https:// URL is refused with a clear message
// rather than silently downgraded. Basic auth only, `Connection: close`,
// Content-Length and chunked bodies both understood.
// ---------------------------------------------------------------------------

struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0] as u32,
            *chunk.get(1).unwrap_or(&0) as u32,
            *chunk.get(2).unwrap_or(&0) as u32,
        ];
        let n = (b[0] << 16) | (b[1] << 8) | b[2];
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Split `http://host[:port]/path` into (host, port, path).
fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    if let Some(rest) = url.strip_prefix("https://") {
        let _ = rest;
        return Err(format!(
            "controller URL {} is https — this std-only reader speaks plain HTTP on the tailnet; use the controller's http:// tailnet address",
            url
        ));
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("controller URL {} is not an http:// URL", url))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rfind(':') {
        Some(i) if authority[i + 1..].chars().all(|c| c.is_ascii_digit()) => (
            authority[..i].to_string(),
            authority[i + 1..].parse::<u16>().unwrap_or(80),
        ),
        _ => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return Err(format!("controller URL {} has no host", url));
    }
    Ok((host, port, path.to_string()))
}

fn decode_chunked(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let line_end = raw[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .map(|i| pos + i)
            .ok_or("chunked body: missing chunk-size line")?;
        let size_str = String::from_utf8_lossy(&raw[pos..line_end]).trim().to_string();
        let size = usize::from_str_radix(size_str.split(';').next().unwrap_or(""), 16)
            .map_err(|_| "chunked body: bad chunk size")?;
        pos = line_end + 2;
        if size == 0 {
            break;
        }
        if pos + size > raw.len() {
            return Err("chunked body: truncated chunk".into());
        }
        out.extend_from_slice(&raw[pos..pos + size]);
        pos += size + 2; // chunk data + trailing CRLF
    }
    Ok(out)
}

fn http_get(url: &str, auth: Option<&str>) -> Result<HttpResponse, String> {
    let (host, port, path) = parse_http_url(url)?;
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {}: {}", host, e))?
        .next()
        .ok_or_else(|| format!("cannot resolve {}", host))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("cannot connect to {}:{}: {}", host, port, e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: phextron-dash\r\nAccept: application/json\r\nConnection: close\r\n",
        path, host
    );
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: Basic {}\r\n", a));
    }
    req.push_str("\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("request write failed: {}", e))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("response read failed: {}", e))?;
    let head_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("response has no header terminator")?;
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or("response has no status line")?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let body_bytes = &raw[head_end + 4..];
    let chunked = headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.to_ascii_lowercase().contains("chunked"));
    let body = if chunked {
        String::from_utf8_lossy(&decode_chunked(body_bytes)?).to_string()
    } else {
        String::from_utf8_lossy(body_bytes).to_string()
    };
    Ok(HttpResponse { status, headers, body })
}

// ---------------------------------------------------------------------------
// API -> panel body. Pure functions over the API's JSON so the fixtures
// captured from a real Jenkins 2.580.2 (tests/fixtures/) pin the shapes.
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn age_from_ms(ts_ms: u64, now: u64) -> String {
    let s = now.saturating_sub(ts_ms) / 1000;
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

fn human_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else if ms < 3_600_000 {
        format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000)
    } else {
        format!("{}h {}m", ms / 3_600_000, (ms % 3_600_000) / 60_000)
    }
}

/// Jenkins `why` strings for queue items carry HTML; the panel shows text.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The build result the panel shows: the last build's own result when it
/// has one, BUILDING while it runs, else the job color's standing result.
fn job_result(job: &Value) -> String {
    let color = job.get("color").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(lb) = job.get("lastBuild") {
        if lb.get("building").and_then(|v| v.as_bool()) == Some(true) {
            return "BUILDING".into();
        }
        if let Some(r) = lb.get("result").and_then(|v| v.as_str()) {
            return r.to_string();
        }
    }
    match color.strip_suffix("_anime") {
        Some(_) => "BUILDING".into(),
        None => match color {
            "blue" => "SUCCESS".into(),
            "red" => "FAILURE".into(),
            "yellow" => "UNSTABLE".into(),
            "aborted" => "ABORTED".into(),
            "disabled" => "DISABLED".into(),
            _ => "NOT_BUILT".into(),
        },
    }
}

fn jobs_body(root: &Value, now: u64) -> (Vec<Value>, Value) {
    let mut rows = Vec::new();
    let mut counts = serde_json::Map::new();
    let bump = |counts: &mut serde_json::Map<String, Value>, key: &str| {
        let cur = counts.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
        counts.insert(key.to_string(), json!(cur + 1));
    };
    if let Some(jobs) = root.get("jobs").and_then(|v| v.as_array()) {
        for j in jobs {
            let result = job_result(j);
            bump(&mut counts, &result);
            let lb = j.get("lastBuild");
            let ts = lb
                .and_then(|b| b.get("timestamp"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            rows.push(json!({
                "name": j.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                "result": result,
                "buildable": j.get("buildable").and_then(|v| v.as_bool()).unwrap_or(false),
                "in_queue": j.get("inQueue").and_then(|v| v.as_bool()).unwrap_or(false),
                "last_number": lb.and_then(|b| b.get("number")).cloned().unwrap_or(Value::Null),
                "duration": lb.and_then(|b| b.get("duration")).and_then(|v| v.as_u64())
                    .map(human_duration_ms),
                "age": if ts > 0 { Some(age_from_ms(ts, now)) } else { None },
            }));
        }
    }
    (rows, Value::Object(counts))
}

fn agents_body(computer: &Value) -> (Vec<Value>, Value) {
    let mut rows = Vec::new();
    let mut online = 0usize;
    if let Some(agents) = computer.get("computer").and_then(|v| v.as_array()) {
        for a in agents {
            let offline = a.get("offline").and_then(|v| v.as_bool()).unwrap_or(true);
            let held = a
                .get("temporarilyOffline")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let (status, status_key) = if !offline {
                online += 1;
                ("online", "online")
            } else if held {
                ("held offline", "held")
            } else {
                ("offline", "offline")
            };
            rows.push(json!({
                "name": a.get("displayName").and_then(|v| v.as_str()).unwrap_or("?"),
                "status": status,
                "status_key": status_key,
                "executors": a.get("numExecutors").and_then(|v| v.as_u64()).unwrap_or(0),
                "idle": a.get("idle").and_then(|v| v.as_bool()).unwrap_or(false),
                "cause": a.get("offlineCauseReason").and_then(|v| v.as_str()).unwrap_or(""),
            }));
        }
    }
    let summary = json!({
        "total": rows.len(),
        "online": online,
        "executors_total": computer.get("totalExecutors").cloned().unwrap_or(Value::Null),
        "executors_busy": computer.get("busyExecutors").cloned().unwrap_or(Value::Null),
    });
    (rows, summary)
}

fn queue_body(queue: &Value, now: u64) -> Value {
    let mut items = Vec::new();
    if let Some(list) = queue.get("items").and_then(|v| v.as_array()) {
        for it in list {
            let since = it.get("inQueueSince").and_then(|v| v.as_u64()).unwrap_or(0);
            items.push(json!({
                "job": it.get("task").and_then(|t| t.get("name")).and_then(|v| v.as_str()).unwrap_or("?"),
                "why": strip_tags(it.get("why").and_then(|v| v.as_str()).unwrap_or("")),
                "waiting": if since > 0 { Some(age_from_ms(since, now)) } else { None },
            }));
        }
    }
    json!({ "length": items.len(), "items": items })
}

// ---------------------------------------------------------------------------
// The panel.
// ---------------------------------------------------------------------------

pub struct JenkinsPanel;

struct Config {
    base: String,
    user: Option<String>,
    token: Option<String>,
}

impl Config {
    fn from_env() -> Config {
        let base = std::env::var("PHEXTRON_JENKINS_URL")
            .unwrap_or_else(|_| DEFAULT_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        Config {
            base,
            user: std::env::var("PHEXTRON_JENKINS_USER").ok().filter(|s| !s.is_empty()),
            token: std::env::var("PHEXTRON_JENKINS_TOKEN").ok().filter(|s| !s.is_empty()),
        }
    }
    fn auth(&self) -> Option<String> {
        match (&self.user, &self.token) {
            (Some(u), Some(t)) => Some(base64_encode(format!("{}:{}", u, t).as_bytes())),
            _ => None,
        }
    }
}

impl JenkinsPanel {
    fn degraded(&self, cfg: &Config, why: String) -> PanelData {
        PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Degraded,
            source: SourceInfo {
                name: "Jenkins controller REST API (/api/json · /computer/api/json · /queue/api/json)".into(),
                location: format!(
                    "aletheia-core — {} (tailnet) — the fleet's one controller; read identity via PHEXTRON_JENKINS_USER / PHEXTRON_JENKINS_TOKEN",
                    cfg.base
                ),
                freshness: why.clone(),
            },
            body: json!({ "error": why }),
        }
    }

    /// Fetch one API endpoint as JSON. On a non-200 the error message names
    /// the refusal honestly, in terms of whether a read identity is
    /// configured at all.
    fn fetch(&self, cfg: &Config, path: &str) -> Result<(Value, Option<String>), String> {
        let resp = http_get(&format!("{}{}", cfg.base, path), cfg.auth().as_deref())
            .map_err(|e| format!("controller unreachable at {}: {}", cfg.base, e))?;
        let version = resp.header("X-Jenkins").map(|s| s.to_string());
        match resp.status {
            200 => serde_json::from_str(&resp.body)
                .map(|v| (v, version))
                .map_err(|e| format!("controller answered 200 but the body was not JSON: {}", e)),
            401 => Err(match &cfg.user {
                Some(u) => format!(
                    "the controller refused the configured credentials for '{}' (401) — the API token may be revoked or rotated; the panel shows nothing rather than stale state",
                    u
                ),
                None => "the controller refused the read (401) and no read identity is configured — set PHEXTRON_JENKINS_USER / PHEXTRON_JENKINS_TOKEN to a read-only API token (provisioning steps: docs/RECON.md §2)".into(),
            }),
            403 => Err(match &cfg.user {
                Some(u) => format!(
                    "the identity '{}' authenticated but is not authorized to read (403) — it needs Read grants in the controller's authorization matrix; the panel shows nothing rather than stale state",
                    u
                ),
                None => "the controller refused an anonymous read (403) and no read identity is configured — set PHEXTRON_JENKINS_USER / PHEXTRON_JENKINS_TOKEN to a read-only API token (provisioning steps: docs/RECON.md §2)".into(),
            }),
            other => Err(format!(
                "the controller answered {} for {} — the panel shows nothing rather than stale state",
                other, path
            )),
        }
    }
}

impl Panel for JenkinsPanel {
    fn id(&self) -> &'static str {
        "jenkins"
    }
    fn title(&self) -> &'static str {
        "Jenkins Instances"
    }

    fn data(&self) -> PanelData {
        let cfg = Config::from_env();
        let now = now_ms();

        let (root, version) = match self.fetch(
            &cfg,
            &format!("/api/json?tree={}", ROOT_TREE),
        ) {
            Ok(v) => v,
            Err(e) => return self.degraded(&cfg, e),
        };
        let (computer, _) = match self.fetch(
            &cfg,
            &format!("/computer/api/json?tree={}", COMPUTER_TREE),
        ) {
            Ok(v) => v,
            Err(e) => return self.degraded(&cfg, e),
        };
        let (queue, _) = match self.fetch(&cfg, &format!("/queue/api/json?tree={}", QUEUE_TREE)) {
            Ok(v) => v,
            Err(e) => return self.degraded(&cfg, e),
        };
        // The reader's own identity is decoration on the panel's face —
        // its failure never degrades the panel.
        let reading_as = self
            .fetch(&cfg, "/whoAmI/api/json")
            .ok()
            .and_then(|(v, _)| v.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()));

        let (jobs, counts) = jobs_body(&root, now);
        let (agents, agents_summary) = agents_body(&computer);
        let queue = queue_body(&queue, now);

        PanelData {
            id: self.id(),
            title: self.title(),
            state: PanelState::Live,
            source: SourceInfo {
                name: "Jenkins controller REST API (/api/json · /computer/api/json · /queue/api/json)".into(),
                location: format!(
                    "aletheia-core — {} (tailnet) — the fleet's one controller; per-node Jenkins servers do not exist yet — the ranch nodes below are this controller's agents",
                    cfg.base
                ),
                freshness: format!(
                    "live — read from the API at request time{} · Jenkins {}",
                    reading_as
                        .as_ref()
                        .map(|n| format!(" · reading as {}", n))
                        .unwrap_or_default(),
                    version.clone().unwrap_or_else(|| "version not reported".into())
                ),
            },
            body: json!({
                "controller": {
                    "url": cfg.base,
                    "version": version,
                    "mode": root.get("mode").cloned().unwrap_or(Value::Null),
                    "quieting_down": root.get("quietingDown").and_then(|v| v.as_bool()).unwrap_or(false),
                    "reading_as": reading_as,
                },
                "counts": counts,
                "jobs": jobs,
                "agents_summary": agents_summary,
                "agents": agents,
                "queue": queue,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const ROOT: &str = include_str!("../tests/fixtures/jenkins-root.json");
    const COMPUTER: &str = include_str!("../tests/fixtures/jenkins-computer.json");
    const QUEUE: &str = include_str!("../tests/fixtures/jenkins-queue.json");

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"user:token"), "dXNlcjp0b2tlbg==");
    }

    #[test]
    fn fixture_jobs_parse_with_results() {
        // Fixtures captured from a real Jenkins 2.580.2 (scratch instance,
        // same version as the production controller).
        let root: Value = serde_json::from_str(ROOT).expect("fixture json");
        let (rows, counts) = jobs_body(&root, now_ms());
        assert_eq!(rows.len(), 4);
        let by_name = |n: &str| rows.iter().find(|r| r["name"] == n).expect("job row");
        assert_eq!(by_name("fixture-ok")["result"], "SUCCESS");
        assert_eq!(by_name("fixture-fail")["result"], "FAILURE");
        assert_eq!(by_name("fixture-never")["result"], "NOT_BUILT");
        assert_eq!(by_name("fixture-ok")["last_number"], 1);
        assert!(by_name("fixture-ok")["duration"].is_string());
        assert_eq!(counts["SUCCESS"], 1);
        assert_eq!(counts["FAILURE"], 1);
        assert_eq!(counts["NOT_BUILT"], 2);
    }

    #[test]
    fn fixture_agents_parse_with_offline() {
        let computer: Value = serde_json::from_str(COMPUTER).expect("fixture json");
        let (rows, summary) = agents_body(&computer);
        assert_eq!(rows.len(), 2);
        assert_eq!(summary["total"], 2);
        assert_eq!(summary["online"], 1);
        let agent = rows.iter().find(|r| r["name"] == "fixture-agent").expect("agent row");
        assert_eq!(agent["status"], "offline");
        assert_eq!(agent["executors"], 2);
    }

    #[test]
    fn fixture_queue_parses_and_why_is_text() {
        let queue: Value = serde_json::from_str(QUEUE).expect("fixture json");
        let body = queue_body(&queue, now_ms());
        assert_eq!(body["length"], 1);
        assert_eq!(body["items"][0]["job"], "fixture-slow");
        let why = body["items"][0]["why"].as_str().unwrap();
        assert!(!why.contains('<'), "why must be tag-free: {}", why);
    }

    #[test]
    fn strip_tags_removes_markup_and_collapses_space() {
        assert_eq!(
            strip_tags("Waiting for <a href=\"/computer/x/\">next executor</a>  now"),
            "Waiting for next executor now"
        );
        assert_eq!(strip_tags("plain"), "plain");
    }

    #[test]
    fn durations_and_ages_humanize() {
        assert_eq!(human_duration_ms(34), "0.0s");
        assert_eq!(human_duration_ms(12_300), "12s");
        assert_eq!(human_duration_ms(83_000), "1m 23s");
        assert_eq!(age_from_ms(0, 45_000), "45s ago");
        assert_eq!(age_from_ms(0, 3_900_000), "65m ago");
        assert_eq!(age_from_ms(0, 5_700_000), "1h 35m ago");
    }

    #[test]
    fn url_parse_accepts_host_port_and_rejects_https() {
        let (h, p, path) = parse_http_url("http://100.88.154.1:8080/api/json").expect("url");
        assert_eq!((h.as_str(), p, path.as_str()), ("100.88.154.1", 8080, "/api/json"));
        assert!(parse_http_url("https://example.invalid:8080/").is_err());
        assert!(parse_http_url("ftp://x/").is_err());
    }

    /// Serve one response on a loopback port, in the given framing.
    fn serve_once(framing: &str, body: &str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let framing = framing.to_string();
        let body = body.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let head = match framing.as_str() {
                "chunked" => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Jenkins: 2.580.2\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_string(),
                _ => format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Jenkins: 2.580.2\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()),
            };
            let _ = stream.write_all(head.as_bytes());
            if framing == "chunked" {
                let half = body.len() / 2;
                let wire = format!("{:x}\r\n{}\r\n{:x}\r\n{}\r\n0\r\n\r\n", half, &body[..half], body.len() - half, &body[half..]);
                let _ = stream.write_all(wire.as_bytes());
            } else {
                let _ = stream.write_all(body.as_bytes());
            }
        });
        port
    }

    #[test]
    fn http_client_reads_content_length_and_chunked() {
        for framing in ["length", "chunked"] {
            let port = serve_once(framing, "{\"mode\":\"NORMAL\"}");
            let resp = http_get(&format!("http://127.0.0.1:{}/api/json", port), None)
                .unwrap_or_else(|e| panic!("{} framing: {}", framing, e));
            assert_eq!(resp.status, 200);
            assert_eq!(resp.header("X-Jenkins"), Some("2.580.2"));
            let v: Value = serde_json::from_str(&resp.body).expect("json body");
            assert_eq!(v["mode"], "NORMAL");
        }
    }
}
