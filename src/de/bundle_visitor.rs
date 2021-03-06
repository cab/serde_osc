use std::io::{Read, Take};
use std::mem;
use serde::de;
use serde::de::{DeserializeSeed, SeqAccess, Visitor};

use error::{Error, ResultE};
use super::iter_visitor::IterVisitor;
use super::osc_reader::OscReader;
use super::pkt_deserializer::PktDeserializer;
use super::prim_deserializer::PrimDeserializer;

/// Deserializes a single bundle, within a packet.
#[derive(Debug)]
pub struct BundleVisitor<'a, R: Read + 'a> {
    read: &'a mut Take<R>,
    state: State,
}

/// Which part of the bundle is being parsed
#[derive(Debug)]
enum State {
    /// Parsing the 64-bit OSC time tag
    TimeTag,
    /// Parsing the body of the bundle: OSC Bundle Elements
    Elements,
}

/// Struct to deserialize a single element from the OSC bundle
enum BundleField<'a, R: Read + 'a> {
    TimeTag((u32, u32)),
    Elements(&'a mut Take<R>),
}

/// Deserializes each item (message/bundle) within the bundle element sequence.
struct ElemAccessor<'a, R: Read + 'a> {
    read: &'a mut Take<R>,
}

impl<'a, R> BundleVisitor<'a, R>
    where R: Read + 'a
{
    pub fn new(read: &'a mut Take<R>) -> Self {
        Self {
            read: read,
            state: State::TimeTag,
        }
    }
}


impl<'de, 'a, R> SeqAccess<'de> for BundleVisitor<'a, R>
    where R: Read + 'a
{
    type Error = Error;
    fn next_element_seed<T>(&mut self, seed: T) -> ResultE<Option<T::Value>>
        where T: DeserializeSeed<'de>
    {
        if self.read.limit() == 0 {
            // end of bundle
            return Ok(None);
        }
        let elem = match mem::replace(&mut self.state, State::Elements) {
            State::TimeTag => BundleField::TimeTag(self.read.parse_timetag()?),
            State::Elements => BundleField::Elements(self.read),
            //State::Elements => BundleField::Packet(PktDeserializer::new(self.read)),
        };
        seed.deserialize(elem).map(Some)
    }
}


impl<'de, 'a, R> de::Deserializer<'de> for BundleField<'a, R>
    where R: Read + 'a
{
    type Error = Error;
    // deserializes a single item from the message, consuming self.
    fn deserialize_any<V>(self, visitor: V) -> ResultE<V::Value>
    where
        V: Visitor<'de>
    {
        match self {
            BundleField::TimeTag((sec, frac)) =>
                visitor.visit_seq(IterVisitor([sec, frac].into_iter().cloned()
                    .map(PrimDeserializer))),
            BundleField::Elements(mut read) =>
                visitor.visit_seq(ElemAccessor{ read }),
        }
    }

    // OSC messages are strongly typed, so we don't make use of any type hints.
    // More info: https://github.com/serde-rs/serde/blob/b7d6c5d9f7b3085a4d40a446eeb95976d2337e07/serde/src/macros.rs#L106
    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit option
        seq bytes byte_buf map unit_struct newtype_struct
        tuple_struct struct identifier tuple enum ignored_any
    }
}


impl<'de, 'a, R> SeqAccess<'de> for ElemAccessor<'a, R>
    where R: Read + 'a
{
    type Error = Error;
    fn next_element_seed<T>(&mut self, seed: T) -> ResultE<Option<T::Value>>
        where T: DeserializeSeed<'de>
    {
        // TODO: handle EOF by returning None
        seed.deserialize(&mut PktDeserializer::new(self.read)).map(Some)
    }
}
