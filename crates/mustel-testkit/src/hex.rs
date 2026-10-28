//! Codificação hexadecimal minúscula usada nos hashes do baseline.

/// Converte bytes para hexadecimal minúsculo (dois dígitos por byte).
pub(crate) fn lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}
