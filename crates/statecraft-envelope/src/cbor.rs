//! Canonical DAG-CBOR (hqgit 011): RFC 8949 deterministic encoding under the
//! IPLD DAG-CBOR restrictions. Definite lengths only, shortest integer
//! encodings, map keys sorted length-first then bytewise, no floats, no
//! indefinite items, no tag but the link tag (42). Decode of encode is the
//! identity, and encode of decode is the identity on canonical bytes: a
//! decoder that meets non-canonical bytes refuses them rather than
//! normalizing, because normalization would make two byte strings share a
//! hash.

use std::collections::BTreeMap;

use crate::Error;
use crate::hash::{Cid, Codec, Hash};
use crate::value::{PORTABLE_MAX, Value};

const MAJOR_UINT: u8 = 0;
const MAJOR_NINT: u8 = 1;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_ARRAY: u8 = 4;
const MAJOR_MAP: u8 = 5;
const MAJOR_TAG: u8 = 6;
const MAJOR_SIMPLE: u8 = 7;
const LINK_TAG: u64 = 42;

/// Encode a value to its canonical bytes.
pub fn encode(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(v, &mut out);
    out
}

fn head(major: u8, n: u64, out: &mut Vec<u8>) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.push(m | 24);
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

/// Canonical key order: shorter keys first, then bytewise.
fn key_order(a: &str, b: &str) -> std::cmp::Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
}

fn encode_into(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.push((MAJOR_SIMPLE << 5) | 22),
        Value::Bool(false) => out.push((MAJOR_SIMPLE << 5) | 20),
        Value::Bool(true) => out.push((MAJOR_SIMPLE << 5) | 21),
        Value::Int(i) => {
            if *i >= 0 {
                head(MAJOR_UINT, *i as u64, out);
            } else {
                head(MAJOR_NINT, (-1 - *i) as u64, out);
            }
        }
        Value::Bytes(b) => {
            head(MAJOR_BYTES, b.len() as u64, out);
            out.extend_from_slice(b);
        }
        Value::Text(t) => {
            head(MAJOR_TEXT, t.len() as u64, out);
            out.extend_from_slice(t.as_bytes());
        }
        Value::Array(a) => {
            head(MAJOR_ARRAY, a.len() as u64, out);
            for x in a {
                encode_into(x, out);
            }
        }
        Value::Map(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| key_order(a, b));
            head(MAJOR_MAP, keys.len() as u64, out);
            for k in keys {
                head(MAJOR_TEXT, k.len() as u64, out);
                out.extend_from_slice(k.as_bytes());
                encode_into(&m[k], out);
            }
        }
        Value::Link(c) => {
            head(MAJOR_TAG, LINK_TAG, out);
            let b = c.to_binary();
            head(MAJOR_BYTES, b.len() as u64, out);
            out.extend_from_slice(&b);
        }
    }
}

/// Decode canonical bytes. Non-canonical input is an error, never normalized.
pub fn decode(bytes: &[u8]) -> Result<Value, Error> {
    let mut d = Decoder { b: bytes, i: 0 };
    let v = d.value()?;
    if d.i != bytes.len() {
        return Err(Error::Decode("trailing bytes after the value".into()));
    }
    Ok(v)
}

struct Decoder<'a> {
    b: &'a [u8],
    i: usize,
}

