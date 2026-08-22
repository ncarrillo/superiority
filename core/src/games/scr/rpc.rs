// the rpc dispatcher in libClientSdk.dylib consumes:
//
//     uint16_be header_size
//     classic.protocol.Header   (header_size bytes, protobuf lite)
//     request or response body  (protobuf lite)
//
// the header carries no descriptor, so the field numbers below come from its
// generated parser and were confirmed against 91 real messages.

use crate::{
    Error, Result,
    platform::wire::raw::{self as protobuf, Message, Value},
};

mod field {
    pub const SERVICE_ID: u32 = 1;
    pub const METHOD_ID: u32 = 2;
    pub const TOKEN: u32 = 3;
    pub const ROUTING_ID: u32 = 4;
    pub const SIZE: u32 = 5;
    pub const OBJECT_ID: u32 = 6;
    pub const IS_RESPONSE: u32 = 9;
    pub const REQUEST_TRACE: u32 = 12;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    pub service_id: u32,
    pub method_id: u32,
    pub token: u32,
    pub routing_id: Option<u32>,
    pub size: Option<u32>,
    pub object_id: Option<u64>,
    pub is_response: Option<bool>,
    pub request_trace: Option<Vec<u8>>,

    // fields this client does not model, appended verbatim on re-encode.
    pub unknown: Vec<u8>,
}

impl Header {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut header = Self::default();
        let (mut has_service, mut has_method, mut has_token) = (false, false, false);
        let mut unknown = Message::new();

        for item in protobuf::fields(data) {
            let item = item?;
            match (item.number, item.value) {
                (field::SERVICE_ID, Value::Varint(value)) => {
                    header.service_id = narrow(value, "service ID")?;
                    has_service = true;
                }
                (field::METHOD_ID, Value::Varint(value)) => {
                    header.method_id = narrow(value, "method ID")?;
                    has_method = true;
                }
                (field::TOKEN, Value::Varint(value)) => {
                    header.token = narrow(value, "token")?;
                    has_token = true;
                }
                (field::ROUTING_ID, Value::Varint(value)) => {
                    header.routing_id = Some(narrow(value, "routing ID")?);
                }
                (field::SIZE, Value::Varint(value)) => {
                    header.size = Some(narrow(value, "body size")?);
                }
                (field::OBJECT_ID, Value::Varint(value)) => header.object_id = Some(value),
                (field::IS_RESPONSE, Value::Varint(value)) => header.is_response = Some(value != 0),
                (field::REQUEST_TRACE, Value::Bytes(value)) => {
                    header.request_trace = Some(value.to_vec());
                }
                _ => unknown = unknown.field(&item),
            }
        }

        let missing: Vec<&str> = [
            (has_service, "service_id"),
            (has_method, "method_id"),
            (has_token, "token"),
        ]
        .into_iter()
        .filter_map(|(present, name)| (!present).then_some(name))
        .collect();
        if !missing.is_empty() {
            return Err(classic_error(format!(
                "classic header is missing required fields {missing:?}"
            )));
        }

        header.unknown = unknown.into_vec();
        Ok(header)
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut message = Message::new()
            .varint(field::SERVICE_ID, u64::from(self.service_id))
            .varint(field::METHOD_ID, u64::from(self.method_id))
            .varint(field::TOKEN, u64::from(self.token));
        if let Some(routing_id) = self.routing_id {
            message = message.varint(field::ROUTING_ID, u64::from(routing_id));
        }
        if let Some(size) = self.size {
            message = message.varint(field::SIZE, u64::from(size));
        }
        if let Some(object_id) = self.object_id {
            message = message.varint(field::OBJECT_ID, object_id);
        }
        if let Some(is_response) = self.is_response {
            message = message.varint(field::IS_RESPONSE, u64::from(is_response));
        }
        if let Some(trace) = &self.request_trace {
            message = message.bytes(field::REQUEST_TRACE, trace);
        }
        let mut encoded = message.into_vec();
        encoded.extend_from_slice(&self.unknown);
        encoded
    }

    #[must_use]
    pub fn is_response(&self) -> bool {
        self.is_response.unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub header: Header,
    pub body: Vec<u8>,
}

