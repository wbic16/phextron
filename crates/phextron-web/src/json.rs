//! Minimal JSON parser — std only, zero dependencies.
//!
//! The web crate's whole job is turning the console's panel JSON (the exact
//! bytes `/api/panels/<id>` serves the desktop shell) into HTML. Pulling in
//! serde_json for that one parse would add serde + serde_json + itoa + ryu +
//! memchr to the browser build's lineage; a recursive-descent parser for the
//! JSON the console actually emits is ~200 lines of std. The precedent is
//! the Data panel's hand-rolled std-only SHA-256 (crates/phextron-dash/src/
//! data.rs), written for the same reason: the dependency lineage stays what
//! the house already carries, and no further.

/// A parsed JSON value. Objects keep insertion order (the console's
/// renderers iterate fixed key lists, so order is cosmetic, but fidelity
/// costs nothing). Numbers keep their raw lexeme for storage; display
/// renders them the way the desktop shell's JS does — through the numeric
/// value, JS-style (`8.0` shows as `8`) — and arithmetic parses on demand.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    /// True when the key is absent or explicitly null — the renderers treat
    /// both as "no value" (the JS shell's `v === null || v === undefined`).
    pub fn missing_or_null(&self, key: &str) -> bool {
        match self.get(key) {
            None => true,
            Some(v) => v.is_null(),
        }
    }

    /// Display form, mirroring what the desktop shell's JS produces when it
    /// stringifies a JSON value into HTML: strings as themselves, booleans
    /// as `true`/`false`, null as empty (callers substitute the em dash
    /// where the JS does). Numbers go through their f64 value, because the
    /// JS shell stringifies the parsed number, not the wire lexeme: the
    /// server emits `8.0`, JSON.parse makes it `8`, and the shell shows
    /// `8` — so the WASM renderer shows `8` too. (Rust's `{}` for f64 is
    /// the same shortest-round-trip form JS uses at panel magnitudes.)
    pub fn display(&self) -> String {
        match self {
            Json::Null => String::new(),
            Json::Bool(b) => b.to_string(),
            Json::Num(s) => match s.parse::<f64>() {
                Ok(v) => format!("{}", v),
                Err(_) => s.clone(),
            },
            Json::Str(s) => s.clone(),
            Json::Arr(_) | Json::Obj(_) => String::new(),
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, what: &str) -> String {
        format!("json parse error at byte {}: {}", self.pos, what)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => Err(self.err("unexpected character")),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.err("bad literal"))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'.' || b == b'e' || b == b'E' || b == b'+' || b == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err("bad number"));
        }
        let lexeme = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.err("number not utf8"))?
            .to_string();
        // Reject lexemes f64 cannot read (e.g. a lone "-"): the renderers
        // rely on Num always being arithmetically usable.
        if lexeme.parse::<f64>().is_err() {
            return Err(self.err("bad number"));
        }
        Ok(Json::Num(lexeme))
    }

    fn string(&mut self) -> Result<String, String> {
        // Caller has peeked the opening quote.
        self.pos += 1;
        let mut out = String::new();
        loop {
            let b = self.peek().ok_or_else(|| self.err("unterminated string"))?;
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.peek().ok_or_else(|| self.err("unterminated escape"))?;
                    self.pos += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            if (0xD800..0xDC00).contains(&hi) {
                                // High surrogate: a \uXXXX low surrogate must follow.
                                if self.peek() == Some(b'\\') {
                                    self.pos += 1;
                                    if self.peek() == Some(b'u') {
                                        self.pos += 1;
                                        let lo = self.hex4()?;
                                        let c = 0x10000
                                            + (((hi - 0xD800) as u32) << 10)
                                            + (lo as u32).saturating_sub(0xDC00);
                                        out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
                                        continue;
                                    }
                                    return Err(self.err("bad surrogate pair"));
                                }
                                return Err(self.err("lone high surrogate"));
                            }
                            out.push(char::from_u32(hi as u32).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
                _ => {
                    // Raw byte: JSON strings are UTF-8; copy the whole
                    // multi-byte sequence through untouched.
                    let start = self.pos - 1;
                    let len = utf8_len(b);
                    let end = start + len;
                    if end > self.bytes.len() {
                        return Err(self.err("truncated utf8 in string"));
                    }
                    match std::str::from_utf8(&self.bytes[start..end]) {
                        Ok(s) => out.push_str(s),
                        Err(_) => return Err(self.err("invalid utf8 in string")),
                    }
                    self.pos = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.err("truncated \\u escape"));
        }
        let s = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
            .map_err(|_| self.err("bad \\u escape"))?;
        let v = u16::from_str_radix(s, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn array(&mut self) -> Result<Json, String> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(self.err("expected ',' or ']' in array")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.pos += 1; // '{'
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected string key in object"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':' in object"));
            }
            self.pos += 1;
            let value = self.value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(pairs));
                }
                _ => return Err(self.err("expected ',' or '}' in object")),
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

/// Parse a complete JSON document; trailing non-whitespace is an error.
pub fn parse(input: &str) -> Result<Json, String> {
    let mut p = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    let v = p.value()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(p.err("trailing characters after document"));
    }
    Ok(v)
}
