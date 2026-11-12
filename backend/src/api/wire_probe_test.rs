//! Two probes `wire_contract_test.rs` reads a type's shape with, beyond
//! what its JSON shows.
//!
//! - [`skipped_fields`] names the fields a value's serialization left
//!   out under `skip_serializing_if`. A sample meant to show every field
//!   the wire can carry must leave none out, or a field both the sample
//!   and the renderer's type forget would pass unseen.
//! - [`accepted_fields`] names every field a request type reads, from
//!   the list serde's derive hands `deserialize_struct`.
//!
//! Both read what serde's derive does, not the source: a field marked
//! `#[serde(skip)]` or `skip_serializing` never reaches a serializer and
//! is not on the wire, so neither probe sees it, and neither needs to.
//! A struct that serializes through `#[serde(flatten)]` is written as a
//! map, whose skipped fields serde does not report: [`skipped_fields`]
//! sees only what such a value writes.

use std::fmt;

use serde::de::{self, DeserializeOwned, Visitor};
use serde::ser::{self, Impossible, Serialize, SerializeMap, SerializeStruct};

#[derive(Debug)]
pub(super) struct ProbeError(String);

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProbeError {}

impl ser::Error for ProbeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

impl de::Error for ProbeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

fn not_a_struct<T>() -> Result<T, ProbeError> {
    Err(ProbeError("the probe reads structs and maps only".into()))
}

/// The fields `value`'s serialization skipped. Panics on a value that
/// does not serialize as a struct or a map.
pub(super) fn skipped_fields<T: Serialize>(value: &T) -> Vec<&'static str> {
    value
        .serialize(Recorder)
        .expect("the value serializes as a struct or a map")
}

struct Recorder;

#[derive(Default)]
struct Skipped(Vec<&'static str>);

impl SerializeStruct for Skipped {
    type Ok = Vec<&'static str>;
    type Error = ProbeError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), ProbeError> {
        Ok(())
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), ProbeError> {
        self.0.push(key);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, ProbeError> {
        Ok(self.0)
    }
}

impl SerializeMap for Skipped {
    type Ok = Vec<&'static str>;
    type Error = ProbeError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, _key: &T) -> Result<(), ProbeError> {
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, _value: &T) -> Result<(), ProbeError> {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, ProbeError> {
        Ok(self.0)
    }
}

/// Answers every non-struct shape with an error.
macro_rules! refuse {
    ($($name:ident($($arg:ty),*);)*) => {
        $(fn $name(self, $(_: $arg),*) -> Result<Self::Ok, ProbeError> { not_a_struct() })*
    };
}

impl ser::Serializer for Recorder {
    type Ok = Vec<&'static str>;
    type Error = ProbeError;
    type SerializeSeq = Impossible<Self::Ok, ProbeError>;
    type SerializeTuple = Impossible<Self::Ok, ProbeError>;
    type SerializeTupleStruct = Impossible<Self::Ok, ProbeError>;
    type SerializeTupleVariant = Impossible<Self::Ok, ProbeError>;
    type SerializeMap = Skipped;
    type SerializeStruct = Skipped;
    type SerializeStructVariant = Impossible<Self::Ok, ProbeError>;

    refuse! {
        serialize_bool(bool);
        serialize_i8(i8);
        serialize_i16(i16);
        serialize_i32(i32);
        serialize_i64(i64);
        serialize_u8(u8);
        serialize_u16(u16);
        serialize_u32(u32);
        serialize_u64(u64);
        serialize_f32(f32);
        serialize_f64(f64);
        serialize_char(char);
        serialize_str(&str);
        serialize_bytes(&[u8]);
        serialize_none();
        serialize_unit();
        serialize_unit_struct(&'static str);
        serialize_unit_variant(&'static str, u32, &'static str);
    }

    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok, ProbeError> {
        not_a_struct()
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, ProbeError> {
        not_a_struct()
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, ProbeError> {
        not_a_struct()
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, ProbeError> {
        not_a_struct()
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, ProbeError> {
        not_a_struct()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, ProbeError> {
        not_a_struct()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, ProbeError> {
        not_a_struct()
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, ProbeError> {
        Ok(Skipped::default())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, ProbeError> {
        Ok(Skipped::default())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, ProbeError> {
        not_a_struct()
    }
}

/// Every field `T` reads from a request body, as serde's derive lists
/// them. Panics on a type that does not deserialize as a struct.
pub(super) fn accepted_fields<T: DeserializeOwned>() -> Vec<&'static str> {
    let mut fields: &'static [&'static str] = &[];
    let _ = T::deserialize(FieldList(&mut fields));
    assert!(
        !fields.is_empty(),
        "{} does not deserialize as a struct",
        std::any::type_name::<T>()
    );
    fields.to_vec()
}

struct FieldList<'a>(&'a mut &'static [&'static str]);

impl<'de> de::Deserializer<'de> for FieldList<'_> {
    type Error = ProbeError;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, ProbeError> {
        not_a_struct()
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, ProbeError> {
        *self.0 = fields;
        Err(ProbeError("listed".into()))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}
