// Cast away lifetimes. This is required because without recursion all state
// needs to be held on the heap and that heap data structure ends up being self
// referential. The serialization and deserialization logic manipulates frames
// on the heap in a way that ensures all internal references are live at the
// right times.
//
// The unsafety is contained to the implementation of microserde::json and not
// exposed to Serialize and Deserialize impls, so the microserde public API
// remains entirely safe to use.
//
//     careful!(EXPR as TYPE)
//
// expands to:
//
//     std::mem::transmute::<TYPE, TYPE>(EXPR)
macro_rules! careful {
    ($($cast:tt)*) => {
        careful_impl!(() $($cast)*)
    };
}

macro_rules! careful_impl {
    (($($expr:tt)*) as $t:ty) => {{
        let expr = $($expr)*;
        unsafe { $crate::lib::mem::transmute::<$t, $t>(expr) }
    }};
    (($($expr:tt)*) $next:tt $($rest:tt)*) => {
        careful_impl!(($($expr)* $next) $($rest)*)
    };
}
