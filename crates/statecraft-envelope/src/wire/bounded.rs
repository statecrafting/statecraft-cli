//! The bounded reader every wire-exchange input passes before it means
//! anything: spec 014 section 3.1.
//!
//! The limits are enforced **while tokenizing**, so a hostile input never
//! becomes an unbounded value first. A duplicate member name at any depth is
//! refused rather than resolved, because two readers that pick different
//! copies would judge different artifacts. Nothing here follows a path, a
//! link, a URL or a command found in the input: a string is only a string.
//!
//! Numbers keep their exact token text. A wire-exchange manifest carries
//! provider-reported values whose meaning is the producer's, so nothing is
//! converted, rounded or re-spelled on the way in.

use std::collections::BTreeMap;

/// At most this many manifest bytes.
pub const MAX_BYTES: usize = 1_048_576;
/// At most this many nested object or array levels.
pub const MAX_DEPTH: usize = 32;
/// At most this many object members plus array elements across the input.
pub const MAX_ITEMS: usize = 4_096;
/// At most this many elements in one array.
pub const MAX_ARRAY: usize = 1_024;
/// At most this many UTF-8 bytes in one member name.
pub const MAX_NAME: usize = 128;
/// At most this many UTF-8 bytes in one string value.
pub const MAX_STRING: usize = 65_536;

/// A decoded value. Objects are ordered by member name, which is the order
/// the producer writes them in and the order every walk reports them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, as its exact token text.
    Number(String),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object; no name occurs twice.
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// The member `name`, when this is an object that has it.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Json> {
        match self {
            Json::Object(members) => members.get(name),
            _ => None,
        }
    }

    /// The string, when this is one.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// The value as a non-negative integer, when its token is one.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Number(text) if text.bytes().all(|b| b.is_ascii_digit()) => text.parse().ok(),
            _ => None,
        }
    }
}

/// Why the input was refused. Every variant is reported as `malformed`; the
/// variant says which bound or rule, and never carries the input's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// More than [`MAX_BYTES`].
    TooLarge,
    /// More than [`MAX_DEPTH`] levels.
    TooDeep,
    /// More than [`MAX_ITEMS`] members plus elements.
    TooManyItems,
    /// More than [`MAX_ARRAY`] elements in one array.
    ArrayTooLong,
    /// A member name over [`MAX_NAME`] bytes.
    NameTooLong,
    /// A string value over [`MAX_STRING`] bytes.
    StringTooLong,
    /// A member name repeated within one object.
    DuplicateMember,
    /// Not valid UTF-8, a byte-order mark, or not JSON.
    Syntax,
}

impl Refusal {
    /// The word a finding records.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Refusal::TooLarge => "too-large",
            Refusal::TooDeep => "too-deep",
            Refusal::TooManyItems => "too-many-items",
            Refusal::ArrayTooLong => "array-too-long",
            Refusal::NameTooLong => "name-too-long",
            Refusal::StringTooLong => "string-too-long",
            Refusal::DuplicateMember => "duplicate-member",
            Refusal::Syntax => "syntax",
        }
    }
}

