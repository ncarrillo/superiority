use subtle::ConstantTimeEq;

use crate::games::sc2::native::errors::native_error;
use crate::{
    Error, Result,
    bsn::value::{BsnStruct, BsnValue},
    native::crypto::{SessionProof, build_session_proof, verify_thumbprint_proof},
};

const MODULE_USAGE: [u8; 8] = *b"auth\0\0\0\0";
const SESSION_MODULE_HASH: [u8; 32] = [
    0x89, 0x50, 0x05, 0x34, 0x0a, 0x63, 0x0a, 0x64, 0x65, 0xa6, 0x5f, 0xec, 0x96, 0x32, 0x3c, 0x31,
    0x0b, 0xca, 0x8a, 0x9f, 0x66, 0xec, 0xee, 0xb1, 0x88, 0x7a, 0x9d, 0x6c, 0x0e, 0x67, 0x61, 0x2e,
];
const THUMBPRINT_MODULE_HASH: [u8; 32] = [
    0xd7, 0xe6, 0x62, 0x40, 0x80, 0xc1, 0xab, 0xa6, 0x6d, 0xee, 0x63, 0xa6, 0xf3, 0x92, 0x8d, 0x8a,
    0x54, 0x69, 0x25, 0x7f, 0x58, 0x20, 0xb5, 0x72, 0x1f, 0xb8, 0xc3, 0x2b, 0x6b, 0x5b, 0xef, 0x5d,
];

pub const SESSION_PROOF_MODULE_ID: [u8; 40] = module_id(SESSION_MODULE_HASH);
pub const THUMBPRINT_MODULE_ID: [u8; 40] = module_id(THUMBPRINT_MODULE_HASH);

pub struct ProofAnswer {
    pub outputs: Vec<Vec<u8>>,
    pub session: SessionProof,
}

impl std::fmt::Debug for ProofAnswer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProofAnswer")
            .field("output_count", &self.outputs.len())
            .finish_non_exhaustive()
    }
}

pub fn answer_proof_request(
    request: &BsnStruct,
    session_seed: &[u8],
    thumbprint_context: &[u8],
) -> Result<ProofAnswer> {
    let modules = expect_array(require(request, "m_request")?, "proof request modules")?;
    let mut saw_thumbprint = false;
    let mut saw_session = false;
    let mut outputs = Vec::with_capacity(1);
    let mut session = None;

    for module in modules {
        let module = expect_struct(module, "proof module")?;
        let identifier = expect_bytes(require(module, "m_id")?, "proof module id")?;
        let data = expect_bytes(require(module, "m_data")?, "proof module data")?;
        if identifier == THUMBPRINT_MODULE_ID {
            if saw_thumbprint {
                return Err(native_error("proof request repeats the thumbprint module"));
            }
            saw_thumbprint = true;
            if !verify_thumbprint_proof(thumbprint_context, data) {
                return Err(native_error("native server thumbprint proof failed"));
            }
        } else if identifier == SESSION_PROOF_MODULE_ID {
            if saw_session {
                return Err(native_error("proof request repeats the session module"));
            }
            saw_session = true;
            if data.len() != 17 || data[0] != 0 {
                return Err(native_error("unsupported native session-proof phase"));
            }
            let proof = build_session_proof(session_seed, &data[1..])?;
            outputs.push(proof.output().to_vec());
            session = Some(proof);
        } else {
            return Err(native_error("server requested an unknown auth module"));
        }
    }
    if !saw_thumbprint || !saw_session {
        return Err(native_error(
            "proof request is missing a required auth module",
        ));
    }
    Ok(ProofAnswer {
        outputs,
        session: session.expect("session module was observed"),
    })
}

pub fn validate_resume_server_proof(resume: &BsnStruct, proof: &SessionProof) -> Result<()> {
    let (outer_index, outer_value) = expect_choice(require(resume, "m_result")?, "resume result")?;
    if outer_index != 0 {
        if outer_index == 1
            && let Ok(failure) = expect_struct(outer_value, "resume failure")
            && let Some(result) = failure.get("m_result")
            && let Ok((1, detail)) = expect_choice(result, "resume failure detail")
            && let Ok(detail) = expect_struct(detail, "resume failure detail")
        {
            let error_code = require_integer(detail, "m_error")?;
            let wait = require_integer(detail, "m_wait")?;
            return Err(Error::NativeResumeRejected { error_code, wait });
        }
        return Err(native_error(format!(
            "native Battle.net resume was rejected (choice={outer_index})"
        )));
    }

    let success = expect_struct(outer_value, "resume success")?;
    let common = expect_struct(
        require(success, "ResponseSuccessCommon")?,
        "resume success common state",
    )?;
    let modules = expect_array(
        require(common, "m_finalRequest")?,
        "resume final auth modules",
    )?;
    if modules.len() != 1 {
        return Err(native_error(
            "resume response has an unexpected final auth-module set",
        ));
    }
    let module = expect_struct(&modules[0], "resume final auth module")?;
    let identifier = expect_bytes(require(module, "m_id")?, "resume module id")?;
    let data = expect_bytes(require(module, "m_data")?, "resume module data")?;
    if identifier != SESSION_PROOF_MODULE_ID {
        return Err(native_error(
            "resume response used an unknown final auth module",
        ));
    }
    if data.len() != 33 || data[0] != 2 {
        return Err(native_error(
            "resume response has an invalid session-proof phase",
        ));
    }
    if !bool::from(data[1..].ct_eq(proof.expected_server_proof())) {
        return Err(native_error("native server session proof failed"));
    }
    Ok(())
}