impl Decoder<'_> {
    fn byte(&mut self) -> Result<u8, Error> {
        let x = *self
            .b
            .get(self.i)
            .ok_or_else(|| Error::Decode("unexpected end".into()))?;
        self.i += 1;
        Ok(x)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        let end = self
            .i
            .checked_add(n)
            .ok_or_else(|| Error::Decode("length overflow".into()))?;
        if end > self.b.len() {
            return Err(Error::Decode("unexpected end".into()));
        }
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }

    /// Read a head; refuse a non-shortest encoding and indefinite lengths.
    fn head(&mut self) -> Result<(u8, u64), Error> {
        let first = self.byte()?;
        let major = first >> 5;
        let ai = first & 0x1f;
        let n = match ai {
            0..=23 => ai as u64,
            24 => {
                let v = self.byte()? as u64;
                if v < 24 {
                    return Err(Error::Decode("non-shortest integer encoding".into()));
                }
                v
            }
            25 => {
                let v = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as u64;
                if v <= 0xff {
                    return Err(Error::Decode("non-shortest integer encoding".into()));
                }
                v
            }
            26 => {
                let v = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as u64;
                if v <= 0xffff {
                    return Err(Error::Decode("non-shortest integer encoding".into()));
                }
                v
            }
            27 => {
                let v = u64::from_be_bytes(self.take(8)?.try_into().unwrap());
                if v <= 0xffff_ffff {
                    return Err(Error::Decode("non-shortest integer encoding".into()));
                }
                v
            }
            31 => return Err(Error::Decode("indefinite length is not canonical".into())),
            _ => return Err(Error::Decode("reserved additional information".into())),
        };
        Ok((major, n))
    }

    fn value(&mut self) -> Result<Value, Error> {
        let (major, n) = self.head()?;
        match major {
            MAJOR_UINT => {
                if n > PORTABLE_MAX as u64 {
                    return Err(Error::Decode("integer outside the portable range".into()));
                }
                Ok(Value::Int(n as i64))
            }
            MAJOR_NINT => {
                if n > (PORTABLE_MAX as u64) - 1 {
                    return Err(Error::Decode("integer outside the portable range".into()));
                }
                Ok(Value::Int(-1 - n as i64))
            }
            MAJOR_BYTES => Ok(Value::Bytes(self.take(n as usize)?.to_vec())),
            MAJOR_TEXT => {
                let s = self.take(n as usize)?;
                let t = std::str::from_utf8(s)
                    .map_err(|_| Error::Decode("invalid UTF-8 in text".into()))?;
                Ok(Value::Text(t.to_string()))
            }
            MAJOR_ARRAY => {
                let mut a = Vec::with_capacity(n.min(1024) as usize);
                for _ in 0..n {
                    a.push(self.value()?);
                }
                Ok(Value::Array(a))
            }
            MAJOR_MAP => {
                let mut m = BTreeMap::new();
                let mut prev: Option<String> = None;
                for _ in 0..n {
                    let (km, kn) = self.head()?;
                    if km != MAJOR_TEXT {
                        return Err(Error::Decode("map key is not text".into()));
                    }
                    let ks = self.take(kn as usize)?;
                    let k = std::str::from_utf8(ks)
                        .map_err(|_| Error::Decode("invalid UTF-8 in key".into()))?
                        .to_string();
                    if let Some(p) = &prev {
                        match key_order(p, &k) {
                            std::cmp::Ordering::Less => {}
                            std::cmp::Ordering::Equal => {
                                return Err(Error::Decode(format!("duplicate key {k:?}")));
                            }
                            std::cmp::Ordering::Greater => {
                                return Err(Error::Decode(
                                    "map keys not in canonical order".into(),
                                ));
                            }
                        }
                    }
                    let v = self.value()?;
                    prev = Some(k.clone());
                    m.insert(k, v);
                }
                Ok(Value::Map(m))
            }
            MAJOR_TAG => {
                if n != LINK_TAG {
                    return Err(Error::Decode(format!("tag {n} is not the link tag")));
                }
                let (bm, bn) = self.head()?;
                if bm != MAJOR_BYTES {
                    return Err(Error::Decode("link tag content is not bytes".into()));
                }
                let b = self.take(bn as usize)?;
                Ok(Value::Link(Cid::from_binary(b)?))
            }
            MAJOR_SIMPLE => match n {
                20 => Ok(Value::Bool(false)),
                21 => Ok(Value::Bool(true)),
                22 => Ok(Value::Null),
                25..=27 => Err(Error::Decode("floats are not permitted".into())),
                _ => Err(Error::Decode(format!("simple value {n} is not permitted"))),
            },
            _ => Err(Error::Decode("unknown major type".into())),
        }
    }
}

/// Hash canonical bytes of a value: the identity of a `DagCbor` object.
pub fn cid_of(v: &Value) -> Cid {
    Cid {
        codec: Codec::DagCbor,
        hash: Hash::of(&encode(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_sort_length_first_then_bytewise() {
        let v = Value::map()
            .with("b", Value::Int(1))
            .unwrap()
            .with("aa", Value::Int(2))
            .unwrap()
            .with("a", Value::Int(3))
            .unwrap();
        let b = encode(&v);
        // a3 (map of 3), then "a", "b", "aa"
        assert_eq!(
            b,
            vec![0xa3, 0x61, b'a', 3, 0x61, b'b', 1, 0x62, b'a', b'a', 2]
        );
        assert_eq!(decode(&b).unwrap(), v);
    }

    #[test]
    fn non_shortest_integer_is_refused() {
        assert!(decode(&[0x18, 0x05]).is_err());
        assert_eq!(decode(&[0x05]).unwrap(), Value::Int(5));
    }

    #[test]
    fn floats_and_indefinite_items_are_refused() {
        assert!(decode(&[0xfb, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(decode(&[0x9f, 0xff]).is_err());
    }

    #[test]
    fn duplicate_and_misordered_keys_are_refused() {
        assert!(decode(&[0xa2, 0x61, b'a', 1, 0x61, b'a', 2]).is_err());
        assert!(decode(&[0xa2, 0x61, b'b', 1, 0x61, b'a', 2]).is_err());
    }

    #[test]
    fn negative_integers_round_trip() {
        for i in [-1i64, -24, -25, -256, -257, -PORTABLE_MAX] {
            let b = encode(&Value::Int(i));
            assert_eq!(decode(&b).unwrap(), Value::Int(i));
        }
    }
}
