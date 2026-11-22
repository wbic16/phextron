//! G-L1 — lineage HTTP+TLS client for phext-lattice.
//!
//! Mirrors phext-lattice's own dependency choices (SPEC §6.1):
//! `ureq` with `default-features = false` and the `tls` feature
//! (rustls), plus `serde`/`serde_json` which are already in
//! lattice's tree. JSON appears only at this boundary, because
//! lattice already speaks it there (`/api/session`, the desktop
//! exchange). No other dependencies.

use serde::Deserialize;

/// Join a base URL and a path with exactly one separating slash.
pub fn join_url(base: &str, path: &str) -> String {
    let b = base.trim_end_matches('/');
    let p = path.strip_prefix('/').unwrap_or(path);
    if p.is_empty() {
        b.to_string()
    } else {
        format!("{b}/{p}")
    }
}

/// Value of the `Cookie` header presenting the session, if any.
pub fn cookie_header(session: Option<&str>) -> Option<String> {
    session.map(|s| format!("phext_session={s}"))
}

/// The `{session, csrf}` pair returned by the desktop exchange.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExchangeResult {
    pub session: String,
    #[serde(default)]
    pub csrf: String,
}

/// Typed client for one lattice base URL.
#[derive(Debug, Clone)]
pub struct LatticeClient {
    pub base_url: String,
    session: Option<String>,
    csrf: Option<String>,
    agent: ureq::Agent,
}

impl LatticeClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            session: None,
            csrf: None,
            agent: ureq::AgentBuilder::new().build(),
        }
    }

    pub fn with_credentials(
        base_url: impl Into<String>,
        session: Option<String>,
        csrf: Option<String>,
    ) -> Self {
        let mut c = Self::new(base_url);
        c.session = session;
        c.csrf = csrf;
        c
    }

    pub fn set_session(&mut self, session: Option<String>) {
        self.session = session;
    }
    pub fn set_csrf(&mut self, csrf: Option<String>) {
        self.csrf = csrf;
    }
    pub fn session_value(&self) -> Option<&str> {
        self.session.as_deref()
    }
    pub fn csrf_value(&self) -> Option<&str> {
        self.csrf.as_deref()
    }
    pub fn url(&self, path: &str) -> String {
        join_url(&self.base_url, path)
    }

    fn apply_read_headers<'a>(&self, req: ureq::Request) -> ureq::Request {
        let mut r = req;
        if let Some(c) = cookie_header(self.session.as_deref()) {
            r = r.set("Cookie", &c);
        }
        r
    }

    fn apply_write_headers(&self, req: ureq::Request) -> ureq::Request {
        let mut r = self.apply_read_headers(req);
        if let Some(csrf) = &self.csrf {
            r = r.set("x-csrf-token", csrf);
        }
        r
    }

    /// Generic GET. Returns `(status, body)`. HTTP error statuses are
    /// returned, not raised, so callers see lattice's status directly.
    pub fn get(&self, path: &str) -> Result<(u16, String), ureq::Error> {
        let req = self.apply_read_headers(self.agent.get(&self.url(path)));
        match req.call() {
            Ok(resp) => {
                let status = resp.status();
                Ok((status, resp.into_string().unwrap_or_default()))
            }
            Err(ureq::Error::Status(status, resp)) => {
                Ok((status, resp.into_string().unwrap_or_default()))
            }
            Err(e) => Err(e),
        }
    }

    /// Generic POST with a string body. Sends the CSRF token.
    pub fn post(&self, path: &str, body: &str) -> Result<(u16, String), ureq::Error> {
        let req = self
            .apply_write_headers(self.agent.post(&self.url(path)))
            .set("Content-Type", "application/json");
        match req.send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                Ok((status, resp.into_string().unwrap_or_default()))
            }
            Err(ureq::Error::Status(status, resp)) => {
                Ok((status, resp.into_string().unwrap_or_default()))
            }
            Err(e) => Err(e),
        }
    }

    /// POST /api/auth/desktop/exchange with the assertion/state headers.
    /// On success stores the returned session + CSRF on this client.
    pub fn exchange(
        &mut self,
        assertion: &str,
        state: &str,
    ) -> Result<(u16, ExchangeResult), Box<dyn std::error::Error>> {
        let req = self
            .agent
            .post(&self.url("/api/auth/desktop/exchange"))
            .set("x-phext-assertion", assertion)
            .set("x-phext-state", state);
        let (status, body) = match req.send_string("") {
            Ok(resp) => {
                let s = resp.status();
                (s, resp.into_string().unwrap_or_default())
            }
            Err(ureq::Error::Status(s, resp)) => {
                (s, resp.into_string().unwrap_or_default())
            }
            Err(e) => return Err(Box::new(e)),
        };
        let parsed: ExchangeResult = serde_json::from_str(&body)
            .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;
        if status >= 200 && status < 300 {
            self.session = Some(parsed.session.clone());
            self.csrf = Some(parsed.csrf.clone());
        }
        Ok((status, parsed))
    }

    /// GET /api/session. Returns `(status, body)`.
    pub fn session(&self) -> Result<(u16, String), ureq::Error> {
        self.get("/api/session")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_joining() {
        assert_eq!(join_url("https://x.test", "/api/session"), "https://x.test/api/session");
        assert_eq!(join_url("https://x.test/", "/api/session"), "https://x.test/api/session");
        assert_eq!(join_url("https://x.test/", "api/session"), "https://x.test/api/session");
        assert_eq!(join_url("https://x.test", ""), "https://x.test");
        assert_eq!(join_url("http://127.0.0.1:9/", "/a/b"), "http://127.0.0.1:9/a/b");
    }

    #[test]
    fn cookie_construction() {
        assert_eq!(cookie_header(Some("abc123")), Some("phext_session=abc123".to_string()));
        assert_eq!(cookie_header(None), None);
    }

    #[test]
    fn exchange_json_shape() {
        let v: ExchangeResult =
            serde_json::from_str(r#"{"session":"s1","csrf":"c1"}"#).unwrap();
        assert_eq!(v.session, "s1");
        assert_eq!(v.csrf, "c1");
    }

    #[test]
    fn client_url_uses_base() {
        let c = LatticeClient::new("https://edit.phext.io/");
        assert_eq!(c.url("/api/session"), "https://edit.phext.io/api/session");
    }
}
