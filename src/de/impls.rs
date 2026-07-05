#[cfg(feature = "std")]
use crate::lib::hash::{BuildHasher, Hash};
use crate::lib::mem;
use crate::lib::str::FromStr;
use crate::lib::*;
#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::path::PathBuf;

use crate::de::{
    transparent, Deserialize, Map, Place, Scalar, Seq, Transparent, TransparentVisitor, Visitor,
};
use crate::error::{Error, Result};

impl Deserialize for () {
    type Visitor<'a> = Place<'a, ()>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl Visitor for Place<'_, ()> {
            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                let Scalar::Null = s else {
                    return Err(Error);
                };
                *self.out = Some(());
                Ok(())
            }
        }
        Place::new(out)
    }
}

impl Deserialize for bool {
    type Visitor<'a> = Place<'a, bool>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl Visitor for Place<'_, bool> {
            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                let Scalar::Bool(b) = *s else {
                    return Err(Error);
                };
                *self.out = Some(b);
                Ok(())
            }
        }
        Place::new(out)
    }
}

impl Deserialize for String {
    type Visitor<'a> = Place<'a, String>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl Visitor for Place<'_, String> {
            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                let Scalar::Str(s) = s else {
                    return Err(Error);
                };
                *self.out = Some((*s).to_owned());
                Ok(())
            }
        }
        Place::new(out)
    }
}

#[cfg(feature = "std")]
impl Deserialize for PathBuf {
    type Visitor<'a> = Place<'a, PathBuf>;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl Visitor for Place<'_, PathBuf> {
            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                let Scalar::Str(s) = s else {
                    return Err(Error);
                };
                *self.out = Some(PathBuf::from(s));
                Ok(())
            }
        }
        Place::new(out)
    }
}

macro_rules! signed {
    ($ty:ident) => {
        impl Deserialize for $ty {
            type Visitor<'a> = Place<'a, $ty>;

            fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
                impl Visitor for Place<'_, $ty> {
                    fn scalar(&mut self, s: &Scalar) -> Result<()> {
                        match *s {
                            Scalar::Negative(n) if n >= $ty::MIN as i64 => {
                                *self.out = Some(n as $ty);
                                Ok(())
                            }
                            Scalar::Nonnegative(n) if n <= $ty::MAX as u64 => {
                                *self.out = Some(n as $ty);
                                Ok(())
                            }
                            _ => Err(Error),
                        }
                    }
                }
                Place::new(out)
            }
        }
    };
}
signed!(i8);
signed!(i16);
signed!(i32);
signed!(i64);
signed!(isize);

macro_rules! unsigned {
    ($ty:ident) => {
        impl Deserialize for $ty {
            type Visitor<'a> = Place<'a, $ty>;

            fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
                impl Visitor for Place<'_, $ty> {
                    fn scalar(&mut self, s: &Scalar) -> Result<()> {
                        match *s {
                            Scalar::Nonnegative(n) if n <= $ty::MAX as u64 => {
                                *self.out = Some(n as $ty);
                                Ok(())
                            }
                            _ => Err(Error),
                        }
                    }
                }
                Place::new(out)
            }
        }
    };
}
unsigned!(u8);
unsigned!(u16);
unsigned!(u32);
unsigned!(u64);
unsigned!(usize);

macro_rules! float {
    ($ty:ident) => {
        impl Deserialize for $ty {
            type Visitor<'a> = Place<'a, $ty>;

            fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
                impl Visitor for Place<'_, $ty> {
                    fn scalar(&mut self, s: &Scalar) -> Result<()> {
                        let n = match *s {
                            Scalar::Negative(n) => n as $ty,
                            Scalar::Nonnegative(n) => n as $ty,
                            Scalar::Float(n) => n as $ty,
                            _ => return Err(Error),
                        };
                        *self.out = Some(n);
                        Ok(())
                    }
                }
                Place::new(out)
            }
        }
    };
}
float!(f32);
float!(f64);

// A boxed value deserializes like the inner value; `Box<T>` is a transparent
// wrapper around `T`.
impl<T: Deserialize> Transparent for Box<T> {
    type Inner = T;

