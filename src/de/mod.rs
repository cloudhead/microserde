//! Deserialization traits.
//!
//! Deserialization in microserde works by handing out a visitor that borrows
//! the output slot into which data may be written through the methods of the
//! `Visitor` trait.
//!
//! A `Deserialize` impl provides an associated visitor type that holds a
//! `&mut Option<Self>` pointing at the output slot. Upon successful
//! deserialization the output object is written as `Some(T)` into that slot.
//!
//! ## Deserializing a primitive
//!
//! The visitor receives scalar values through the `scalar` method and matches
//! on the [`Scalar`] variants that the Rust type supports deserializing from.
//!
//! ```rust
//! use microserde::{Error, Result};
//! use microserde::de::{Deserialize, Scalar, Visitor};
//!
//! struct MyBoolean(bool);
//!
//! struct MyBooleanVisitor<'a> {
//!     out: &'a mut Option<MyBoolean>,
//! }
//!
//! // We match the scalar variants that our Rust type supports deserializing
//! // from, and write the result into the output slot.
//! //
//! // This method may perform validation and decide to return an error.
//! impl Visitor for MyBooleanVisitor<'_> {
//!     fn scalar(&mut self, s: &Scalar) -> Result<()> {
//!         let Scalar::Bool(b) = *s else {
//!             return Err(Error);
//!         };
//!         *self.out = Some(MyBoolean(b));
//!         Ok(())
//!     }
//! }
//!
//! impl Deserialize for MyBoolean {
//!     type Visitor<'a> = MyBooleanVisitor<'a>;
//!
//!     fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
//!         MyBooleanVisitor { out }
//!     }
//! }
//! ```
//!
//! ## Deserializing a sequence
//!
//! In the case of a sequence (JSON array), the visitor method consumes the
//! visitor and returns a builder that can hand out visitors to write sequence
//! elements one element at a time.
//!
//! ```rust
//! use microserde::Result;
//! use microserde::de::{Deserialize, Seq, Visitor};
//! use std::mem;
//!
//! struct MyVec<T>(Vec<T>);
//!
//! struct MyVecVisitor<'a, T> {
//!     out: &'a mut Option<MyVec<T>>,
//! }
//!
//! impl<T: Deserialize> Visitor for MyVecVisitor<'_, T> {
//!     fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
//!     where
//!         Self: 's,
//!     {
//!         Ok(Box::new(VecBuilder {
//!             out: self.out,
//!             vec: Vec::new(),
//!             element: None,
//!         }))
//!     }
//! }
//!
//! struct VecBuilder<'a, T: 'a> {
//!     // At the end, output will be written here.
//!     out: &'a mut Option<MyVec<T>>,
//!     // Previous elements are accumulated here.
//!     vec: Vec<T>,
//!     // Next element will be placed here.
//!     element: Option<T>,
//! }
//!
//! impl<'a, T: Deserialize> Seq for VecBuilder<'a, T> {
//!     fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
//!         // Free up the place by transfering the most recent element
//!         // into self.vec.
//!         self.vec.extend(self.element.take());
//!         // Hand out a visitor to write the next element.
//!         Ok(Box::new(Deserialize::begin(&mut self.element)))
//!     }
//!
//!     fn finish(&mut self) -> Result<()> {
//!         // Transfer the last element.
//!         self.vec.extend(self.element.take());
//!         // Move the output object into self.out.
//!         let vec = mem::replace(&mut self.vec, Vec::new());
//!         *self.out = Some(MyVec(vec));
//!         Ok(())
//!     }
//! }
//!
//! impl<T: Deserialize> Deserialize for MyVec<T> {
//!     type Visitor<'a>
//!         = MyVecVisitor<'a, T>
//!     where
//!         Self: 'a;
//!
//!     fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
//!         MyVecVisitor { out }
//!     }
//! }
//! ```
//!
//! ## Deserializing a map or struct
//!
//! This code demonstrates what is generated for structs by
//! `#[derive(Deserialize)]`.
//!
//! ```rust
//! use microserde::Result;
//! use microserde::de::{Deserialize, Map, Visitor};
//!
//! // The struct that we would like to deserialize.
//! struct Demo {
//!     code: u32,
//!     message: String,
//! }
//!
//! struct DemoVisitor<'a> {
//!     out: &'a mut Option<Demo>,
//! }
//!
//! impl Visitor for DemoVisitor<'_> {
//!     fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
//!     where
//!         Self: 's,
//!     {
//!         // Like for sequences, we produce a builder that can hand out
//!         // visitors to write one struct field at a time.
//!         Ok(Box::new(DemoBuilder {
//!             code: None,
//!             message: None,
//!             out: self.out,
//!         }))
//!     }
//! }
//!
//! struct DemoBuilder<'a> {
//!     code: Option<u32>,
//!     message: Option<String>,
//!     out: &'a mut Option<Demo>,
//! }
//!
//! impl<'a> Map for DemoBuilder<'a> {
//!     fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>> {
//!         // Figure out which field is being deserialized and return a
//!         // visitor to write it.
//!         //
//!         // The code here ignores unrecognized fields but an implementation
//!         // would be free to return an error instead. Similarly an
//!         // implementation may want to check for duplicate fields by
//!         // returning an error if the current field already has a value.
//!         match k {
//!             "code" => Ok(Box::new(Deserialize::begin(&mut self.code))),
//!             "message" => Ok(Box::new(Deserialize::begin(&mut self.message))),
//!             _ => Ok(<dyn Visitor>::ignore()),
//!         }
//!     }
//!
//!     fn finish(&mut self) -> Result<()> {
//!         // Make sure we have every field and then write the output object
//!         // into self.out.
//!         let code = self.code.take().ok_or(microserde::Error)?;
//!         let message = self.message.take().ok_or(microserde::Error)?;
//!         *self.out = Some(Demo { code, message });
//!         Ok(())
//!     }
//! }
//!
//! impl Deserialize for Demo {
//!     type Visitor<'a> = DemoVisitor<'a>;
//!
//!     fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
//!         DemoVisitor { out }
//!     }
//! }
//! ```

