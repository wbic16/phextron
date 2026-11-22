//! Phextron — the Exocortex console (stage 1).
//!
//! A std-first Rust server in the phext-lattice / markdown-wizard lineage:
//! no web framework, no dependency tree beyond libphext (the reference phext
//! parser) and serde_json. It serves one shell page and one JSON endpoint per
//! dashboard panel; every panel is fed by its own live-source reader (see
//! `panels.rs`). Dashboards read live sources directly — never rendered
//! human-facing pages as data plumbing.
//!
//! Serving-layer auth follows Will's console rulings (2026-11-21): Desktop
//! SSO for humans, MCP Auth for agents. Stage 1 binds localhost and carries
//! no auth of its own; the SSO/MCP gate lands with the console deployment.
//!
//! Config: PHEXTRON_PORT (default 8901), PHEXTRON_BIND (default 127.0.0.1),
//! plus the per-panel source paths documented in each reader module.

mod nodes;
mod panels;
mod pending;

use std::io::{Read, Write};
use std::net::TcpListener;

const SHELL: &str = include_str!("../web/shell.html");

fn security_headers() -> &'static str {
    "X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'\r\n"
}

fn respond(stream: &mut std::net::TcpStream, status: &str, ctype: &str, body: &str) {
    let out = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\n{}\r\n{}",
        status,
        ctype,
        body.len(),
        security_headers(),
        body
    );
    let _ = stream.write_all(out.as_bytes());
}

fn handle(mut stream: std::net::TcpStream) {
    let mut buf = vec![0u8; 64 * 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();

    let registry = panels::registry();
    match path.as_str() {
        "/" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", SHELL),
        "/health" => respond(&mut stream, "200 OK", "text/plain", "ok\n"),
        "/api/panels" => {
            let list: Vec<serde_json::Value> = registry
                .iter()
                .map(|p| {
                    let d = p.data();
                    serde_json::json!({
                        "id": d.id, "title": d.title, "state": d.state.as_str(),
                    })
                })
                .collect();
            respond(
                &mut stream,
                "200 OK",
                "application/json",
                &serde_json::json!({ "panels": list }).to_string(),
            );
        }
        _ => {
            if let Some(id) = path.strip_prefix("/api/panels/") {
                if let Some(panel) = registry.iter().find(|p| p.id() == id) {
                    let body = panel.data().to_json().to_string();
                    respond(&mut stream, "200 OK", "application/json", &body);
                    return;
                }
            }
            respond(&mut stream, "404 Not Found", "text/plain", "not found\n");
        }
    }
}

fn main() {
    let port: u16 = std::env::var("PHEXTRON_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8901);
    let bind = std::env::var("PHEXTRON_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
    let listener = TcpListener::bind(format!("{}:{}", bind, port)).expect("bind");
    println!("phextron listening on {}:{}", bind, port);
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                std::thread::spawn(move || handle(s));
            }
            Err(e) => eprintln!("accept error: {}", e),
        }
    }
}
