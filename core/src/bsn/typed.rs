use crate::bsn::value::{BsnBitArray, BsnValue};
use crate::{Error, Result};

pub trait FromBsn: Sized {
    fn from_bsn(value: &BsnValue) -> Result<Self>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FourCc(pub u32);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bytes(pub Vec<u8>);

#[doc(hidden)]
#[must_use]
pub fn expected_struct(type_name: &str) -> Error {
    Error::BsnWire(format!("expected a struct value for `{type_name}`"))
}

#[doc(hidden)]
#[must_use]
pub fn missing_field(field: &str) -> Error {
    Error::BsnWire(format!("struct is missing field `{field}`"))
}

fn wrong_kind(want: &str, found: &BsnValue) -> Error {
    Error::BsnWire(format!("expected {want}, found {found:?}"))
}

macro_rules! from_bsn_int {
    ($($t:ty),*) => {$(
        impl FromBsn for $t {
            fn from_bsn(value: &BsnValue) -> Result<Self> {
                let raw = value.as_integer().ok_or_else(|| wrong_kind("integer", value))?;
                <$t>::try_from(raw).map_err(|_| {
                    Error::BsnWire(format!("integer {raw} does not fit {}", stringify!($t)))
                })
            }
        }
    )*};
}
from_bsn_int!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

impl FromBsn for () {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Void => Ok(()),
            other => Err(wrong_kind("void", other)),
        }
    }
}

impl FromBsn for bool {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Bool(flag) => Ok(*flag),
            other => Err(wrong_kind("bool", other)),
        }
    }
}

impl FromBsn for f32 {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Float32(number) => Ok(*number),
            other => Err(wrong_kind("float32", other)),
        }
    }
}

impl FromBsn for f64 {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Float64(number) => Ok(*number),
            other => Err(wrong_kind("float64", other)),
        }
    }
}

impl FromBsn for String {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        value
            .as_string()
            .map(str::to_owned)
            .ok_or_else(|| wrong_kind("string", value))
    }
}

impl FromBsn for FourCc {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::FourCc(code) => Ok(Self(*code)),
            other => Err(wrong_kind("fourcc", other)),
        }
    }
}

impl FromBsn for Bytes {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Bytes(data) => Ok(Self(data.clone())),
            other => Err(wrong_kind("bytes", other)),
        }
    }
}

impl FromBsn for BsnBitArray {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::BitArray(bits) => Ok(bits.clone()),
            other => Err(wrong_kind("bit array", other)),
        }
    }
}

impl<T: FromBsn> FromBsn for Option<T> {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Optional(None) => Ok(None),
            BsnValue::Optional(Some(inner)) => Ok(Some(T::from_bsn(inner)?)),
            other => Err(wrong_kind("optional", other)),
        }
    }
}

impl<T: FromBsn> FromBsn for Vec<T> {
    fn from_bsn(value: &BsnValue) -> Result<Self> {
        match value {
            BsnValue::Array(items) => items.iter().map(T::from_bsn).collect(),
            other => Err(wrong_kind("array", other)),
        }
    }
}
