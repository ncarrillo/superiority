#[derive(Clone, Debug, PartialEq)]
pub struct BsnField {
    pub index: i128,
    pub name: Option<String>,
    pub value: BsnValue,
}

impl BsnField {
    #[must_use]
    pub fn named(index: i128, name: impl Into<String>, value: BsnValue) -> Self {
        Self {
            index,
            name: Some(name.into()),
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BsnStruct {
    pub type_id: u32,
    pub fields: Vec<BsnField>,
}

impl BsnStruct {
    #[must_use]
    pub fn new(type_id: u32, fields: Vec<BsnField>) -> Self {
        Self { type_id, fields }
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&BsnValue> {
        self.fields
            .iter()
            .find(|field| field.name.as_deref() == Some(name))
            .map(|field| &field.value)
    }

    #[must_use]
    pub fn get_index(&self, index: i128) -> Option<&BsnValue> {
        self.fields
            .iter()
            .find(|field| field.index == index)
            .map(|field| &field.value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BsnBitArray {
    pub data: Vec<u8>,
    pub bit_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BsnValue {
    Void,
    Bool(bool),
    Integer(i128),
    FourCc(u32),
    Float32(f32),
    Float64(f64),
    Bytes(Vec<u8>),
    String(String),
    BitArray(BsnBitArray),
    Array(Vec<BsnValue>),
    Optional(Option<Box<BsnValue>>),
    Choice { index: i128, value: Box<BsnValue> },
    Struct(BsnStruct),
}

impl BsnValue {
    #[must_use]
    pub fn choice(index: i128, value: BsnValue) -> Self {
        Self::Choice {
            index,
            value: Box::new(value),
        }
    }

    #[must_use]
    pub fn some(value: BsnValue) -> Self {
        Self::Optional(Some(Box::new(value)))
    }

    #[must_use]
    pub const fn none() -> Self {
        Self::Optional(None)
    }

    #[must_use]
    pub const fn as_struct(&self) -> Option<&BsnStruct> {
        if let Self::Struct(value) = self {
            Some(value)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn as_integer(&self) -> Option<i128> {
        if let Self::Integer(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        if let Self::String(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

impl From<bool> for BsnValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i128> for BsnValue {
    fn from(value: i128) -> Self {
        Self::Integer(value)
    }
}

impl From<i64> for BsnValue {
    fn from(value: i64) -> Self {
        Self::Integer(i128::from(value))
    }
}

impl From<u32> for BsnValue {
    fn from(value: u32) -> Self {
        Self::Integer(i128::from(value))
    }
}

impl From<String> for BsnValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for BsnValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<Vec<u8>> for BsnValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}
