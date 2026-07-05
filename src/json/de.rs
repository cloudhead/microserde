use crate::lib::char;
use crate::lib::mem;
use crate::lib::str;
use crate::lib::{Box, Vec};

use crate::de::Scalar::{Float, Negative, Nonnegative};
use crate::de::{Deserialize, Map, Scalar, Seq, Visitor};
use crate::error::{Error, Result};

/// Deserialize a JSON string into any deserializable type.
///
/// ```rust
/// use microserde::{json, Deserialize};
///
/// #[derive(Deserialize, Debug)]
/// struct Example {
///     code: u32,
///     message: String,
/// }
///
/// fn main() -> microserde::Result<()> {
///     let j = r#" {"code": 200, "message": "reminiscent of Serde"} "#;
///
///     let out: Example = json::from_str(&j)?;
///     println!("{:?}", out);
///
///     Ok(())
/// }
/// ```
pub fn from_str<T: Deserialize>(j: &str) -> Result<T> {
    let mut out = None;
    from_str_impl(j, Box::new(T::begin(&mut out)))?;
    out.ok_or(Error)
}

struct Deserializer<'a, 'b> {
    cursor: Cursor<'a>,
    /// Scratch space for value strings containing escape sequences.
    buffer: Vec<u8>,
    /// Scratch space for map keys containing escape sequences. Separate from
    /// `buffer` so that a key and the scalar value that follows it can be
    /// borrowed at the same time.
    key_buffer: Vec<u8>,
    stack: Vec<Layer<'b>>,
}

