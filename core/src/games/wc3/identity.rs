use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha1::{Digest as _, Sha1};
use zeroize::Zeroizing;

use crate::{Error, Result, platform::bgs::SecretBytes};

const UUID_TEXT_BYTES: usize = 36;

#[cfg(target_os = "macos")]
struct InterfaceList(*mut libc::ifaddrs);

#[cfg(target_os = "macos")]
impl Drop for InterfaceList {
    fn drop(&mut self) {
        // SAFETY: this is the successful getifaddrs result and is freed once.
        unsafe { libc::freeifaddrs(self.0) };
    }
}

#[derive(Clone, Debug)]
pub struct ClientIdentity(SecretBytes);

impl ClientIdentity {
    pub fn derive(host_uuid: &str, physical_address: &[u8; 6]) -> Result<Self> {
        if host_uuid.len() != UUID_TEXT_BYTES
            || !host_uuid
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        {
            return Err(identity_error("host UUID is not canonical"));
        }
        let mut digest = Sha1::new();
        digest.update(host_uuid.to_ascii_uppercase().as_bytes());
        digest.update(physical_address);
        let encoded = Zeroizing::new(BASE64.encode(digest.finalize()).into_bytes());
        if encoded.len() != 28 {
            return Err(identity_error("derived identity has an invalid length"));
        }
        Ok(Self(SecretBytes::new(encoded.to_vec())?))
    }

    #[cfg(target_os = "macos")]
    pub fn for_current_host() -> Result<Self> {
        Self::derive(&host_uuid()?, &physical_address()?)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn for_current_host() -> Result<Self> {
        Err(identity_error(
            "WC3 host identity derivation is currently available on macOS",
        ))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.expose()
    }
}

#[cfg(target_os = "macos")]
fn host_uuid() -> Result<Zeroizing<String>> {
    let mut uuid = [0_u8; 16];
    let timeout = libc::timespec {
        tv_sec: 5,
        tv_nsec: 0,
    };
    // SAFETY: gethostuuid receives its required 16-byte buffer and valid timeout.
    if unsafe { libc::gethostuuid(uuid.as_mut_ptr(), &raw const timeout) } != 0 {
        return Err(identity_error(format!(
            "gethostuuid failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(Zeroizing::new(format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        uuid[0],
        uuid[1],
        uuid[2],
        uuid[3],
        uuid[4],
        uuid[5],
        uuid[6],
        uuid[7],
        uuid[8],
        uuid[9],
        uuid[10],
        uuid[11],
        uuid[12],
        uuid[13],
        uuid[14],
        uuid[15]
    )))
}

#[cfg(target_os = "macos")]
fn physical_address() -> Result<[u8; 6]> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs initializes head and the guard releases it once.
    if unsafe { libc::getifaddrs(&raw mut head) } != 0 {
        return Err(identity_error(format!(
            "getifaddrs failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let list = InterfaceList(head);
    let mut cursor = list.0;
    while !cursor.is_null() {
        // SAFETY: cursor comes from the live getifaddrs list.
        let interface = unsafe { &*cursor };
        let address = interface.ifa_addr;
        if interface.ifa_flags & (libc::IFF_LOOPBACK as u32) == 0 && !address.is_null() {
            // SAFETY: every sockaddr begins with its family.
            if unsafe { i32::from((*address).sa_family) } == libc::AF_LINK {
                // SAFETY: AF_LINK denotes sockaddr_dl on macOS.
                let link = unsafe { &*address.cast::<libc::sockaddr_dl>() };
                if link.sdl_type == 6 && link.sdl_alen == 6 {
                    let start = usize::from(link.sdl_nlen);
                    if let Some(raw) = link.sdl_data.get(start..start + 6) {
                        let mut result = [0_u8; 6];
                        for (output, input) in result.iter_mut().zip(raw) {
                            *output = input.cast_unsigned();
                        }
                        if result != [0; 6] {
                            return Ok(result);
                        }
                    }
                }
            }
        }
        cursor = interface.ifa_next;
    }
    Err(identity_error("no physical network interface was found"))
}

fn identity_error(message: impl Into<String>) -> Error {
    Error::Authentication(format!("WC3 client identity: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_the_retail_identity_shape() {
        let identity = ClientIdentity::derive(
            "00112233-4455-6677-8899-AABBCCDDEEFF",
            &[0x10, 0x20, 0x30, 0x40, 0x50, 0x60],
        )
        .unwrap();
        assert_eq!(identity.as_bytes().len(), 28);
        assert_eq!(BASE64.decode(identity.as_bytes()).unwrap().len(), 20);
    }
}
