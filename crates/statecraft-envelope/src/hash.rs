//! Native identity: BLAKE3-256 hashes, content identifiers, key identifiers
//! (hqgit 010, 013). Serialized as lowercase hex in JSON; as bytes in CBOR.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Error;

/// A BLAKE3-256 hash.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Hash bytes.
    pub fn of(bytes: &[u8]) -> Self {
        Hash(*blake3::hash(bytes).as_bytes())
    }

    /// Lowercase hex.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// From lowercase hex; uppercase and wrong lengths are refused.
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        if s.len() != 64 || s.bytes().any(|b| b.is_ascii_uppercase()) {
            return Err(Error::Validation(format!(
                "not a lowercase 64-hex hash: {s:?}"
            )));
        }
        let v = hex::decode(s).map_err(|e| Error::Validation(format!("hex: {e}")))?;
        let mut a = [0u8; 32];
        a.copy_from_slice(&v);
        Ok(Hash(a))
    }

    /// From exactly 32 bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        if b.len() != 32 {
            return Err(Error::Validation(format!(
                "hash is {} bytes, not 32",
                b.len()
            )));
        }
        let mut a = [0u8; 32];
        a.copy_from_slice(b);
        Ok(Hash(a))
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", &self.to_hex()[..16])
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Hash::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// The codec of a stored object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    /// Opaque bytes: an artifact, or an entry's canonical bytes stored as such.
    Raw,
    /// Canonical DAG-CBOR.
    DagCbor,
}

impl Codec {
    /// The wire word.
    pub fn as_str(self) -> &'static str {
        match self {
            Codec::Raw => "raw",
            Codec::DagCbor => "dag-cbor",
        }
    }
    fn multicodec(self) -> u8 {
        match self {
            Codec::Raw => 0x55,
            Codec::DagCbor => 0x71,
        }
    }
}

/// A content identifier: codec plus hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cid {
    /// How the bytes are to be read.
    pub codec: Codec,
    /// BLAKE3-256 over the stored bytes.
    pub hash: Hash,
}

const MULTIHASH_BLAKE3: u8 = 0x1e;

impl Cid {
    /// The identifier of raw bytes.
    pub fn raw(bytes: &[u8]) -> Self {
        Cid {
            codec: Codec::Raw,
            hash: Hash::of(bytes),
        }
    }

    /// CIDv1 binary form with the identity multibase prefix, as the link tag
    /// carries it: `0x00 0x01 <codec> 0x1e 0x20 <32 bytes>`.
    pub fn to_binary(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(37);
        b.push(0x00);
        b.push(0x01);
        b.push(self.codec.multicodec());
        b.push(MULTIHASH_BLAKE3);
        b.push(32);
        b.extend_from_slice(&self.hash.0);
        b
    }

    /// Parse the binary form; anything but a BLAKE3-256 CIDv1 of a known codec is refused.
    pub fn from_binary(b: &[u8]) -> Result<Self, Error> {
        if b.len() != 37 || b[0] != 0x00 || b[1] != 0x01 || b[3] != MULTIHASH_BLAKE3 || b[4] != 32 {
            return Err(Error::Decode("not a BLAKE3-256 CIDv1 link".into()));
        }
        let codec = match b[2] {
            0x55 => Codec::Raw,
            0x71 => Codec::DagCbor,
            other => return Err(Error::Decode(format!("unknown codec 0x{other:02x}"))),
        };
        Ok(Cid {
            codec,
            hash: Hash::from_bytes(&b[5..])?,
        })
    }

    /// `<codec>:<hex>`, the storage key form.
    pub fn to_string_key(&self) -> String {
        format!("{}:{}", self.codec.as_str(), self.hash.to_hex())
    }
}

/// The identifier of a public key: the hash of its 32 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KeyId(pub Hash);

impl KeyId {
    /// Of a public key's bytes.
    pub fn of(public_key: &[u8; 32]) -> Self {
        KeyId(Hash::of(public_key))
    }
}

/// A SHA-256 digest as lowercase hex, the foreign-artifact digest (G-07).
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes))
}
