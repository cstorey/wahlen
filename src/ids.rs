use failure::{bail, Fail};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use data_encoding::BASE32_DNSSEC;
use failure::Error;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::untyped_ids::UntypedId;

pub(crate) const ENCODED_BARE_ID_LEN: usize = 26;

#[derive(Debug)]
pub struct Id<T> {
    // Unix time in ms
    inner: UntypedId,
    phantom: PhantomData<T>,
}

#[derive(Debug, Clone, Fail)]
pub enum IdParseError {
    InvalidPrefix,
    Unparseable,
}

pub trait Entity {
    const PREFIX: &'static str;
}

#[derive(Debug, Clone, Default)]
pub struct IdGen {}

const DIVIDER: &str = ".";

impl<T> Id<T> {
    /// Returns a id nominally at time zero, but with a random portion derived
    /// from the given entity.
    pub fn hashed<H: Hash>(entity: H) -> Self {
        let inner = UntypedId::hashed(entity);
        let phantom = PhantomData;
        Id { inner, phantom }
    }
}

impl IdGen {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn generate<T>(&self) -> Id<T> {
        let inner = self.untyped();
        let phantom = PhantomData;

        Id { inner, phantom }
    }
}

impl<T> Id<T> {
    fn from_bytes(bytes: &[u8]) -> Self {
        let inner = UntypedId::from_bytes(bytes);

        let phantom = PhantomData;

        Id { inner, phantom }
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    pub(crate) fn from_untyped(src: UntypedId) -> Self {
        Id {
            inner: src,
            phantom: PhantomData,
        }
    }

    pub fn untyped(&self) -> UntypedId {
        self.inner
    }
}

impl<T: Entity> fmt::Display for Id<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; ENCODED_BARE_ID_LEN];
        BASE32_DNSSEC.encode_mut(&self.to_bytes(), &mut buf);

        write!(
            fmt,
            "{}{}{}",
            T::PREFIX,
            DIVIDER,
            String::from_utf8_lossy(&buf[..])
        )?;
        Ok(())
    }
}

impl<T: Entity> std::str::FromStr for Id<T> {
    type Err = Error;
    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let expected_length = T::PREFIX.len() + DIVIDER.len();
        if src.len() < expected_length {
            bail!(IdParseError::InvalidPrefix);
        };
        let (start, remainder) = src.split_at(T::PREFIX.len());
        if start != T::PREFIX {
            bail!(IdParseError::InvalidPrefix);
        }
        let (divider, b64) = remainder.split_at(DIVIDER.len());

        if divider != DIVIDER {
            bail!(IdParseError::Unparseable);
        }

        let mut bytes = [0u8; 16];
        BASE32_DNSSEC
            .decode_mut(b64.as_bytes(), &mut bytes)
            .map_err(|e| failure::format_err!("{:?}", e))?;

        Ok(Self::from_bytes(&bytes[..]))
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Id {
            inner: self.inner,
            phantom: self.phantom,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.inner.hash(hasher);
    }
}

impl<T: Entity> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de, T: Entity> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdStrVisitor<T>(PhantomData<T>);
        impl<'vi, T: Entity> de::Visitor<'vi> for IdStrVisitor<T> {
            type Value = Id<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "an Id string")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Id<T>, E> {
                value.parse::<Id<T>>().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(IdStrVisitor(PhantomData))
    }
}