impl Frame {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let prefix: [u8; 2] = data
            .get(..2)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| classic_error("RPC frame is shorter than its two-byte length prefix"))?;
        let header_size = usize::from(u16::from_be_bytes(prefix));
        let header_bytes = data.get(2..2 + header_size).ok_or_else(|| {
            classic_error(format!(
                "RPC header length {header_size} exceeds {} available bytes",
                data.len().saturating_sub(2)
            ))
        })?;
        let header = Header::decode(header_bytes)?;
        let body = data[2 + header_size..].to_vec();
        if header
            .size
            .is_some_and(|size| usize::try_from(size).is_ok_and(|size| size != body.len()))
        {
            return Err(classic_error(format!(
                "header body size {} does not match {} bytes",
                header.size.unwrap_or_default(),
                body.len()
            )));
        }
        Ok(Self { header, body })
    }

    pub fn encode(header: &Header, body: &[u8]) -> Result<Vec<u8>> {
        let size = u32::try_from(body.len())
            .map_err(|_| classic_error("RPC body exceeds the uint32 size field"))?;
        if header.size.is_some_and(|declared| declared != size) {
            return Err(classic_error(format!(
                "header body size {} does not match {} bytes",
                header.size.unwrap_or_default(),
                body.len()
            )));
        }
        let header = Header {
            size: Some(size),
            ..header.clone()
        };
        let encoded_header = header.encode();
        let header_length = u16::try_from(encoded_header.len())
            .map_err(|_| classic_error("encoded RPC header exceeds uint16 length"))?;

        let mut frame = Vec::with_capacity(2 + encoded_header.len() + body.len());
        frame.extend_from_slice(&header_length.to_be_bytes());
        frame.extend_from_slice(&encoded_header);
        frame.extend_from_slice(body);
        Ok(frame)
    }
}

fn narrow(value: u64, name: &str) -> Result<u32> {
    u32::try_from(value).map_err(|_| classic_error(format!("classic header {name} exceeds uint32")))
}

fn classic_error(message: impl Into<String>) -> Error {
    Error::ClassicWire(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_header() -> Header {
        Header {
            service_id: 0x17CD_FF07,
            method_id: 0x95F5_9163,
            token: 1,
            routing_id: Some(2_525_111_537),
            object_id: Some(0),
            is_response: Some(false),
            request_trace: Some(b"RT-0123456789ABCDEF".to_vec()),
            ..Header::default()
        }
    }

    #[test]
    fn round_trips_a_request_frame() {
        let encoded = Frame::encode(&request_header(), b"body").expect("encodable");
        let decoded = Frame::decode(&encoded).expect("decodable");
        assert_eq!(decoded.body, b"body");
        assert_eq!(
            decoded.header,
            Header {
                size: Some(4),
                ..request_header()
            }
        );
    }

    #[test]
    fn emits_explicit_zero_valued_object_and_response_fields() {
        // the retail client sends object_id=0 and is_response=0 explicitly,
        // and the edge is sensitive to the exact header bytes.
        let encoded = request_header().encode();
        let numbers: Vec<u32> = protobuf::fields(&encoded)
            .map(|field| field.expect("valid header").number)
            .collect();
        assert_eq!(numbers, [1, 2, 3, 4, 6, 9, 12]);
    }

    #[test]
    fn preserves_unknown_header_fields() {
        let mut raw = request_header().encode();
        raw.extend_from_slice(Message::new().varint(20, 99).as_slice());
        let header = Header::decode(&raw).expect("decodable");
        assert_eq!(protobuf::first_varint(&header.encode(), 20), Some(99));
    }

    #[test]
    fn rejects_malformed_frames() {
        let partial = Message::new().varint(1, 5).varint(2, 6).into_vec();
        assert!(Header::decode(&partial).is_err());

        let header = Header {
            size: Some(9),
            ..request_header()
        };
        assert!(Frame::encode(&header, b"body").is_err());

        let mut encoded = Frame::encode(&request_header(), b"body").expect("frame");
        encoded.push(b'!');
        assert!(Frame::decode(&encoded).is_err());

        assert!(Frame::decode(&[0xFF, 0xFF, 0x08]).is_err());
        assert!(Frame::decode(&[0x00]).is_err());
    }
}
