//! The three names for absence, and the wire contract that makes an untagged
//! `Recorded<T>` unambiguous.
//!
//! The vocabulary is statecraft-cli spec 005 section 3.8: absence is one of
//! three named things, and **none of them reads as success**. Spec 007 adds
//! what 005 left open, because two products now read the same bytes.
//!
//! # The ambiguity, stated
//!
//! `Recorded<T>` is untagged: a recorded value is written as itself, and an
//! absence as one of the three words. For `T = String` the two overlap. The
//! byte string `"not-recorded"` is what `Absent(NotRecorded)` writes, and it
//! is also what `Present("not-recorded".into())` writes. Nothing in the bytes
//! distinguishes them, so a reader must guess, and two readers guessing
//! differently disagree about a receipt without either of them being wrong.
//!
//! **Declaring one variant before the other does not fix this.** Serde's
//! untagged deserializer tries the variants in order, so the declaration order
//! picks which reading wins; it does not make the other reading unreachable,
//! and it does not stop a writer from emitting the colliding bytes in the
//! first place. It moves the disagreement, it does not end it.
//!
//! # The contract
//!
//! The three words are **reserved** for every `Recorded<T>`, whatever `T` is:
//!
//! 1. On the wire, `"none"`, `"not-recorded"` and `"stale"` are absences. A
//!    decoder never reads one of them as a present value.
//! 2. A present value that would serialize to a reserved word **cannot be
//!    encoded**. [`Recorded::present`] refuses to build one, and the
//!    serializer refuses to write one, so the bytes never exist to be
//!    misread. See [`Recordable`], which is how a type says what it could
//!    collide with.
//!
//! Together these make the encoding injective: every byte string has exactly
//! one reading, and every encodable value has exactly one encoding. The
//! encoding itself is **unchanged** from what statecraft-cli wrote before this
//! contract: this crate emits the same bytes for the same values, and the only
//! values it now refuses are the ones that were ambiguous.
//!
//! # Legacy decoding
//!
//! Bytes written by statecraft-cli at or before `8f6591f` could carry a
//! present string equal to a reserved word, because nothing refused it. Such a
//! record decodes **as the absence**, here and in every consumer of this
//! crate. That is a deliberate reinterpretation of a byte sequence that was
//! ambiguous when it was written, and it is the reading the CLI intended: the
//! field that motivated the type (`harness_revision`) is written as
//! `"not-recorded"` precisely to say that no harness revision was observed.
//!
//! The reinterpretation is not free, and the cost is named rather than hidden:
//! a record that genuinely meant the literal string `not-recorded` now reads
//! as an absence and cannot be re-encoded. No such record is known to exist;
//! the fixture `testdata/fixtures/cli/recorded-collision-legacy.json` is the
//! bytes the pre-contract CLI would have written for one, and
//! `tests/cli_compat.rs` pins how they read today.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Why something is not here.
///
/// A positive statement in every case. `None` is "somebody looked and there
/// was nothing", `NotRecorded` is "no record of this kind exists", `Stale` is
/// "a record exists for a revision that is no longer current".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Absence {
    /// Somebody looked; there was nothing.
    None,
    /// No record of this kind exists, including every field a future contract adds.
    NotRecorded,
    /// A record exists for a revision that is no longer current.
    Stale,
}

impl Absence {
    /// The word this is written as.
    pub fn word(self) -> &'static str {
        match self {
            Absence::None => "none",
            Absence::NotRecorded => "not-recorded",
            Absence::Stale => "stale",
        }
    }

    /// The absence a reserved word names, if it is one of the three.
    pub fn from_word(word: &str) -> Option<Absence> {
        match word {
            "none" => Some(Absence::None),
            "not-recorded" => Some(Absence::NotRecorded),
            "stale" => Some(Absence::Stale),
            _ => Option::None,
        }
    }

    /// Every name. A test asserts there are exactly three.
    pub fn all() -> [Absence; 3] {
        [Absence::None, Absence::NotRecorded, Absence::Stale]
    }
}

/// The three reserved words, in the order [`Absence::all`] gives them.
pub const RESERVED_WORDS: [&str; 3] = ["none", "not-recorded", "stale"];

