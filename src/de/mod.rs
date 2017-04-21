use std::io::{Cursor, Read};
use serde;
use error::ResultE;

mod bundle_visitor;
mod iter_visitor;
mod maybe_skip_comma;
mod msg_visitor;
mod osc_reader;
mod pkt_deserializer;
mod prim_deserializer;

pub use self::pkt_deserializer::OwnedPktDeserializer as Deserializer;

/// Deserialize an OSC packet from some readable device.
pub fn from_read<'de, D, R>(rd: R) -> ResultE<D>
    where R: Read, D: serde::de::Deserialize<'de>
{
    let mut de = Deserializer::new(rd);
    D::deserialize(&mut de)
}

/// Deserialize an OSC packet from a `Vec<u8>` type.
/// This is a wrapper around the `from_read` function.
pub fn from_vec<'de, T>(vec: &Vec<u8>) -> ResultE<T>
    where T: serde::de::Deserialize<'de>
{
    from_read(Cursor::new(vec))
}