    fn wrap(inner: T) -> Self {
        Box::new(inner)
    }
}

impl<T: Deserialize> Deserialize for Box<T> {
    type Visitor<'a>
        = TransparentVisitor<'a, Box<T>>
    where
        Self: 'a;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        transparent(out)
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    type Visitor<'a>
        = Place<'a, Option<T>>
    where
        Self: 'a;

    #[inline]
    fn default() -> Option<Self> {
        Some(None)
    }

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl<T: Deserialize> Visitor for Place<'_, Option<T>> {
            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                match s {
                    Scalar::Null => {
                        *self.out = Some(None);
                        Ok(())
                    }
                    _ => Deserialize::begin(self.out.insert(None)).scalar(s),
                }
            }

            fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
            where
                Self: 's,
            {
                let out = self.out;
                Box::new(Deserialize::begin(out.insert(None))).seq()
            }

            fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
            where
                Self: 's,
            {
                let out = self.out;
                Box::new(Deserialize::begin(out.insert(None))).map()
            }
        }

        Place::new(out)
    }
}

impl<A: Deserialize, B: Deserialize> Deserialize for (A, B) {
    type Visitor<'a>
        = Place<'a, (A, B)>
    where
        Self: 'a;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl<A: Deserialize, B: Deserialize> Visitor for Place<'_, (A, B)> {
            fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
            where
                Self: 's,
            {
                Ok(Box::new(TupleBuilder {
                    out: self.out,
                    tuple: (None, None),
                }))
            }
        }

        struct TupleBuilder<'a, A: 'a, B: 'a> {
            out: &'a mut Option<(A, B)>,
            tuple: (Option<A>, Option<B>),
        }

        impl<'a, A: Deserialize, B: Deserialize> Seq for TupleBuilder<'a, A, B> {
            fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
                if self.tuple.0.is_none() {
                    Ok(Box::new(Deserialize::begin(&mut self.tuple.0)))
                } else if self.tuple.1.is_none() {
                    Ok(Box::new(Deserialize::begin(&mut self.tuple.1)))
                } else {
                    Err(Error)
                }
            }

            fn finish(&mut self) -> Result<()> {
                if let (Some(a), Some(b)) = (self.tuple.0.take(), self.tuple.1.take()) {
                    *self.out = Some((a, b));
                    Ok(())
                } else {
                    Err(Error)
                }
            }

            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                if self.tuple.0.is_none() {
                    Deserialize::begin(&mut self.tuple.0).scalar(s)
                } else if self.tuple.1.is_none() {
                    Deserialize::begin(&mut self.tuple.1).scalar(s)
                } else {
                    Err(Error)
                }
            }
        }

        Place::new(out)
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    type Visitor<'a>
        = Place<'a, Vec<T>>
    where
        Self: 'a;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl<T: Deserialize> Visitor for Place<'_, Vec<T>> {
            fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
            where
                Self: 's,
            {
                Ok(Box::new(VecBuilder {
                    out: self.out,
                    vec: Vec::new(),
                    element: None,
                }))
            }
        }

        struct VecBuilder<'a, T: 'a> {
            out: &'a mut Option<Vec<T>>,
            vec: Vec<T>,
            element: Option<T>,
        }

        impl<'a, T> VecBuilder<'a, T> {
            fn shift(&mut self) {
                if let Some(e) = self.element.take() {
                    self.vec.push(e);
                }
            }
        }

        impl<'a, T: Deserialize> Seq for VecBuilder<'a, T> {
            fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
                self.shift();
                Ok(Box::new(Deserialize::begin(&mut self.element)))
            }

            fn finish(&mut self) -> Result<()> {
                self.shift();
                *self.out = Some(mem::take(&mut self.vec));
                Ok(())
            }

            fn scalar(&mut self, s: &Scalar) -> Result<()> {
                self.shift();
                Deserialize::begin(&mut self.element).scalar(s)
            }
        }

        Place::new(out)
    }
}