/// What a value can be recorded as, and what it would collide with.
///
/// Implemented by every `T` that appears in a `Recorded<T>` that is written.
/// The default answer is "this type cannot be confused with an absence", which
/// is true of every type whose serialized form is not a bare string, so most
/// implementations are one line.
///
/// A type whose values *can* serialize to a bare string must answer honestly,
/// or the encoding stops being injective for it.
pub trait Recordable {
    /// The absence this value would be indistinguishable from on the wire.
    fn reserved_collision(&self) -> Option<Absence> {
        None
    }
}

impl Recordable for String {
    fn reserved_collision(&self) -> Option<Absence> {
        Absence::from_word(self)
    }
}

impl Recordable for &str {
    fn reserved_collision(&self) -> Option<Absence> {
        Absence::from_word(self)
    }
}

/// Types whose serialized form is never a bare string.
macro_rules! recordable_never_collides {
    ($($t:ty),* $(,)?) => {
        $(impl Recordable for $t {})*
    };
}

recordable_never_collides!(
    bool, u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64
);

impl<T> Recordable for Vec<T> {}

/// A present value that cannot be encoded, because its bytes are a reserved
/// absence word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservedValue {
    /// The absence the value would have been read as.
    pub absence: Absence,
}

impl fmt::Display for ReservedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is reserved for the absence {:?}: a present value cannot be written as it",
            self.absence.word(),
            self.absence
        )
    }
}

impl std::error::Error for ReservedValue {}

/// A value, or a named absence.
///
/// Serialized untagged: a present value as itself, an absence as its word. The
/// reserved-word contract at the module level is what keeps that unambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded<T> {
    /// Absent, with which kind.
    Absent(Absence),
    /// A value was recorded.
    Present(T),
}

impl<T: Recordable> Recorded<T> {
    /// Record a value, or refuse it because its bytes are reserved.
    ///
    /// The checked constructor. Building `Recorded::Present(v)` directly is
    /// still possible and still safe: the serializer performs the same check,
    /// so an unencodable value is refused at the point it would have produced
    /// ambiguous bytes rather than silently producing them.
    pub fn present(value: T) -> Result<Self, ReservedValue> {
        match value.reserved_collision() {
            Some(absence) => Err(ReservedValue { absence }),
            None => Ok(Recorded::Present(value)),
        }
    }
}

impl<T> Recorded<T> {
    /// Whether a value is actually here.
    pub fn is_present(&self) -> bool {
        matches!(self, Recorded::Present(_))
    }

    /// The value, if there is one.
    pub fn value(&self) -> Option<&T> {
        match self {
            Recorded::Present(v) => Some(v),
            Recorded::Absent(_) => None,
        }
    }

    /// The kind of absence, if absent.
    pub fn absence(&self) -> Option<Absence> {
        match self {
            Recorded::Present(_) => None,
            Recorded::Absent(a) => Some(*a),
        }
    }
}

impl<T: Serialize + Recordable> Serialize for Recorded<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Recorded::Absent(a) => s.serialize_str(a.word()),
            Recorded::Present(v) => match v.reserved_collision() {
                Some(absence) => Err(serde::ser::Error::custom(ReservedValue { absence })),
                None => v.serialize(s),
            },
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Recorded<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(RecordedVisitor(PhantomData))
    }
}

struct RecordedVisitor<T>(PhantomData<T>);

