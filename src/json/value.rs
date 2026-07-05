use crate::lib::*;

use crate::de::{Deserialize, Map, Place, Scalar, Seq, Visitor};
use crate::error::Result;
use crate::json::{Array, Number, Object};
use crate::private;
use crate::ser::{Fragment, Serialize};

/// Any valid JSON value.
///
/// This type has a non-recursive drop implementation so it is safe to build
/// arbitrarily deeply nested instances.
///
/// ```rust
/// use microserde::json::{Array, Value};
///
/// let mut value = Value::Null;
/// for _ in 0..100000 {
///     let mut array = Array::new();
///     array.push(value);
///     value = Value::Array(array);
/// }
/// // no stack overflow when `value` goes out of scope
/// ```
#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Array),
    Object(Object),
}

impl Default for Value {
    /// The default value is null.
    fn default() -> Self {
        Value::Null
    }
}

impl Serialize for Value {
    fn begin(&self) -> Fragment {
        match self {
            Value::Null => Fragment::Null,
            Value::Bool(b) => Fragment::Bool(*b),
            Value::Number(Number::U64(n)) => Fragment::U64(*n),
            Value::Number(Number::I64(n)) => Fragment::I64(*n),
            Value::Number(Number::F64(n)) => Fragment::F64(*n),
            Value::String(s) => Fragment::Str(Cow::Borrowed(s)),
            Value::Array(array) => private::stream_slice(array),
            Value::Object(object) => private::stream_object(object),
        }
    }
}

impl Deserialize for Value {
    type Visitor<'a> = Place<'a, Value>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl Visitor for Place<'_, Value> {
            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                *self.out = Some(match *s {
                    Scalar::Null => Value::Null,
                    Scalar::Bool(b) => Value::Bool(b),
                    Scalar::Str(s) => Value::String(s.to_owned()),
                    Scalar::Negative(n) => Value::Number(Number::I64(n)),
                    Scalar::Nonnegative(n) => Value::Number(Number::U64(n)),
                    Scalar::Float(n) => Value::Number(Number::F64(n)),
                });
                Ok(())
            }

            fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
            where
                Self: 's,
            {
                Ok(Box::new(ArrayBuilder {
                    out: self.out,
                    array: Array::new(),
                    element: None,
                }))
            }

            fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
            where
                Self: 's,
            {
                Ok(Box::new(ObjectBuilder {
                    out: self.out,
                    object: Object::new(),
                    key: None,
                    value: None,
                }))
            }
        }

        struct ArrayBuilder<'a> {
            out: &'a mut Option<Value>,
            array: Array,
            element: Option<Value>,
        }

        impl<'a> ArrayBuilder<'a> {
            fn shift(&mut self) {
                if let Some(e) = self.element.take() {
                    self.array.push(e);
                }
            }
        }

        impl<'a> Seq for ArrayBuilder<'a> {
            fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
                self.shift();
                Ok(Box::new(Deserialize::begin(&mut self.element)))
            }

            fn finish(&mut self) -> Result<()> {
                self.shift();
                *self.out = Some(Value::Array(mem::replace(&mut self.array, Array::new())));
                Ok(())
            }

            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                self.shift();
                Deserialize::begin(&mut self.element).scalar(s)
            }
        }

        struct ObjectBuilder<'a> {
            out: &'a mut Option<Value>,
            object: Object,
            key: Option<String>,
            value: Option<Value>,
        }

        impl<'a> ObjectBuilder<'a> {
            fn shift(&mut self) {
                if let (Some(k), Some(v)) = (self.key.take(), self.value.take()) {
                    self.object.insert(k, v);
                }
            }
        }

        impl<'a> Map for ObjectBuilder<'a> {
            fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>> {
                self.shift();
                self.key = Some(k.to_owned());
                Ok(Box::new(Deserialize::begin(&mut self.value)))
            }

            fn finish(&mut self) -> Result<()> {
                self.shift();
                *self.out = Some(Value::Object(mem::replace(&mut self.object, Object::new())));
                Ok(())
            }

            fn scalar(&mut self, k: &str, s: &Scalar) -> Result<()> {
                self.shift();
                self.key = Some(k.to_owned());
                Deserialize::begin(&mut self.value).scalar(s)
            }
        }

        Place::new(out)
    }
}
