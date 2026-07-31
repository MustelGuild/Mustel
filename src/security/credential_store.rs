use crate::error::{MustelError, Result};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, password_hash::SaltString};
use rand::rngs::OsRng;
use rand::RngCore;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::LocalFree;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

/// Credential encryption scheme version for migration support
const ENCRYPTION_VERSION: u8 = 2;

/// Service for securely encrypting and decrypting database passwords.
pub struct CredentialStore;

/// Returns the path to the key file used for non-Windows encryption
fn get_key_file_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "Mustel")
        .map(|dirs| dirs.config_dir().join(".key"))
}

/// Derives a 256-bit key from a password using Argon2id
fn derive_key(password: &[u8], salt: &[u8]) -> [u8; 32] {
    let argon2 = Argon2::default();
    let mut key = [0u8; 32];
    argon2.hash_password_into(password, salt, &mut key).expect("Argon2 should not fail with valid parameters");
    key
}

/// Returns the machine-specific encryption key for non-Windows platforms.
/// Creates a new key file if it doesn't exist (first-time setup).
fn get_machine_key() -> Result<[u8; 32]> {
    let key_path = get_key_file_path()
        .ok_or_else(|| MustelError::Security("Could not determine config directory".into()))?;

    if key_path.exists() {
        let key_data = fs::read(&key_path)?;
        if key_data.len() != 32 {
            return Err(MustelError::Security("Invalid key file length".into()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_data);
        Ok(key)
    } else {
        // Generate a new random key
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        // Create parent directory if needed
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Set restrictive permissions (Unix: read/write for user only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::Permissions::mode(0o600);
            fs::set_permissions(&key_path, perms)?;
        }

        fs::write(&key_path, &key)?;
        Ok(key)
    }
}

/// Encrypts data using AES-256-GCM
fn aes_encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| MustelError::Security("Failed to create cipher".into()))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|_| MustelError::Security("Encryption failed".into()))?;

    // Format: version (1 byte) + nonce (12 bytes) + ciphertext
    let mut result = Vec::with_capacity(1 + 12 + ciphertext.len());
    result.push(ENCRYPTION_VERSION);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypts data using AES-256-GCM
fn aes_decrypt(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if encrypted.len() < 13 {
        return Err(MustelError::Security("Encrypted data too short".into()));
    }

    let version = encrypted[0];
    let nonce = Nonce::from_slice(&encrypted[1..13]);
    let ciphertext = &encrypted[13..];

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| MustelError::Security("Failed to create cipher".into()))?;

    match version {
        2 => {
            cipher.decrypt(nonce, ciphertext)
                .map_err(|_| MustelError::Security("Decryption failed (wrong key or corrupted data)".into()))
        }
        1 => {
            // Legacy base64 "encryption" - cannot decrypt
            Err(MustelError::Security("Legacy unencrypted credentials detected. Please re-set the password using: mustel settings db-servers set-password".into()))
        }
        _ => Err(MustelError::Security(format!("Unknown encryption version: {}", version))),
    }
}

impl CredentialStore {
    /// Encrypts plaintext password string into an encrypted, base64 encoded string.
    /// Uses Windows DPAPI on Windows, AES-256-GCM with machine-derived key on other platforms.
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

                LocalFree(data_out.pbData as _);

                Ok(encoded)
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Use AES-256-GCM with machine-derived key
            let key = get_machine_key()?;
            let encrypted = aes_encrypt(plaintext.as_bytes(), &key)?;
            Ok(base64_encode(&encrypted))
        }
    }

    /// Decrypts an encrypted, base64 encoded string previously encrypted with encrypt_password.
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
            let key = get_machine_key()?;
            let decrypted = aes_decrypt(&encrypted_bytes, &key)?;
            String::from_utf8(decrypted)
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