/// Every arm but the string one forwards to `T`: only a bare string can be a
/// reserved word, so only a bare string is examined before `T` sees it.
impl<'de, T: Deserialize<'de>> Visitor<'de> for RecordedVisitor<T> {
    type Value = Recorded<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a recorded value, or one of the absences {:?}",
            RESERVED_WORDS
        )
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        match Absence::from_word(v) {
            Some(a) => Ok(Recorded::Absent(a)),
            None => T::deserialize(v.into_deserializer()).map(Recorded::Present),
        }
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        match Absence::from_word(&v) {
            Some(a) => Ok(Recorded::Absent(a)),
            None => T::deserialize(v.into_deserializer()).map(Recorded::Present),
        }
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        T::deserialize(v.into_deserializer()).map(Recorded::Present)
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        T::deserialize(v.into_deserializer()).map(Recorded::Present)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        T::deserialize(v.into_deserializer()).map(Recorded::Present)
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        T::deserialize(v.into_deserializer()).map(Recorded::Present)
    }

    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        T::deserialize(de::value::BytesDeserializer::new(v)).map(Recorded::Present)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        T::deserialize(de::value::UnitDeserializer::new()).map(Recorded::Present)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        T::deserialize(de::value::UnitDeserializer::new()).map(Recorded::Present)
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        T::deserialize(d).map(Recorded::Present)
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        T::deserialize(d).map(Recorded::Present)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        T::deserialize(de::value::SeqAccessDeserializer::new(a)).map(Recorded::Present)
    }

    fn visit_map<A: MapAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        T::deserialize(de::value::MapAccessDeserializer::new(a)).map(Recorded::Present)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_exactly_three_names_for_absence_and_none_reads_as_a_pass() {
        assert_eq!(Absence::all().len(), 3);
        for a in Absence::all() {
            assert_ne!(a.word(), "pass");
            assert_ne!(a.word(), "ok");
            assert!(!a.word().is_empty());
            assert_eq!(Absence::from_word(a.word()), Some(a));
        }
        let words: Vec<&str> = Absence::all().iter().map(|a| a.word()).collect();
        assert_eq!(words, RESERVED_WORDS);
    }

    #[test]
    fn an_absence_is_written_as_its_word() {
        for a in Absence::all() {
            let r: Recorded<String> = Recorded::Absent(a);
            assert_eq!(
                serde_json::to_string(&r).unwrap(),
                format!("\"{}\"", a.word())
            );
        }
    }

    #[test]
    fn every_reserved_word_reads_as_an_absence_and_never_as_a_value() {
        for a in Absence::all() {
            let text = format!("\"{}\"", a.word());
            let r: Recorded<String> = serde_json::from_str(&text).unwrap();
            assert_eq!(r, Recorded::Absent(a));
            assert!(!r.is_present());
        }
    }

    #[test]
    fn a_present_value_equal_to_a_reserved_word_is_refused_at_construction() {
        for a in Absence::all() {
            assert_eq!(
                Recorded::<String>::present(a.word().to_string()),
                Err(ReservedValue { absence: a })
            );
        }
        assert!(Recorded::<String>::present("not-recorded-here".into()).is_ok());
    }

    #[test]
    fn a_present_value_equal_to_a_reserved_word_is_refused_at_serialization() {
        // Constructed around the checked constructor on purpose: the bytes
        // must not exist even when the value was built by hand.
        let r = Recorded::Present("stale".to_string());
        let err = serde_json::to_string(&r).unwrap_err();
        assert!(err.to_string().contains("reserved"), "{err}");
    }

    #[test]
    fn an_ordinary_value_round_trips_unchanged() {
        let r = Recorded::<String>::present("harness-2026.09".into()).unwrap();
        let text = serde_json::to_string(&r).unwrap();
        assert_eq!(text, "\"harness-2026.09\"");
        assert_eq!(serde_json::from_str::<Recorded<String>>(&text).unwrap(), r);
    }

    #[test]
    fn a_non_string_value_is_never_ambiguous_and_is_not_examined() {
        let r = Recorded::<u32>::present(7).unwrap();
        assert_eq!(serde_json::to_string(&r).unwrap(), "7");
        assert_eq!(
            serde_json::from_str::<Recorded<u32>>("7").unwrap(),
            Recorded::Present(7)
        );
        // The absences stay readable for a numeric payload too.
        assert_eq!(
            serde_json::from_str::<Recorded<u32>>("\"stale\"").unwrap(),
            Recorded::Absent(Absence::Stale)
        );
    }

    #[test]
    fn a_structured_value_round_trips_through_the_map_arm() {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct Claim {
            text: String,
        }
        impl Recordable for Claim {}

        let r = Recorded::present(Claim {
            text: "all done".into(),
        })
        .unwrap();
        let text = serde_json::to_string(&r).unwrap();
        assert_eq!(text, r#"{"text":"all done"}"#);
        assert_eq!(serde_json::from_str::<Recorded<Claim>>(&text).unwrap(), r);
        assert_eq!(
            serde_json::from_str::<Recorded<Claim>>("\"none\"").unwrap(),
            Recorded::Absent(Absence::None)
        );
    }

    #[test]
    fn a_future_field_is_present_reading_not_recorded_rather_than_omitted() {
        let field: Recorded<String> = Recorded::Absent(Absence::NotRecorded);
        assert_eq!(serde_json::to_string(&field).unwrap(), "\"not-recorded\"");
        assert!(!field.is_present());
    }
}