#[cfg(feature = "std")]
impl<K, V, H> Deserialize for HashMap<K, V, H>
where
    K: FromStr + Hash + Eq,
    V: Deserialize,
    H: BuildHasher + Default,
{
    type Visitor<'a>
        = Place<'a, HashMap<K, V, H>>
    where
        Self: 'a;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl<K, V, H> Visitor for Place<'_, HashMap<K, V, H>>
        where
            K: FromStr + Hash + Eq,
            V: Deserialize,
            H: BuildHasher + Default,
        {
            fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
            where
                Self: 's,
            {
                Ok(Box::new(MapBuilder {
                    out: self.out,
                    map: HashMap::with_hasher(H::default()),
                    key: None,
                    value: None,
                }))
            }
        }

        struct MapBuilder<'a, K: 'a, V: 'a, H: 'a> {
            out: &'a mut Option<HashMap<K, V, H>>,
            map: HashMap<K, V, H>,
            key: Option<K>,
            value: Option<V>,
        }

        impl<'a, K, V, H> MapBuilder<'a, K, V, H>
        where
            K: FromStr + Hash + Eq,
            H: BuildHasher,
        {
            fn shift(&mut self) {
                if let (Some(k), Some(v)) = (self.key.take(), self.value.take()) {
                    self.map.insert(k, v);
                }
            }

            fn parse_key(&mut self, k: &str) -> Result<()> {
                self.shift();
                self.key = Some(match K::from_str(k) {
                    Ok(key) => key,
                    Err(_) => return Err(Error),
                });
                Ok(())
            }
        }

        impl<'a, K, V, H> Map for MapBuilder<'a, K, V, H>
        where
            K: FromStr + Hash + Eq,
            V: Deserialize,
            H: BuildHasher + Default,
        {
            fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>> {
                self.parse_key(k)?;
                Ok(Box::new(Deserialize::begin(&mut self.value)))
            }

            fn finish(&mut self) -> Result<()> {
                self.shift();
                let substitute = HashMap::with_hasher(H::default());
                *self.out = Some(mem::replace(&mut self.map, substitute));
                Ok(())
            }

            fn scalar(&mut self, k: &str, s: &Scalar) -> Result<()> {
                self.parse_key(k)?;
                Deserialize::begin(&mut self.value).scalar(s)
            }
        }

        Place::new(out)
    }
}

impl<K: FromStr + Ord, V: Deserialize> Deserialize for BTreeMap<K, V> {
    type Visitor<'a>
        = Place<'a, BTreeMap<K, V>>
    where
        Self: 'a;

    fn begin(out: &mut Option<Self>) -> Self::Visitor<'_> {
        impl<K: FromStr + Ord, V: Deserialize> Visitor for Place<'_, BTreeMap<K, V>> {
            fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
            where
                Self: 's,
            {
                Ok(Box::new(MapBuilder {
                    out: self.out,
                    map: BTreeMap::new(),
                    key: None,
                    value: None,
                }))
            }
        }

        struct MapBuilder<'a, K: 'a, V: 'a> {
            out: &'a mut Option<BTreeMap<K, V>>,
            map: BTreeMap<K, V>,
            key: Option<K>,
            value: Option<V>,
        }

        impl<'a, K: FromStr + Ord, V> MapBuilder<'a, K, V> {
            fn shift(&mut self) {
                if let (Some(k), Some(v)) = (self.key.take(), self.value.take()) {
                    self.map.insert(k, v);
                }
            }

            fn parse_key(&mut self, k: &str) -> Result<()> {
                self.shift();
                self.key = Some(match K::from_str(k) {
                    Ok(key) => key,
                    Err(_) => return Err(Error),
                });
                Ok(())
            }
        }

        impl<'a, K: FromStr + Ord, V: Deserialize> Map for MapBuilder<'a, K, V> {
            fn key(&mut self, k: &str) -> Result<Box<dyn Visitor + '_>> {
                self.parse_key(k)?;
                Ok(Box::new(Deserialize::begin(&mut self.value)))
            }

            fn finish(&mut self) -> Result<()> {
                self.shift();
                *self.out = Some(mem::take(&mut self.map));
                Ok(())
            }

            fn scalar(&mut self, k: &str, s: &Scalar) -> Result<()> {
                self.parse_key(k)?;
                Deserialize::begin(&mut self.value).scalar(s)
            }
        }

        Place::new(out)
    }
}