impl fmt::Display for IdParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            &IdParseError::InvalidPrefix => write!(fmt, "Invalid prefix"),
            &IdParseError::Unparseable => write!(fmt, "Unparseable Id"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json;

    #[derive(Debug)]
    struct Canary;

    impl Entity for Canary {
        const PREFIX: &'static str = "canary";
    }

    #[test]
    fn round_trips_via_to_from_str() {
        let id = Id::<Canary>::hashed(&"Hi!");
        let s = id.to_string();
        println!("String: {}", s);
        let id2 = s.parse::<Id<Canary>>().expect("parse id");
        assert_eq!(id, id2);
    }

    #[test]
    fn round_trips_via_to_from_str_now() {
        let id = IdGen::new().generate::<Canary>();
        let s = id.to_string();
        println!("String: {}", s);
        let id2 = s.parse::<Id<Canary>>().expect("parse id");
        assert_eq!(id, id2);
    }

    #[test]
    fn round_trips_via_serde_json() {
        let id = Id::<Canary>::hashed(&"boo");

        let json = serde_json::to_string(&id).expect("serde_json::to_string");
        println!("Json: {}", json);
        let id2 = serde_json::from_str(&json).expect("serde_json::from_str");
        assert_eq!(id, id2);
    }

    #[test]
    fn round_trips_via_untyped() {
        let id = Id::<Canary>::hashed(&"boo");

        let untyped: UntypedId = id.untyped();
        println!("untyped: {}", untyped);
        let id2: Id<Canary> = Id::from_untyped(untyped);
        assert_eq!(id, id2);
    }

    #[test]
    fn serializes_to_string_like() {
        let id = Id::<Canary>::hashed(&"Hi!");

        let json = serde_json::to_string(&id).expect("serde_json::to_string");
        let s: String = serde_json::from_str(&json).expect("serde_json::from_str");
        assert_eq!(id.to_string(), s);
    }

    #[test]
    fn should_allow_random_generation() {
        let idgen = IdGen::new();
        let id = idgen.generate::<Canary>();
        let id2 = idgen.generate::<Canary>();

        assert_ne!(id, id2);
    }

    #[test]
    fn should_allow_ordering() {
        let idgen = IdGen::new();
        let id = idgen.generate::<Canary>();
        let mut id2 = idgen.generate::<Canary>();
        while id2 == id {
            id2 = idgen.generate::<Canary>();
        }

        assert!(id < id2 || id > id2);
    }

    #[test]
    fn to_string_should_be_prefixed_with_type_name() {
        let idgen = IdGen::new();
        let id = idgen.generate::<Canary>();

        let s = id.to_string();

        assert!(
            s.starts_with("canary"),
            "string: {:?} starts with {:?}",
            s,
            "canary"
        )
    }

    #[test]
    fn should_parse_correct_example() {
        let s = "canary.0000000000001q5nnvfqq7krfo";

        let result = s.parse::<Id<Canary>>();

        assert!(
            result.is_ok(),
            "Parsing {:?} should return ok; got {:?}",
            s,
            result,
        )
    }

    #[test]
    fn should_verify_has_correct_entity_prefix() {
        let s = "wrongy-0000000000001q5nnvfqq7krfo";

        let result = s.parse::<Id<Canary>>();

        assert!(
            result.is_err(),
            "Parsing {:?} should return error; got {:?}",
            s,
            result,
        )
    }

    #[test]
    fn should_yield_useful_error_when_invalid_prefix() {
        #[derive(Debug)]
        struct Long;
        impl Entity for Long {
            // Borrowed from https://en.wikipedia.org/wiki/Longest_word_in_English
            // We want it to be longer than the id string in total.
            const PREFIX: &'static str = "pseudopseudohypoparathyroidism";
        }
        let s = "wrong-0000000000001q5nnvfqq7krfo";

        let result = s.parse::<Id<Long>>();

        assert!(
            result.is_err(),
            "Parsing {:?} should return error; got {:?}",
            s,
            result,
        )
    }

    #[test]
    fn should_yield_useful_error_when_just_prefix() {
        let s = "canary";
        let result = s.parse::<Id<Canary>>();

        assert!(
            result.is_err(),
            "Parsing {:?} should return error; got {:?}",
            s,
            result,
        )
    }
    #[test]
    fn should_yield_useful_error_when_wrong_divider() {
        let s = "canary#0000000000001q5nnvfqq7krfo";
        let result = s.parse::<Id<Canary>>();

        assert!(
            result.is_err(),
            "Parsing {:?} should return error; got {:?}",
            s,
            result,
        )
    }
}