enum Layer<'a> {
    Seq(Box<dyn Seq + 'a>),
    Map(Box<dyn Map + 'a>),
}

impl<'a> Layer<'a> {
    /// Opens the sequence or map started by the given event, writing into the
    /// given visitor.
    fn open(visitor: Box<dyn Visitor + 'a>, event: &Event) -> Result<Layer<'a>> {
        match event {
            Event::SeqStart => Ok(Layer::Seq(visitor.seq()?)),
            Event::MapStart => Ok(Layer::Map(visitor.map()?)),
            Event::Scalar(_) => Err(Error),
        }
    }
}

impl<'a, 'b> Drop for Deserializer<'a, 'b> {
    fn drop(&mut self) {
        // Drop layers in reverse order: each layer's builder may borrow from
        // the builders below it on the stack.
        while !self.stack.is_empty() {
            self.stack.pop();
        }
    }
}

enum Event<'a> {
    Scalar(Scalar<'a>),
    SeqStart,
    MapStart,
}

fn from_str_impl(j: &str, visitor: Box<dyn Visitor + '_>) -> Result<()> {
    let mut visitor = careful!(visitor as Box<dyn Visitor>);
    let mut de = Deserializer {
        cursor: Cursor {
            input: j.as_bytes(),
            pos: 0,
        },
        buffer: Vec::new(),
        key_buffer: Vec::new(),
        stack: Vec::new(),
    };

    // Read the root value.
    let event = de.cursor.event(&mut de.buffer)?;
    let mut layer = match &event {
        Event::Scalar(s) => {
            visitor.scalar(s)?;
            return de.cursor.expect_eof();
        }
        event => Layer::open(visitor, event)?,
    };
    let mut accept_comma = false;

    loop {
        // Handle commas and closing brackets.
        loop {
            match de.cursor.parse_whitespace().unwrap_or(b'\0') {
                b',' if accept_comma => {
                    de.cursor.bump();
                    break;
                }
                close @ b']' | close @ b'}' => {
                    de.cursor.bump();
                    match &mut layer {
                        Layer::Seq(seq) if close == b']' => seq.finish()?,
                        Layer::Map(map) if close == b'}' => map.finish()?,
                        _ => return Err(Error),
                    };
                    // The assignment drops the finished layer before its
                    // parent, preserving reverse drop order.
                    layer = match de.stack.pop() {
                        Some(parent) => parent,
                        None => return de.cursor.expect_eof(),
                    };
                    accept_comma = true;
                }
                _ => {
                    if accept_comma {
                        return Err(Error);
                    } else {
                        break;
                    }
                }
            }
        }

        // Read one element or entry into the current layer. Scalars are
        // written directly through the layer; sequences and maps open a new
        // layer that becomes the current one.
        let inner = match &mut layer {
            Layer::Seq(seq) => {
                let event = de.cursor.event(&mut de.buffer)?;
                match &event {
                    Event::Scalar(s) => {
                        seq.scalar(s)?;
                        None
                    }
                    event => {
                        let visitor = careful!(seq.element()? as Box<dyn Visitor>);
                        Some(Layer::open(visitor, event)?)
                    }
                }
            }
            Layer::Map(map) => {
                match de.cursor.parse_whitespace() {
                    Some(b'"') => de.cursor.bump(),
                    _ => return Err(Error),
                }
                let k = de.cursor.parse_str(&mut de.key_buffer)?;
                match de.cursor.parse_whitespace() {
                    Some(b':') => de.cursor.bump(),
                    _ => return Err(Error),
                }
                let event = de.cursor.event(&mut de.buffer)?;
                match &event {
                    Event::Scalar(s) => {
                        map.scalar(k, s)?;
                        None
                    }
                    event => {
                        let visitor = careful!(map.key(k)? as Box<dyn Visitor>);
                        Some(Layer::open(visitor, event)?)
                    }
                }
            }
        };

        match inner {
            None => accept_comma = true,
            Some(inner) => {
                de.stack.push(mem::replace(&mut layer, inner));
                accept_comma = false;
            }
        }
    }
}

macro_rules! overflow {
    ($a:ident * 10 + $b:ident, $c:expr) => {
        $a >= $c / 10 && ($a > $c / 10 || $b > $c % 10)
    };
}

struct Cursor<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn next(&mut self) -> Option<u8> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn next_or_nul(&mut self) -> u8 {
        self.next().unwrap_or(b'\0')
    }

    fn peek(&mut self) -> Option<u8> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    fn peek_or_nul(&mut self) -> u8 {
        self.peek().unwrap_or(b'\0')
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    /// Fails if anything other than whitespace remains.
    fn expect_eof(&mut self) -> Result<()> {
        match self.parse_whitespace() {
            Some(_) => Err(Error),
            None => Ok(()),
        }
    }

    fn parse_str<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<&'s str>
    where
        'a: 's,
    {
        fn result(bytes: &[u8]) -> &str {
            // The input is assumed to be valid UTF-8 and the \u-escapes are
            // checked along the way, so don't need to check here.
            unsafe { str::from_utf8_unchecked(bytes) }
        }

        let input = self.input;
        // Index of the first byte not yet copied into the scratch space.
        let mut start = self.pos;
        scratch.clear();

        loop {
            while self.pos < input.len() && !ESCAPE[usize::from(input[self.pos])] {
                self.pos += 1;
            }
            if self.pos == input.len() {
                return Err(Error);
            }
            match input[self.pos] {
                b'"' => {
                    if scratch.is_empty() {
                        // Fast path: return a slice of the raw JSON without any
                        // copying.
                        let borrowed = &input[start..self.pos];
                        self.pos += 1;
                        return Ok(result(borrowed));
                    } else {
                        scratch.extend_from_slice(&input[start..self.pos]);
                        self.pos += 1;
                        return Ok(result(scratch));
                    }
                }
                b'\\' => {
                    scratch.extend_from_slice(&input[start..self.pos]);
                    self.pos += 1;
                    self.parse_escape(scratch)?;
                    start = self.pos;
                }
                _ => {
                    return Err(Error);
                }
            }
        }
    }

    fn next_or_eof(&mut self) -> Result<u8> {
        self.next().ok_or(Error)
    }

    /// Parses a JSON escape sequence and appends it into the scratch space. Assumes
    /// the previous byte read was a backslash.
    fn parse_escape(&mut self, scratch: &mut Vec<u8>) -> Result<()> {
        let ch = self.next_or_eof()?;

        match ch {
            b'"' => scratch.push(b'"'),
            b'\\' => scratch.push(b'\\'),
            b'/' => scratch.push(b'/'),
            b'b' => scratch.push(b'\x08'),
            b'f' => scratch.push(b'\x0c'),
            b'n' => scratch.push(b'\n'),
            b'r' => scratch.push(b'\r'),
            b't' => scratch.push(b'\t'),
            b'u' => {
                let c = match self.decode_hex_escape()? {
                    0xDC00..=0xDFFF => {
                        return Err(Error);
                    }

                    // Non-BMP characters are encoded as a sequence of
                    // two hex escapes, representing UTF-16 surrogates.
                    n1 @ 0xD800..=0xDBFF => {
                        if self.next_or_eof()? != b'\\' {
                            return Err(Error);
                        }
                        if self.next_or_eof()? != b'u' {
                            return Err(Error);
                        }

                        let n2 = self.decode_hex_escape()?;

                        if n2 < 0xDC00 || n2 > 0xDFFF {
                            return Err(Error);
                        }

                        let n = (u32::from(n1 - 0xD800) << 10 | u32::from(n2 - 0xDC00)) + 0x1_0000;

                        match char::from_u32(n) {
                            Some(c) => c,
                            None => {
                                return Err(Error);
                            }
                        }
                    }

                    n => match char::from_u32(u32::from(n)) {
                        Some(c) => c,
                        None => {
                            return Err(Error);
                        }
                    },
                };

                scratch.extend_from_slice(c.encode_utf8(&mut [0_u8; 4]).as_bytes());
            }
            _ => {
                return Err(Error);
            }
        }

        Ok(())
    }

    fn decode_hex_escape(&mut self) -> Result<u16> {
        let mut n = 0;
        for _ in 0..4 {
            n = match self.next_or_eof()? {
                c @ b'0'..=b'9' => n * 16_u16 + u16::from(c - b'0'),
                b'a' | b'A' => n * 16_u16 + 10_u16,
                b'b' | b'B' => n * 16_u16 + 11_u16,
                b'c' | b'C' => n * 16_u16 + 12_u16,
                b'd' | b'D' => n * 16_u16 + 13_u16,
                b'e' | b'E' => n * 16_u16 + 14_u16,
                b'f' | b'F' => n * 16_u16 + 15_u16,
                _ => {
                    return Err(Error);
                }
            };
        }
        Ok(n)
    }

    fn parse_whitespace(&mut self) -> Option<u8> {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\n') | Some(b'\t') | Some(b'\r') => {
                    self.bump();
                }
                other => {
                    return other;
                }
            }
        }
    }

    fn parse_ident(&mut self, ident: &[u8]) -> Result<()> {
        for expected in ident {
            match self.next() {
                None => {
                    return Err(Error);
                }
                Some(next) => {
                    if next != *expected {
                        return Err(Error);
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_integer(&mut self, nonnegative: bool, first_digit: u8) -> Result<Scalar<'a>> {
        match first_digit {
            b'0' => {
                // There can be only one leading '0'.
                match self.peek_or_nul() {
                    b'0'..=b'9' => Err(Error),
                    _ => self.parse_number(nonnegative, 0),
                }
            }
            c @ b'1'..=b'9' => {
                let mut res = u64::from(c - b'0');

                loop {
                    match self.peek_or_nul() {
                        c @ b'0'..=b'9' => {
                            self.bump();
                            let digit = u64::from(c - b'0');

                            // We need to be careful with overflow. If we can, try to keep the
                            // number as a `u64` until we grow too large. At that point, switch to
                            // parsing the value as a `f64`.
                            if overflow!(res * 10 + digit, u64::MAX) {
                                return self
                                    .parse_long_integer(
                                        nonnegative,
                                        res,
                                        1, // res * 10^1
                                    )
                                    .map(Float);
                            }

                            res = res * 10 + digit;
                        }
                        _ => {
                            return self.parse_number(nonnegative, res);
                        }
                    }
                }
            }
            _ => Err(Error),
        }
    }

    fn parse_long_integer(
        &mut self,
        nonnegative: bool,
        significand: u64,
        mut exponent: i32,
    ) -> Result<f64> {
        loop {
            match self.peek_or_nul() {
                b'0'..=b'9' => {
                    self.bump();
                    // This could overflow... if your integer is gigabytes long.
                    // Ignore that possibility.
                    exponent += 1;
                }
                b'.' => {
                    return self.parse_decimal(nonnegative, significand, exponent);
                }
                b'e' | b'E' => {
                    return self.parse_exponent(nonnegative, significand, exponent);
                }
                _ => {
                    return f64_from_parts(nonnegative, significand, exponent);
                }
            }
        }
    }

    fn parse_number(&mut self, nonnegative: bool, significand: u64) -> Result<Scalar<'a>> {
        match self.peek_or_nul() {
            b'.' => self.parse_decimal(nonnegative, significand, 0).map(Float),
            b'e' | b'E' => self.parse_exponent(nonnegative, significand, 0).map(Float),
            _ => {
                Ok(if nonnegative {
                    Nonnegative(significand)
                } else {
                    let neg = (significand as i64).wrapping_neg();

                    // Convert into a float if we underflow.
                    if neg > 0 {
                        Float(-(significand as f64))
                    } else {
                        Negative(neg)
                    }
                })
            }
        }
    }

    fn parse_decimal(
        &mut self,
        nonnegative: bool,
        mut significand: u64,
        mut exponent: i32,
    ) -> Result<f64> {
        self.bump();

        let mut at_least_one_digit = false;
        while let c @ b'0'..=b'9' = self.peek_or_nul() {
            self.bump();
            let digit = u64::from(c - b'0');
            at_least_one_digit = true;

            if overflow!(significand * 10 + digit, u64::MAX) {
                // The next multiply/add would overflow, so just ignore all
                // further digits.
                while let b'0'..=b'9' = self.peek_or_nul() {
                    self.bump();
                }
                break;
            }

            significand = significand * 10 + digit;
            exponent -= 1;
        }

        if !at_least_one_digit {
            return Err(Error);
        }

        match self.peek_or_nul() {
            b'e' | b'E' => self.parse_exponent(nonnegative, significand, exponent),
            _ => f64_from_parts(nonnegative, significand, exponent),
        }
    }

    fn parse_exponent(
        &mut self,
        nonnegative: bool,
        significand: u64,
        starting_exp: i32,
    ) -> Result<f64> {
        self.bump();

        let positive_exp = match self.peek_or_nul() {
            b'+' => {
                self.bump();
                true
            }
            b'-' => {
                self.bump();
                false
            }
            _ => true,
        };

        // Make sure a digit follows the exponent place.
        let mut exp = match self.next_or_nul() {
            c @ b'0'..=b'9' => i32::from(c - b'0'),
            _ => {
                return Err(Error);
            }
        };

        while let c @ b'0'..=b'9' = self.peek_or_nul() {
            self.bump();
            let digit = i32::from(c - b'0');

            if overflow!(exp * 10 + digit, i32::MAX) {
                return self.parse_exponent_overflow(nonnegative, significand, positive_exp);
            }

            exp = exp * 10 + digit;
        }

        let final_exp = if positive_exp {
            starting_exp.saturating_add(exp)
        } else {
            starting_exp.saturating_sub(exp)
        };

        f64_from_parts(nonnegative, significand, final_exp)
    }

    // This cold code should not be inlined into the middle of the hot
    // exponent-parsing loop above.
    #[cold]
    #[inline(never)]
    fn parse_exponent_overflow(
        &mut self,
        nonnegative: bool,
        significand: u64,
        positive_exp: bool,
    ) -> Result<f64> {
        // Error instead of +/- infinity.
        if significand != 0 && positive_exp {
            return Err(Error);
        }

        while let b'0'..=b'9' = self.peek_or_nul() {
            self.bump();
        }
        Ok(if nonnegative { 0.0 } else { -0.0 })
    }

    fn event<'s>(&mut self, scratch: &'s mut Vec<u8>) -> Result<Event<'s>>
    where
        'a: 's,
    {
        let peek = match self.parse_whitespace() {
            Some(b) => b,
            None => return Err(Error),
        };
        self.bump();
        match peek {
            b'"' => self
                .parse_str(scratch)
                .map(|s| Event::Scalar(Scalar::Str(s))),
            digit @ b'0'..=b'9' => self.parse_integer(true, digit).map(Event::Scalar),
            b'-' => {
                let first_digit = self.next_or_nul();
                self.parse_integer(false, first_digit).map(Event::Scalar)
            }
            b'{' => Ok(Event::MapStart),
            b'[' => Ok(Event::SeqStart),
            b'n' => {
                self.parse_ident(b"ull")?;
                Ok(Event::Scalar(Scalar::Null))
            }
            b't' => {
                self.parse_ident(b"rue")?;
                Ok(Event::Scalar(Scalar::Bool(true)))
            }
            b'f' => {
                self.parse_ident(b"alse")?;
                Ok(Event::Scalar(Scalar::Bool(false)))
            }
            _ => Err(Error),
        }
    }
}

fn f64_from_parts(nonnegative: bool, significand: u64, mut exponent: i32) -> Result<f64> {
    let mut f = significand as f64;
    loop {
        match POW10.get(exponent.unsigned_abs() as usize) {
            Some(&pow) => {
                if exponent >= 0 {
                    f *= pow;
                    if f.is_infinite() {
                        return Err(Error);
                    }
                } else {
                    f /= pow;
                }
                break;
            }
            None => {
                if f == 0.0 {
                    break;
                }
                if exponent >= 0 {
                    return Err(Error);
                }
                f /= 1e308;
                exponent += 308;
            }
        }
    }
    Ok(if nonnegative { f } else { -f })
}

// Clippy bug: https://github.com/rust-lang/rust-clippy/issues/5201
#[allow(clippy::excessive_precision)]
static POW10: [f64; 309] = [
    1e000, 1e001, 1e002, 1e003, 1e004, 1e005, 1e006, 1e007, 1e008, 1e009, //
    1e010, 1e011, 1e012, 1e013, 1e014, 1e015, 1e016, 1e017, 1e018, 1e019, //
    1e020, 1e021, 1e022, 1e023, 1e024, 1e025, 1e026, 1e027, 1e028, 1e029, //
    1e030, 1e031, 1e032, 1e033, 1e034, 1e035, 1e036, 1e037, 1e038, 1e039, //
    1e040, 1e041, 1e042, 1e043, 1e044, 1e045, 1e046, 1e047, 1e048, 1e049, //
    1e050, 1e051, 1e052, 1e053, 1e054, 1e055, 1e056, 1e057, 1e058, 1e059, //
    1e060, 1e061, 1e062, 1e063, 1e064, 1e065, 1e066, 1e067, 1e068, 1e069, //
    1e070, 1e071, 1e072, 1e073, 1e074, 1e075, 1e076, 1e077, 1e078, 1e079, //
    1e080, 1e081, 1e082, 1e083, 1e084, 1e085, 1e086, 1e087, 1e088, 1e089, //
    1e090, 1e091, 1e092, 1e093, 1e094, 1e095, 1e096, 1e097, 1e098, 1e099, //
    1e100, 1e101, 1e102, 1e103, 1e104, 1e105, 1e106, 1e107, 1e108, 1e109, //
    1e110, 1e111, 1e112, 1e113, 1e114, 1e115, 1e116, 1e117, 1e118, 1e119, //
    1e120, 1e121, 1e122, 1e123, 1e124, 1e125, 1e126, 1e127, 1e128, 1e129, //
    1e130, 1e131, 1e132, 1e133, 1e134, 1e135, 1e136, 1e137, 1e138, 1e139, //
    1e140, 1e141, 1e142, 1e143, 1e144, 1e145, 1e146, 1e147, 1e148, 1e149, //
    1e150, 1e151, 1e152, 1e153, 1e154, 1e155, 1e156, 1e157, 1e158, 1e159, //
    1e160, 1e161, 1e162, 1e163, 1e164, 1e165, 1e166, 1e167, 1e168, 1e169, //
    1e170, 1e171, 1e172, 1e173, 1e174, 1e175, 1e176, 1e177, 1e178, 1e179, //
    1e180, 1e181, 1e182, 1e183, 1e184, 1e185, 1e186, 1e187, 1e188, 1e189, //
    1e190, 1e191, 1e192, 1e193, 1e194, 1e195, 1e196, 1e197, 1e198, 1e199, //
    1e200, 1e201, 1e202, 1e203, 1e204, 1e205, 1e206, 1e207, 1e208, 1e209, //
    1e210, 1e211, 1e212, 1e213, 1e214, 1e215, 1e216, 1e217, 1e218, 1e219, //
    1e220, 1e221, 1e222, 1e223, 1e224, 1e225, 1e226, 1e227, 1e228, 1e229, //
    1e230, 1e231, 1e232, 1e233, 1e234, 1e235, 1e236, 1e237, 1e238, 1e239, //
    1e240, 1e241, 1e242, 1e243, 1e244, 1e245, 1e246, 1e247, 1e248, 1e249, //
    1e250, 1e251, 1e252, 1e253, 1e254, 1e255, 1e256, 1e257, 1e258, 1e259, //
    1e260, 1e261, 1e262, 1e263, 1e264, 1e265, 1e266, 1e267, 1e268, 1e269, //
    1e270, 1e271, 1e272, 1e273, 1e274, 1e275, 1e276, 1e277, 1e278, 1e279, //
    1e280, 1e281, 1e282, 1e283, 1e284, 1e285, 1e286, 1e287, 1e288, 1e289, //
    1e290, 1e291, 1e292, 1e293, 1e294, 1e295, 1e296, 1e297, 1e298, 1e299, //
    1e300, 1e301, 1e302, 1e303, 1e304, 1e305, 1e306, 1e307, 1e308,
];

const CT: bool = true; // control character \x00..=\x1F
const QU: bool = true; // quote \x22
const BS: bool = true; // backslash \x5C
const O: bool = false; // allow unescaped

// Lookup table of bytes that must be escaped. A value of true at index i means
// that byte i requires an escape sequence in the input.
#[rustfmt::skip]
static ESCAPE: [bool; 256] = [
    //   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, // 0
    CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, CT, // 1
     O,  O, QU,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 2
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 3
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 4
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, BS,  O,  O,  O, // 5
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 6
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 7
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 8
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // 9
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // A
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // B
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // C
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // D
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // E
     O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O,  O, // F
];
