use gl1_http::LatticeClient;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

fn serve_once(handler: impl Fn(&str) -> (u16, String) + Send + 'static) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let cap = captured.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut s = match stream { Ok(s) => s, Err(_) => continue };
            let mut reader = BufReader::new(s.try_clone().unwrap());
            let mut request = String::new();
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 { break; }
                if line.to_ascii_lowercase().starts_with("content-length:") {
                    content_length = line.split(':').nth(1).map(|v| v.trim().parse().unwrap_or(0)).unwrap_or(0);
                }
                let done = line == "\r\n";
                request.push_str(&line);
                if done { break; }
            }
            if content_length > 0 {
                let mut buf = vec![0u8; content_length];
                reader.read_exact(&mut buf).ok();
                request.push_str(&String::from_utf8_lossy(&buf));
            }
            cap.lock().unwrap().push(request.clone());
            let (status, body) = handler(&request);
            let resp = format!("HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            s.write_all(resp.as_bytes()).ok();
        }
    });
    (format!("http://{addr}"), captured)
}

#[test]
fn get_post_exchange_shapes() {
    let (base, captured) = serve_once(|req| {
        if req.starts_with("POST /api/auth/desktop/exchange") {
            (200, r#"{"session":"sess1","csrf":"csrf1"}"#.to_string())
        } else if req.starts_with("GET /api/session") {
            (200, r#"{"signed_in":true}"#.to_string())
        } else {
            (200, r#"{"ok":true}"#.to_string())
        }
    });
    let mut client = LatticeClient::new(base.clone());
    let (status, parsed) = client.exchange("assert-xyz", "state-123").unwrap();
    assert_eq!(status, 200);
    assert_eq!(parsed.session, "sess1");
    assert_eq!(parsed.csrf, "csrf1");

    let (status, body) = client.session().unwrap();
    assert_eq!(status, 200);
    assert!(body.contains("signed_in"));

    let (status, _) = client.post("/api/update", "{}").unwrap();
    assert_eq!(status, 200);

    let reqs = captured.lock().unwrap();
    assert!(reqs[0].to_ascii_lowercase().contains("x-phext-assertion: assert-xyz"), "exchange assertion header: {}", reqs[0]);
    assert!(reqs[0].to_ascii_lowercase().contains("x-phext-state: state-123"), "exchange state header: {}", reqs[0]);
    assert!(reqs[1].to_ascii_lowercase().contains("cookie: phext_session=sess1"), "session cookie: {}", reqs[1]);
    assert!(reqs[2].to_ascii_lowercase().contains("x-csrf-token: csrf1"), "csrf header: {}", reqs[2]);
    assert!(reqs[2].to_ascii_lowercase().contains("cookie: phext_session=sess1"), "post cookie: {}", reqs[2]);
}
