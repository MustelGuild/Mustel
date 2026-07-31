use crate::error::{MustelError, Result};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::LocalFree;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

/// Service for securely encrypting and decrypting database passwords.
pub struct CredentialStore;

impl CredentialStore {
    /// Encrypts plaintext password string into a base64 encoded string using Windows DPAPI (or OS fallback).
    pub fn encrypt_password(plaintext: &str) -> Result<String> {
        if plaintext.is_empty() {
            return Ok(String::new());
        }

        #[cfg(target_os = "windows")]
        {
            let bytes = plaintext.as_bytes();
            let mut data_in = CRYPT_INTEGER_BLOB {
                cbData: bytes.len() as u32,
                pbData: bytes.as_ptr() as *mut u8,
            };
            let mut data_out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            unsafe {
                let res = CryptProtectData(
                    &mut data_in,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0,
                    &mut data_out,
                );

                if res == 0 {
                    return Err(MustelError::Security("DPAPI CryptProtectData failed".into()));
                }

                let out_slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
                let encoded = base64_encode(out_slice);
                
                // Free DPAPI allocated memory via LocalFree
                LocalFree(data_out.pbData as _);

                Ok(encoded)
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Simple fallback for non-windows platforms (obfuscation)
            Ok(base64_encode(plaintext.as_bytes()))
        }
    }

    /// Decrypts a base64 encoded string previously encrypted with DPAPI.
    pub fn decrypt_password(encrypted_base64: &str) -> Result<String> {
        if encrypted_base64.is_empty() {
            return Ok(String::new());
        }

        let encrypted_bytes = base64_decode(encrypted_base64)?;

        #[cfg(target_os = "windows")]
        {
            let mut data_in = CRYPT_INTEGER_BLOB {
                cbData: encrypted_bytes.len() as u32,
                pbData: encrypted_bytes.as_ptr() as *mut u8,
            };
            let mut data_out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            unsafe {
                let res = CryptUnprotectData(
                    &mut data_in,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0,
                    &mut data_out,
                );

                if res == 0 {
                    return Err(MustelError::Security("DPAPI CryptUnprotectData failed".into()));
                }

                let out_slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
                let decrypted_str = String::from_utf8(out_slice.to_vec())
                    .map_err(|e| MustelError::Security(format!("Decrypted data invalid UTF-8: {}", e)))?;

                LocalFree(data_out.pbData as _);

                Ok(decrypted_str)
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            String::from_utf8(encrypted_bytes)
                .map_err(|e| MustelError::Security(format!("Decrypted data invalid UTF-8: {}", e)))
        }
    }
}

fn base64_encode(input: &[u8]) -> String {
    let mut s = String::new();
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triplet = (b0 << 16) | (b1 << 8) | b2;

        s.push(CHARS[((triplet >> 18) & 0x3F) as usize] as char);
        s.push(CHARS[((triplet >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            s.push(CHARS[((triplet >> 6) & 0x3F) as usize] as char);
        } else {
            s.push('=');
        }
        if chunk.len() > 2 {
            s.push(CHARS[(triplet & 0x3F) as usize] as char);
        } else {
            s.push('=');
        }
    }
    s
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    let clean = input.trim_end_matches('=');
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;

    for c in clean.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => continue,
        };

        buf = (buf << 6) | val;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_password() {
        let original = "SuperSecretP@ssw0rd!2026";
        let encrypted = CredentialStore::encrypt_password(original).unwrap();
        assert!(!encrypted.is_empty());
        assert_ne!(original, encrypted);

        let decrypted = CredentialStore::decrypt_password(&encrypted).unwrap();
        assert_eq!(original, decrypted);
    }
}
