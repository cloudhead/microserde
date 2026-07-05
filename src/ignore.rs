use crate::de::{Map, Scalar, Seq, Visitor};
use crate::error::Result;
use crate::lib::Box;

impl dyn Visitor {
    // `Ignore` is a zero-sized type so no allocation happens here.
    pub fn ignore() -> Box<dyn Visitor> {
        Box::new(Ignore)
    }
}

struct Ignore;

impl Visitor for Ignore {
    fn scalar(&mut self, _s: &Scalar) -> Result<()> {
        Ok(())
    }

    fn seq<'s>(self: Box<Self>) -> Result<Box<dyn Seq + 's>>
    where
        Self: 's,
    {
        Ok(self)
    }

    fn map<'s>(self: Box<Self>) -> Result<Box<dyn Map + 's>>
    where
        Self: 's,
    {
        Ok(self)
    }
}

impl Seq for Ignore {
    fn element(&mut self) -> Result<Box<dyn Visitor + '_>> {
        Ok(<dyn Visitor>::ignore())
    }

    fn finish(&mut self) -> Result<()> {
        Ok(())
    }

    fn scalar(&mut self, _s: &Scalar) -> Result<()> {
        Ok(())
    }
}

impl Map for Ignore {
    fn key(&mut self, _k: &str) -> Result<Box<dyn Visitor + '_>> {
        Ok(<dyn Visitor>::ignore())
    }

    fn finish(&mut self) -> Result<()> {
        Ok(())
    }

    fn scalar(&mut self, _k: &str, _s: &Scalar) -> Result<()> {
        Ok(())
    }
}