mod impls;

use crate::error::{Error, Result};
use crate::lib::Box;

/// Trait for data structures that can be deserialized from a JSON string.
///
/// [Refer to the module documentation for examples.][crate::de]
pub trait Deserialize: Sized {
    /// A visitor holding a mutable reference to the output slot, into which
    /// the deserialized value is written.
    type Visitor<'a>: Visitor + 'a
    where
        Self: 'a;

    /// Build a visitor that writes into the given output slot.
    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_>;

    // Not public API. This method is only intended for Option<T>, should not
    // need to be implemented outside of this crate.
    #[doc(hidden)]
    #[inline]
    fn default() -> Option<Self> {
        None
    }
}

/// A single scalar value.
pub enum Scalar<'a> {
    Null,
    Bool(bool),
    Str(&'a str),
    Negative(i64),
    Nonnegative(u64),
    Float(f64),
}

/// Trait that can write data into an output place.
///
/// [Refer to the module documentation for examples.][crate::de]
pub trait Visitor {
    /// Write a scalar value into the output slot.
    fn scalar(&mut self, s: &Scalar) -> Result<()> {
        let _ = s;
        Err(Error)
    }

    /// Begin a sequence (JSON array). Consumes the visitor and returns a
    /// builder owning everything the visitor held, so that the builder can
    /// hand out element visitors and finally write the output slot.
    fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
    where
        Self: 's,
    {
        Err(Error)
    }

    /// Begin a map (JSON object). Consumes the visitor and returns a builder
    /// owning everything the visitor held, so that the builder can hand out
    /// value visitors and finally write the output slot.
    fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
    where
        Self: 's,
    {
        Err(Error)
    }
}

/// Trait that can hand out visitors to write sequence elements.
///
/// [Refer to the module documentation for examples.][crate::de]
pub trait Seq {
    fn element(&mut self) -> Result<Box<dyn Visitor + '_>>;
    fn finish(&mut self) -> Result<()>;

    /// Write a scalar element. Equivalent to writing through the visitor
    /// returned by `element`, which is what the default implementation does;
    /// implementations can override this to write the element without
    /// allocating a boxed visitor.
    fn scalar(&mut self, s: &Scalar) -> Result<()> {
        self.element()?.scalar(s)
    }
}

/// Trait that can hand out visitors to write values of a map.
///
/// [Refer to the module documentation for examples.][crate::de]
pub trait Map {
    fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>>;
    fn finish(&mut self) -> Result<()>;