pub fn native_server_error(record: &BsnStruct) -> Result<Error> {
    Ok(Error::NativeServerRejected {
        error_code: require_integer(record, "m_error")?,
    })
}

fn require<'a>(value: &'a BsnStruct, name: &str) -> Result<&'a BsnValue> {
    value
        .get(name)
        .ok_or_else(|| native_error(format!("native struct is missing {name}")))
}

fn require_integer(value: &BsnStruct, name: &str) -> Result<i128> {
    expect_integer(require(value, name)?, name)
}

fn transparent(value: &BsnValue) -> &BsnValue {
    match value {
        BsnValue::Optional(Some(value)) => transparent(value),
        _ => value,
    }
}

fn expect_struct<'a>(value: &'a BsnValue, label: &str) -> Result<&'a BsnStruct> {
    transparent(value)
        .as_struct()
        .ok_or_else(|| native_error(format!("{label} is not a struct")))
}

fn expect_array<'a>(value: &'a BsnValue, label: &str) -> Result<&'a [BsnValue]> {
    match transparent(value) {
        BsnValue::Array(value) => Ok(value),
        _ => Err(native_error(format!("{label} is not an array"))),
    }
}

fn expect_bytes<'a>(value: &'a BsnValue, label: &str) -> Result<&'a [u8]> {
    match transparent(value) {
        BsnValue::Bytes(value) => Ok(value),
        _ => Err(native_error(format!("{label} is not bytes"))),
    }
}

fn expect_integer(value: &BsnValue, label: &str) -> Result<i128> {
    match transparent(value) {
        BsnValue::Integer(value) => Ok(*value),
        _ => Err(native_error(format!("{label} is not an integer"))),
    }
}

fn expect_choice<'a>(value: &'a BsnValue, label: &str) -> Result<(i128, &'a BsnValue)> {
    match transparent(value) {
        BsnValue::Choice { index, value } => Ok((*index, value)),
        _ => Err(native_error(format!("{label} is not a choice"))),
    }
}

const fn module_id(hash: [u8; 32]) -> [u8; 40] {
    let mut output = [0_u8; 40];
    let mut index = 0;
    while index < MODULE_USAGE.len() {
        output[index] = MODULE_USAGE[index];
        index += 1;
    }
    index = 0;
    while index < hash.len() {
        output[index + MODULE_USAGE.len()] = hash[index];
        index += 1;
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::bsn::value::BsnField;

    use super::*;

    fn field(index: i128, name: &str, value: BsnValue) -> BsnField {
        BsnField::named(index, name, value)
    }

    fn successful_resume(data: Vec<u8>) -> BsnStruct {
        let module = BsnValue::Struct(BsnStruct::new(
            0,
            vec![
                field(
                    -2,
                    "m_id",
                    BsnValue::Bytes(SESSION_PROOF_MODULE_ID.to_vec()),
                ),
                field(-1, "m_data", BsnValue::Bytes(data)),
            ],
        ));
        let common = BsnValue::Struct(BsnStruct::new(
            0,
            vec![field(-3, "m_finalRequest", BsnValue::Array(vec![module]))],
        ));
        let success = BsnValue::Struct(BsnStruct::new(
            0,
            vec![field(-1, "ResponseSuccessCommon", common)],
        ));
        BsnStruct::new(0, vec![field(-1, "m_result", BsnValue::choice(0, success))])
    }

    #[test]
    fn resume_phase_two_accepts_only_the_expected_server_proof() {
        let proof = build_session_proof(&[0; 64], &[0; 16]).unwrap();
        let mut data = vec![2];
        data.extend_from_slice(proof.expected_server_proof());
        assert!(validate_resume_server_proof(&successful_resume(data.clone()), &proof).is_ok());
        *data.last_mut().unwrap() ^= 1;
        assert!(validate_resume_server_proof(&successful_resume(data), &proof).is_err());
    }
}
