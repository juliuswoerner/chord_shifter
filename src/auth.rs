/// Password hashing helpers shared by both the desktop (SQLite) and web
/// (localStorage) backends.
///
/// Uses Argon2id – the recommended algorithm for new applications (winner of
/// the Password Hashing Competition, resistant to GPU and side-channel attacks).
///
/// We use the low-level `argon2` API directly so we control salt generation
/// via `getrandom`, which works on both native and wasm32 (with the `js` backend).
use argon2::{Algorithm, Argon2, Params, Version};

/// Length of the random salt in bytes (16 bytes = 128 bits).
const SALT_LEN: usize = 16;
/// Length of the output hash in bytes.
const HASH_LEN: usize = 32;

/// Hash a plaintext password. Returns a `"$salt_hex$hash_hex"` string that
/// should be stored verbatim in the database.
pub fn hash_password(password: &str) -> Result<String, String> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).map_err(|e| format!("RNG error: {e}"))?;

    let mut hash = [0u8; HASH_LEN];
    argon2id()
        .hash_password_into(password.as_bytes(), &salt, &mut hash)
        .map_err(|e| format!("Hash error: {e}"))?;

    Ok(format!("{}${}", hex::encode(salt), hex::encode(hash)))
}

/// Verify a plaintext password against a stored `"$salt_hex$hash_hex"` string.
/// Returns `true` if the password is correct.
pub fn verify_password(password: &str, stored: &str) -> bool {
    let Some((salt_hex, hash_hex)) = stored.split_once('$') else {
        return false;
    };
    let (Ok(salt), Ok(expected)) = (hex::decode(salt_hex), hex::decode(hash_hex)) else {
        return false;
    };
    let mut actual = vec![0u8; expected.len()];
    argon2id()
        .hash_password_into(password.as_bytes(), &salt, &mut actual)
        .is_ok()
        && constant_time_eq(&actual, &expected)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn argon2id() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19_456, 2, 1, Some(HASH_LEN)).expect("valid params"),
    )
}

/// Constant-time comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// ── Hex encoding (tiny inline impl, no extra dep) ─────────────────────────────

mod hex {
    const CHARS: &[u8] = b"0123456789abcdef";

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .flat_map(|b| {
                [
                    CHARS[(b >> 4) as usize] as char,
                    CHARS[(b & 0xf) as usize] as char,
                ]
            })
            .collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        if s.len() % 2 != 0 {
            return Err(());
        }
        s.as_bytes()
            .chunks(2)
            .map(|chunk| {
                let hi = from_hex_char(chunk[0])?;
                let lo = from_hex_char(chunk[1])?;
                Ok((hi << 4) | lo)
            })
            .collect()
    }

    fn from_hex_char(c: u8) -> Result<u8, ()> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn two_hashes_of_same_password_differ() {
        let h1 = hash_password("abc").unwrap();
        let h2 = hash_password("abc").unwrap();
        // Salt is random so the stored strings must differ even for the same input
        assert_ne!(h1, h2);
        // …but both must still verify correctly
        assert!(verify_password("abc", &h1));
        assert!(verify_password("abc", &h2));
    }
}
