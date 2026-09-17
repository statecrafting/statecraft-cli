//! Strict portable-input scanning for JSON artifacts (spec 003 B-15; PKG-04).
//!
//! Refuses, as `ambiguous-json`, a duplicate member at any depth, a numeric
//! token that is not an integer, an integer outside the portable range,
//! invalid UTF-8 and a byte-order mark. It does not normalize: it answers
//! whether the bytes can be bound unambiguously, and nothing else.

use crate::value::PORTABLE_MAX;

/// Why the bytes are ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ambiguity {
    /// Not valid UTF-8.
    InvalidUtf8,
    /// Starts with a byte-order mark.
    ByteOrderMark,
    /// A member name repeated within one object, at any depth.
    DuplicateMember(String),
    /// A number that is not an integer token (fraction, exponent, `-0`).
    NonIntegerNumber(String),
    /// An integer outside `-(2^53-1)..2^53-1`.
    IntegerOutOfRange(String),
    /// Not JSON at all.
    Malformed(String),
}

/// Scan bytes. `Ok(())` means every conforming reader binds them the same way.
pub fn scan(bytes: &[u8]) -> Result<(), Ambiguity> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(Ambiguity::ByteOrderMark);
    }
    let s = std::str::from_utf8(bytes).map_err(|_| Ambiguity::InvalidUtf8)?;
    let mut p = Parser {
        s: s.as_bytes(),
        i: 0,
    };
    p.ws();
    p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(Ambiguity::Malformed("trailing content".into()));
    }
    Ok(())
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn expect(&mut self, b: u8) -> Result<(), Ambiguity> {
        if self.peek() == Some(b) {
            self.i += 1;
            Ok(())
        } else {
            Err(Ambiguity::Malformed(format!(
                "expected {:?} at {}",
                b as char, self.i
            )))
        }
    }
    fn value(&mut self) -> Result<(), Ambiguity> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(|_| ()),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-') | Some(b'0'..=b'9') => self.number(),
            _ => Err(Ambiguity::Malformed(format!(
                "unexpected byte at {}",
                self.i
            ))),
        }
    }
    fn literal(&mut self, lit: &[u8]) -> Result<(), Ambiguity> {
        if self.s[self.i..].starts_with(lit) {
            self.i += lit.len();
            Ok(())
        } else {
            Err(Ambiguity::Malformed(format!("bad literal at {}", self.i)))
        }
    }
    fn number(&mut self) -> Result<(), Ambiguity> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let digits_start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if self.i == digits_start {
            return Err(Ambiguity::Malformed("number without digits".into()));
        }
        if matches!(self.peek(), Some(b'.') | Some(b'e') | Some(b'E')) {
            // consume the rest of the token for the message
            while matches!(
                self.peek(),
                Some(b'.') | Some(b'e') | Some(b'E') | Some(b'+') | Some(b'-') | Some(b'0'..=b'9')
            ) {
                self.i += 1;
            }
            return Err(Ambiguity::NonIntegerNumber(
                String::from_utf8_lossy(&self.s[start..self.i]).into(),
            ));
        }
        let tok = std::str::from_utf8(&self.s[start..self.i]).unwrap();
        if tok == "-0" {
            return Err(Ambiguity::NonIntegerNumber(tok.into()));
        }
        let digits = &tok[usize::from(tok.starts_with('-'))..];
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(Ambiguity::Malformed("leading zero".into()));
        }
        match tok.parse::<i64>() {
            Ok(v) if (-PORTABLE_MAX..=PORTABLE_MAX).contains(&v) => Ok(()),
            _ => Err(Ambiguity::IntegerOutOfRange(tok.into())),
        }
    }
    fn string(&mut self) -> Result<String, Ambiguity> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return Err(Ambiguity::Malformed("unterminated string".into())),
                Some(b'"') => {
                    self.i += 1;
                    break;
                }
                Some(b'\\') => {
                    self.i += 1;
                    match self.peek() {
                        Some(b'u') => {
                            self.i += 1;
                            let h = self
                                .s
                                .get(self.i..self.i + 4)
                                .ok_or_else(|| Ambiguity::Malformed("short escape".into()))?;
                            let cp = u32::from_str_radix(
                                std::str::from_utf8(h)
                                    .map_err(|_| Ambiguity::Malformed("escape".into()))?,
                                16,
                            )
                            .map_err(|_| Ambiguity::Malformed("escape".into()))?;
                            self.i += 4;
                            out.extend_from_slice(format!("\\u{cp:04x}").as_bytes());
                        }
                        Some(c) => {
                            self.i += 1;
                            out.push(b'\\');
                            out.push(c);
                        }
                        None => return Err(Ambiguity::Malformed("unterminated escape".into())),
                    }
                }
                Some(c) => {
                    if c < 0x20 {
                        return Err(Ambiguity::Malformed("control character in string".into()));
                    }
                    out.push(c);
                    self.i += 1;
                }
            }
        }
        Ok(String::from_utf8_lossy(&out).into())
    }
    fn array(&mut self) -> Result<(), Ambiguity> {
        self.expect(b'[')?;
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(());
        }
        loop {
            self.ws();
            self.value()?;
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return Err(Ambiguity::Malformed("bad array".into())),
            }
        }
    }
    fn object(&mut self) -> Result<(), Ambiguity> {
        self.expect(b'{')?;
        self.ws();
        let mut seen = std::collections::BTreeSet::new();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(());
        }
        loop {
            self.ws();
            let k = self.string()?;
            if !seen.insert(k.clone()) {
                return Err(Ambiguity::DuplicateMember(k));
            }
            self.ws();
            self.expect(b':')?;
            self.ws();
            self.value()?;
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return Err(Ambiguity::Malformed("bad object".into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicates_at_depth_are_refused() {
        assert_eq!(
            scan(br#"{"a":{"b":1,"b":2}}"#),
            Err(Ambiguity::DuplicateMember("b".into()))
        );
        assert_eq!(
            scan(br#"{"a":1,"b":[{"c":1,"c":1}]}"#),
            Err(Ambiguity::DuplicateMember("c".into()))
        );
    }

    #[test]
    fn numbers_that_cannot_be_bound_are_refused() {
        assert!(matches!(scan(b"1.5"), Err(Ambiguity::NonIntegerNumber(_))));
        assert!(matches!(scan(b"1e3"), Err(Ambiguity::NonIntegerNumber(_))));
        assert!(matches!(scan(b"-0"), Err(Ambiguity::NonIntegerNumber(_))));
        assert!(matches!(
            scan(b"9007199254740992"),
            Err(Ambiguity::IntegerOutOfRange(_))
        ));
        assert_eq!(scan(b"9007199254740991"), Ok(()));
        assert_eq!(scan(b"-9007199254740991"), Ok(()));
    }

    #[test]
    fn bom_and_invalid_utf8_are_refused() {
        assert_eq!(scan(b"\xef\xbb\xbf{}"), Err(Ambiguity::ByteOrderMark));
        assert_eq!(scan(b"\"\xff\""), Err(Ambiguity::InvalidUtf8));
    }

    #[test]
    fn whitespace_and_order_variants_scan_clean_and_are_not_normalized() {
        assert_eq!(scan(b"{ \"b\" : 1 , \"a\" : [ true , null ] }"), Ok(()));
    }
}
