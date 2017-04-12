/// Deserialization

use byteorder::{BigEndian, ReadBytesExt};

use std;
use std::fmt;
use std::fmt::Display;
use std::io;
use std::io::Read;
use std::vec;
use serde::de;
use serde::de::Visitor;

enum OscArg {
    /// 32-bit signed integer
    i(i32),
    /// 32-bit float
    f(f32),
    /// String; specified as null-terminated ascii.
    /// This might also represent the message address pattern (aka path)
    s(String),
    /// 'blob' (binary) data
    b(Vec<u8>),
}

struct OscDeserializer<R> {
    read: R,
    state: State,
}

/// Which part of the OSC message is being parsed
enum State {
    /// Deserializing the address pattern.
    Address,
    /// Deserializing the typestring.
    Typestring,
    /// Deserializing the argument data.
    /// Each entry in the Vec is the typecode we parsed earlier
    /// We store this as an iterator to avoid tracking the index of the current arg.
    Arguments(vec::IntoIter<u8>),
}

#[derive(Debug)]
enum Error {
    /// User provided error message (via serde::de::Error::custom)
    Message(String),
    /// Unknown argument type (i.e. not a 'f'=f32, 'i'=i32, etc)
    UnknownType(u8),
    /// Attempt to read more arguments than were in the typestring
    ArgMiscount,
    /// OSC expects all data to be aligned to 4 bytes lengths.
    /// Likely violators of this are strings, especially those at the end of a packet.
    BadPadding,
    /// Error encountered due to std::io::Read
    Io(io::Error),
    /// We store ascii strings as UTF-8.
    /// Technically, this is safe, but if we received non-ascii data, we could have invalid UTF-8
    StrParseError(std::string::FromUtf8Error),
}

/// Alias for a 'Result' with the error type 'serde_osc::de::Error'
type ResultE<T> = Result<T, Error>;


impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "serde_osc::de::Error")
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Message(ref msg) => msg,
            _ => "Unknown serde_osc::de::Error",
        }
    }
    fn cause(&self) -> Option<&std::error::Error> {
        None
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}


impl<R> OscDeserializer<R>
    where R: Read
{
    pub fn new(read: R) -> Self {
        Self {
            read: read,
            state: State::Address,
        }
    }
    /// Strings in OSC are ascii and null-terminated.
    /// Strict specification is 1-4 null terminators, to make them end on a 4-byte boundary.
    fn read_0term_bytes(&mut self) -> ResultE<Vec<u8>> {
        let mut data = Vec::new();
        // Because of the 4-byte required padding, we can process 4 characters at a time
        let mut buf: [u8; 4] = [0, 0, 0, 0];
        while true {
            match self.read.read_exact(&mut buf) {
                Err(err) => return Err(Error::Io(err)),
                Ok(_) => {
                    // Copy the NON-NULL characters to the buffer.
                    let num_zeros = buf.iter().filter(|c| **c == 0).count();
                    if buf[4-num_zeros..4].iter().any(|c| *c != 0) {
                        // We had data after the null terminator.
                        return Err(Error::BadPadding);
                    }
                    data.extend_from_slice(&buf[0..4-num_zeros]);
                },
            }
        }
        Ok(data)
    }
    fn parse_str(&mut self) -> ResultE<String> {
        // Note: although OSC specifies ascii only, we may have data >= 128 in the vector.
        // We can safely assume a UTF-8 encoding, because no byte of any multibyte UTF-8
        // contains a zero; the only zero possible in a UTF-8 string is the ASCII zero.
        // See the UTF-8 table here: https://en.wikipedia.org/wiki/UTF-8#History
        let bytes = self.read_0term_bytes()?;
        String::from_utf8(bytes).map_err(|err| {
            Error::StrParseError(err)
        })
    }
    fn parse_typetag(&mut self) -> ResultE<vec::IntoIter<u8>> {
        // The type tag is a string type, with 4-byte null padding.
        self.read_0term_bytes().map(|bytes| bytes.into_iter())
    }

    fn parse_next(&mut self) -> ResultE<OscArg> {
        let typetag = match self.state {
            State::Address => {
                let address = self.parse_str()?;
                // Successfully parsed the address component; advance to the typestring.
                self.state = State::Typestring;
                return Ok(OscArg::s(address));
            },
            State::Typestring => {
                // parse the type tag
                let mut tags = self.parse_typetag()?;
                let parsed = self.parse_arg(tags.next())?;
                self.state = State::Arguments(tags);
                return Ok(parsed);
            },
            State::Arguments(ref mut tags) => {
                // Because parse_arg borrows self as mut, we need to do this weird
                // thing where we pop the typetag here, and then call parse_arg OUTSIDE
                tags.next()
            },
        };
        let parsed = self.parse_arg(typetag)?;
        return Ok(parsed);
    }
    fn parse_arg(&mut self, typecode: Option<u8>) -> ResultE<OscArg> {
        match typecode {
            Some(b'i') => self.parse_i32().map(|i| { OscArg::i(i) }),
            Some(b'f') => self.parse_f32().map(|f| { OscArg::f(f) }),
            Some(b's') => self.parse_str().map(|s| { OscArg::s(s) }),
            Some(b'b') => self.parse_blob().map(|b| { OscArg::b(b) }),
            Some(c) => Err(Error::UnknownType(c)),
            None => Err(Error::ArgMiscount),
        }
    }
    fn parse_i32(&mut self) -> ResultE<i32> {
        self.read.read_i32::<BigEndian>().map_err(|err| {
            Error::Io(err)
        })
    }
    fn parse_f32(&mut self) -> ResultE<f32> {
        self.read.read_f32::<BigEndian>().map_err(|err| {
            Error::Io(err)
        })
    }
    fn parse_blob(&mut self) -> ResultE<Vec<u8>> {
        unimplemented!()
    }
}

impl<'a, R> de::Deserializer for &'a mut OscDeserializer<R>
    where R: Read
{
    type Error = Error;
    fn deserialize<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor
    {
        let value = self.parse_next()?;
        match value {
            OscArg::i(i) => visitor.visit_i32(i),
            OscArg::f(f) => visitor.visit_f32(f),
            OscArg::s(s) => visitor.visit_string(s),
            OscArg::b(b) => unimplemented!(),
        }
    }
    fn deserialize_bool<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_u8<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_u16<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_u32<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_u64<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_i8<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_i16<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_i32<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_i64<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_f32<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_f64<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_char<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_str<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_string<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_bytes<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_byte_buf<V>(
        self,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_option<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_unit<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_seq<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_seq_fixed_size<V>(
        self,
        len: usize,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_tuple<V>(
        self,
        len: usize,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_map<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_struct_field<V>(
        self,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
    fn deserialize_ignored_any<V>(
        self,
        visitor: V
    ) -> ResultE<V::Value>
    where
        V: Visitor { unimplemented!() }
}
