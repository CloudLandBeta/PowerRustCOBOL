// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Content hashes: the short hash that names a file, and the Subresource Integrity value
//! (`sha384-<base64>`) a browser checks (WASM-013 R10).

use sha2::{Digest, Sha256, Sha384};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (what an `integrity` attribute holds).
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `sha384-<base64>`: the integrity metadata of a file.
pub fn sri(bytes: &[u8]) -> String {
    format!("sha384-{}", base64(&Sha384::digest(bytes)))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Eight hex digits of the SHA-256: the part of a file name that changes when the content does.
pub fn short(bytes: &[u8]) -> String {
    sha256_hex(bytes)[..8].to_owned()
}

/// `name.ext` → `name.<hash>.ext` (a leading directory is kept; a name with no extension gets `.<hash>`).
pub fn hashed_name(path: &str, bytes: &[u8]) -> String {
    let h = short(bytes);
    let (dir, file) = match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    };
    match file.rfind('.') {
        Some(i) if i > 0 => format!("{dir}{}.{h}{}", &file[..i], &file[i..]),
        _ => format!("{dir}{file}.{h}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (i, o) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(i.as_bytes()), o);
        }
    }

    #[test]
    fn sri_is_the_sha384_of_the_bytes() {
        // sha384("abc") is a known vector.
        assert_eq!(
            sri(b"abc"),
            "sha384-ywB1P0WjXou1oD1pmsZQBycsMqsO3tFjGotgWkP/W+2AhgcroefMI1i67KE0yCWn"
        );
    }

    #[test]
    fn hashed_names_keep_directory_and_extension() {
        let h = short(b"x");
        assert_eq!(hashed_name("program.wasm", b"x"), format!("program.{h}.wasm"));
        assert_eq!(hashed_name("player/plain/player_bg.wasm", b"x"), format!("player/plain/player_bg.{h}.wasm"));
        assert_eq!(hashed_name("LICENSE", b"x"), format!("LICENSE.{h}"));
        assert_ne!(short(b"x"), short(b"y"));
    }
}