/// Decode `bytes` under every section 3.1 bound.
///
/// # Errors
///
/// The first bound or rule the input breaks.
pub fn decode(bytes: &[u8]) -> Result<Json, Refusal> {
    if bytes.len() > MAX_BYTES {
        return Err(Refusal::TooLarge);
    }
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(Refusal::Syntax);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Syntax)?;
    let mut reader = Reader {
        s: text.as_bytes(),
        i: 0,
        items: 0,
    };
    reader.ws();
    let value = reader.value(0)?;
    reader.ws();
    if reader.i != reader.s.len() {
        return Err(Refusal::Syntax);
    }
    Ok(value)
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
    items: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> Result<(), Refusal> {
        if self.peek() == Some(b) {
            self.i += 1;
            Ok(())
        } else {
            Err(Refusal::Syntax)
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), Refusal> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(())
        } else {
            Err(Refusal::Syntax)
        }
    }

    fn count(&mut self) -> Result<(), Refusal> {
        self.items += 1;
        if self.items > MAX_ITEMS {
            Err(Refusal::TooManyItems)
        } else {
            Ok(())
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, Refusal> {
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => Ok(Json::String(
                self.string(MAX_STRING, Refusal::StringTooLong)?,
            )),
            Some(b't') => self.literal(b"true").map(|()| Json::Bool(true)),
            Some(b'f') => self.literal(b"false").map(|()| Json::Bool(false)),
            Some(b'n') => self.literal(b"null").map(|()| Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(Refusal::Syntax),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, Refusal> {
        if depth > MAX_DEPTH {
            return Err(Refusal::TooDeep);
        }
        self.eat(b'{')?;
        let mut members = BTreeMap::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.ws();
            self.count()?;
            let name = self.string(MAX_NAME, Refusal::NameTooLong)?;
            self.ws();
            self.eat(b':')?;
            self.ws();
            let value = self.value(depth)?;
            if members.insert(name, value).is_some() {
                return Err(Refusal::DuplicateMember);
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Object(members));
                }
                _ => return Err(Refusal::Syntax),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, Refusal> {
        if depth > MAX_DEPTH {
            return Err(Refusal::TooDeep);
        }
        self.eat(b'[')?;
        let mut elements = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Array(elements));
        }
        loop {
            self.ws();
            self.count()?;
            if elements.len() == MAX_ARRAY {
                return Err(Refusal::ArrayTooLong);
            }
            elements.push(self.value(depth)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Array(elements));
                }
                _ => return Err(Refusal::Syntax),
            }
        }
    }

    /// A string, refused with `too_long` once its decoded bytes pass `limit`.
    fn string(&mut self, limit: usize, too_long: Refusal) -> Result<String, Refusal> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(Refusal::Syntax);
            };
            self.i += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(Refusal::Syntax);
                    };
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let c = self.escaped_char()?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(Refusal::Syntax),
                    }
                }
                0x00..=0x1f => return Err(Refusal::Syntax),
                _ => out.push(b),
            }
            if out.len() > limit {
                return Err(too_long);
            }
        }
        // The input was valid UTF-8 and every escape encodes a scalar value.
        String::from_utf8(out).map_err(|_| Refusal::Syntax)
    }

    fn hex4(&mut self) -> Result<u32, Refusal> {
        let digits = self.s.get(self.i..self.i + 4).ok_or(Refusal::Syntax)?;
        let text = std::str::from_utf8(digits).map_err(|_| Refusal::Syntax)?;
        if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Refusal::Syntax);
        }
        self.i += 4;
        u32::from_str_radix(text, 16).map_err(|_| Refusal::Syntax)
    }

    /// The scalar after `\u`, joining a surrogate pair; a lone surrogate is
    /// refused rather than replaced.
    fn escaped_char(&mut self) -> Result<char, Refusal> {
        let high = self.hex4()?;
        let code = if (0xd800..0xdc00).contains(&high) {
            self.literal(b"\\u")?;
            let low = self.hex4()?;
            if !(0xdc00..0xe000).contains(&low) {
                return Err(Refusal::Syntax);
            }
            0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
        } else {
            high
        };
        char::from_u32(code).ok_or(Refusal::Syntax)
    }

    fn number(&mut self) -> Result<Json, Refusal> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(Refusal::Syntax),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            self.required_digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            self.required_digits()?;
        }
        if self.i - start > MAX_NAME {
            return Err(Refusal::Syntax);
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| Refusal::Syntax)?;
        Ok(Json::Number(text.to_string()))
    }

    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
    }

    fn required_digits(&mut self) -> Result<(), Refusal> {
        let before = self.i;
        self.digits();
        if self.i == before {
            Err(Refusal::Syntax)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surrogate_pair_joins_and_a_lone_one_is_refused() {
        assert_eq!(
            decode(b"\"\\ud83d\\ude00\"").unwrap(),
            Json::String("\u{1f600}".into())
        );
        assert_eq!(decode(br#""\ud83d""#), Err(Refusal::Syntax));
    }

    #[test]
    fn numbers_keep_their_token_text() {
        assert_eq!(decode(b"1.50e3").unwrap(), Json::Number("1.50e3".into()));
        assert_eq!(decode(b"01"), Err(Refusal::Syntax));
        assert_eq!(decode(b"1."), Err(Refusal::Syntax));
    }

    #[test]
    fn trailing_content_and_a_bom_are_refused() {
        assert_eq!(decode(b"{} {}"), Err(Refusal::Syntax));
        assert_eq!(decode(b"\xef\xbb\xbf{}"), Err(Refusal::Syntax));
    }
}