    /// Write a scalar value under the given key. Equivalent to writing
    /// through the visitor returned by `key`, which is what the default
    /// implementation does; implementations can override this to write the
    /// value without allocating a boxed visitor.
    fn scalar(&mut self, k: &str, s: &Scalar) -> Result<()> {
        self.key(k)?.scalar(s)
    }
}

/// A visitor that holds nothing but a mutable reference to the output slot.
///
/// This is the visitor type used by the `Deserialize` impls built into
/// microserde for types whose visitor needs no state besides the output slot.
///
/// Note that the orphan rules prevent other crates from writing `impl Visitor
/// for Place<'_, TheirType>`, so outside of microserde a bespoke visitor
/// struct holding the output slot must be defined instead. [Refer to the
/// module documentation for examples.][crate::de]
pub struct Place<'a, T> {
    /// The output slot. Upon successful deserialization the output object is
    /// written here as `Some(T)`.
    pub out: &'a mut Option<T>,
}

impl<'a, T> Place<'a, T> {
    pub fn new(out: &'a mut Option<T>) -> Self {
        Place { out }
    }
}

// Not public API. Implemented by types that deserialize by delegating to an
// inner type and wrapping the result: `#[serde(transparent)]` newtypes and
// `Box<T>`.
#[doc(hidden)]
pub trait Transparent: Sized {
    type Inner: Deserialize;
    fn wrap(inner: Self::Inner) -> Self;
}

// Not public API. The visitor type of `#[serde(transparent)]` derived impls.
#[doc(hidden)]
pub struct TransparentVisitor<'a, T: Transparent> {
    out: &'a mut Option<T>,
}

// Not public API. The `Deserialize::begin` of `#[serde(transparent)]` derived
// impls.
#[doc(hidden)]
pub fn transparent<T: Transparent>(out: &mut Option<T>) -> TransparentVisitor<'_, T> {
    TransparentVisitor { out }
}

impl<'a, T: Transparent> Visitor for TransparentVisitor<'a, T> {
    fn scalar(&mut self, s: &Scalar) -> Result<()> {
        let mut value: Option<T::Inner> = None;
        Deserialize::begin(&mut value).scalar(s)?;
        *self.out = Some(T::wrap(value.ok_or(Error)?));
        Ok(())
    }

    fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
    where
        Self: 's,
    {
        let mut value = Box::new(None);
        let ptr = careful!(&mut *value as &mut Option<T::Inner>);
        Ok(Box::new(TransparentSeq {
            out: self.out,
            seq: Box::new(Deserialize::begin(ptr)).seq()?,
            value,
        }))
    }

    fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
    where
        Self: 's,
    {
        let mut value = Box::new(None);
        let ptr = careful!(&mut *value as &mut Option<T::Inner>);
        Ok(Box::new(TransparentMap {
            out: self.out,
            map: Box::new(Deserialize::begin(ptr)).map()?,
            value,
        }))
    }
}

struct TransparentSeq<'a, T: Transparent + 'a> {
    out: &'a mut Option<T>,
    // Borrows from `value`, so it is declared first to be dropped first.
    seq: Box<dyn Seq + 'a>,
    value: Box<Option<T::Inner>>,
}

impl<'a, T: Transparent> Seq for TransparentSeq<'a, T> {
    fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
        self.seq.element()
    }

    fn finish(&mut self) -> Result<()> {
        self.seq.finish()?;
        *self.out = Some(T::wrap(self.value.take().ok_or(Error)?));
        Ok(())
    }

    fn scalar(&mut self, s: &Scalar) -> Result<()> {
        self.seq.scalar(s)
    }
}

struct TransparentMap<'a, T: Transparent + 'a> {
    out: &'a mut Option<T>,
    // Borrows from `value`, so it is declared first to be dropped first.
    map: Box<dyn Map + 'a>,
    value: Box<Option<T::Inner>>,
}

impl<'a, T: Transparent> Map for TransparentMap<'a, T> {
    fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>> {
        self.map.key(k)
    }

    fn finish(&mut self) -> Result<()> {
        self.map.finish()?;
        *self.out = Some(T::wrap(self.value.take().ok_or(Error)?));
        Ok(())
    }

    fn scalar(&mut self, k: &str, s: &Scalar) -> Result<()> {
        self.map.scalar(k, s)
    }
}
