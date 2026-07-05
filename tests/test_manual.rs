//! Tests for handwritten `Deserialize` impls using the safe visitor API,
//! without any `make_place!`-style unsafe casting.

use std::collections::BTreeMap;

use microserde::de::{Deserialize, Map, Scalar, Seq, Visitor};
use microserde::{json, Result};

/// A string-encoded identifier that is validated while deserializing.
#[derive(Debug, PartialEq)]
struct Oid([u8; 4]);

impl std::str::FromStr for Oid {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, ()> {
        let bytes = s.as_bytes();
        if bytes.len() == 8 && bytes.iter().all(u8::is_ascii_hexdigit) {
            let mut oid = [0; 4];
            for (i, chunk) in bytes.chunks(2).enumerate() {
                oid[i] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap();
            }
            Ok(Oid(oid))
        } else {
            Err(())
        }
    }
}

struct OidVisitor<'a> {
    out: &'a mut Option<Oid>,
}

impl Visitor for OidVisitor<'_> {
    fn scalar(&mut self, s: &Scalar) -> Result<()> {
        let Scalar::Str(value) = s else {
            return Err(microserde::Error);
        };
        *self.out = Some(value.parse().map_err(|()| microserde::Error)?);
        Ok(())
    }
}

impl Deserialize for Oid {
    type Visitor<'a> = OidVisitor<'a>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        OidVisitor { out }
    }
}

#[test]
fn test_manual_string_visitor() {
    let oid: Oid = json::from_str(r#""deadbeef""#).unwrap();
    assert_eq!(oid, Oid([0xde, 0xad, 0xbe, 0xef]));

    // Validation errors are surfaced as deserialization errors.
    assert!(json::from_str::<Oid>(r#""nonsense""#).is_err());
    // Non-string input is rejected by the default visitor methods.
    assert!(json::from_str::<Oid>("17").is_err());
}

#[test]
fn test_manual_visitor_nested() {
    // Manual impls compose with built-in and derived impls.
    let oids: Vec<Oid> = json::from_str(r#"["00000000", "deadbeef"]"#).unwrap();
    assert_eq!(oids, vec![Oid([0; 4]), Oid([0xde, 0xad, 0xbe, 0xef])]);

    let map: BTreeMap<String, Option<Oid>> =
        json::from_str(r#"{"a": "deadbeef", "b": null}"#).unwrap();
    assert_eq!(map["a"], Some(Oid([0xde, 0xad, 0xbe, 0xef])));
    assert_eq!(map["b"], None);

    #[derive(microserde::Deserialize, Debug, PartialEq)]
    struct Commit {
        id: Oid,
        parents: Vec<Oid>,
    }

    let commit: Commit = json::from_str(r#"{"id": "deadbeef", "parents": []}"#).unwrap();
    assert_eq!(commit.id, Oid([0xde, 0xad, 0xbe, 0xef]));
    assert_eq!(commit.parents, vec![]);
}

/// A set of unique numbers, deserialized from a JSON array through a manual
/// sequence visitor.
#[derive(Debug, PartialEq)]
struct NumberSet(Vec<u32>);

struct NumberSetVisitor<'a> {
    out: &'a mut Option<NumberSet>,
}

impl Visitor for NumberSetVisitor<'_> {
    fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
    where
        Self: 's,
    {
        Ok(Box::new(NumberSetBuilder {
            out: self.out,
            set: Vec::new(),
            element: None,
        }))
    }
}

struct NumberSetBuilder<'a> {
    out: &'a mut Option<NumberSet>,
    set: Vec<u32>,
    element: Option<u32>,
}

impl NumberSetBuilder<'_> {
    fn shift(&mut self) -> Result<()> {
        if let Some(e) = self.element.take() {
            if self.set.contains(&e) {
                return Err(microserde::Error);
            }
            self.set.push(e);
        }
        Ok(())
    }
}

impl Seq for NumberSetBuilder<'_> {
    fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
        self.shift()?;
        Ok(Box::new(Deserialize::begin(&mut self.element)))
    }

    fn finish(&mut self) -> Result<()> {
        self.shift()?;
        *self.out = Some(NumberSet(std::mem::take(&mut self.set)));
        Ok(())
    }
}

impl Deserialize for NumberSet {
    type Visitor<'a> = NumberSetVisitor<'a>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        NumberSetVisitor { out }
    }
}

#[test]
fn test_manual_seq_visitor() {
    let set: NumberSet = json::from_str("[3, 1, 2]").unwrap();
    assert_eq!(set, NumberSet(vec![3, 1, 2]));

    assert!(json::from_str::<NumberSet>("[1, 2, 1]").is_err());
    assert!(json::from_str::<NumberSet>("{}").is_err());
}

/// A struct deserialized from a JSON object through a manual map visitor.
#[derive(Debug, PartialEq)]
struct Version {
    major: u32,
    minor: u32,
}

struct VersionVisitor<'a> {
    out: &'a mut Option<Version>,
}

impl Visitor for VersionVisitor<'_> {
    fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
    where
        Self: 's,
    {
        Ok(Box::new(VersionBuilder {
            out: self.out,
            major: None,
            minor: None,
        }))
    }
}

struct VersionBuilder<'a> {
    out: &'a mut Option<Version>,
    major: Option<u32>,
    minor: Option<u32>,
}

impl Map for VersionBuilder<'_> {
    fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>> {
        match k {
            "major" => Ok(Box::new(Deserialize::begin(&mut self.major))),
            "minor" => Ok(Box::new(Deserialize::begin(&mut self.minor))),
            _ => Ok(<dyn Visitor>::ignore()),
        }
    }

    fn finish(&mut self) -> Result<()> {
        *self.out = Some(Version {
            major: self.major.take().ok_or(microserde::Error)?,
            minor: self.minor.take().ok_or(microserde::Error)?,
        });
        Ok(())
    }
}

impl Deserialize for Version {
    type Visitor<'a> = VersionVisitor<'a>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        VersionVisitor { out }
    }
}

#[test]
fn test_manual_map_visitor() {
    let version: Version =
        json::from_str(r#"{"major": 1, "ignored": [null], "extra": "x", "minor": 2}"#).unwrap();
    assert_eq!(version, Version { major: 1, minor: 2 });

    assert!(json::from_str::<Version>(r#"{"major": 1}"#).is_err());
}
