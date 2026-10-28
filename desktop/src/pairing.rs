//! Short pairing codes that expand to the 16-byte wire token.
//!
//! Thirty-two hexadecimal characters are unusable on a touch keyboard, so the
//! desktop prints a six-character code instead and both sides expand it with
//! [`token_from_code`]. The code *is* the secret: the expansion is
//! deterministic and adds no entropy. Six Crockford base32 characters carry 30
//! bits, which stops accidental cross-session injection when wrong guesses are
//! rate limited (see [`crate::session`]) and is nowhere near enough to resist a
//! hostile network. Use `absolutetouch token --reset --hex` with 32 hexadecimal
//! characters when that matters.

use std::fmt;

/// Number of characters in a pairing code.
pub const CODE_LENGTH: usize = 6;

/// Crockford base32 alphabet: no `I`, `L`, `O`, or `U`.
///
/// Input folds the confusable letters to the digits they resemble (`I` and `L`
/// to `1`, `O` to `0`); `U` is excluded so generated codes cannot spell
/// accidental words.
pub const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// FNV-1a offset basis, as published by the FNV specification.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a prime, as published by the FNV specification.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Domain separator, so this expansion cannot collide with another use.
const DOMAIN: &[u8] = b"absolutetouch.pairing.v1:";

/// Why a pairing code was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingError {
    /// The code did not have exactly [`CODE_LENGTH`] characters.
    Length(usize),
    /// The code contained a character outside [`ALPHABET`].
    Character(char),
}

impl fmt::Display for PairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(length) => write!(
                formatter,
                "a pairing code has {CODE_LENGTH} characters, got {length}"
            ),
            Self::Character(character) => write!(
                formatter,
                "{character:?} is not a pairing code character; use {ALPHABET:?}"
            ),
        }
    }
}

impl std::error::Error for PairingError {}

/// Normalize a typed code to its canonical upper-case form.
///
/// Whitespace, `-`, and `:` separators are ignored, so a code read from a
/// terminal or written in groups is accepted. Lower case is folded to upper
/// case, and the letters Crockford excludes are folded to the digits they
/// resemble: `I` and `L` to `1`, `O` to `0`.
///
/// # Errors
///
/// Returns [`PairingError::Length`] or [`PairingError::Character`] when the
/// result is not a valid code.
pub fn normalize(text: &str) -> Result<String, PairingError> {
    let mut code = String::with_capacity(CODE_LENGTH);
    for character in text.chars() {
        if character.is_whitespace() || character == '-' || character == ':' {
            continue;
        }
        if !character.is_ascii() {
            return Err(PairingError::Character(character));
        }
        let upper = character.to_ascii_uppercase();
        let folded = match upper {
            'I' | 'L' => '1',
            'O' => '0',
            other => other,
        };
        if !ALPHABET.contains(&(folded as u8)) {
            return Err(PairingError::Character(character));
        }
        code.push(folded);
    }

    if code.len() == CODE_LENGTH {
        Ok(code)
    } else {
        Err(PairingError::Length(code.chars().count()))
    }
}

/// Expand a pairing code into the 16-byte token sent in `HELLO`.
///
/// The mapping is a plain deterministic expansion, not a password hash. It
/// needs no stretching: the code carries 30 bits, the derived token travels in
/// clear text, and anyone who can read the token already has the secret.
///
/// # Errors
///
/// Returns [`PairingError`] when the text is not a valid code.
pub fn token_from_code(text: &str) -> Result<[u8; 16], PairingError> {
    let code = normalize(text)?;
    Ok(expand(code.as_bytes()))
}

/// Generate a random code from the system random number generator.
///
/// Each character consumes the low five bits of one random byte. The alphabet
/// has 32 symbols, so every value is equally likely and no rejection sampling
/// is needed.
#[must_use]
pub fn generate_code() -> String {
    let mut bytes = [0_u8; CODE_LENGTH];
    if getrandom::fill(&mut bytes).is_err() {
        // Entropy is unavailable; fall back to the clock so the CLI can still
        // print something. The wire protocol does not rely on this path.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos() as u64);
        bytes = now.to_le_bytes()[..CODE_LENGTH]
            .try_into()
            .unwrap_or([0_u8; CODE_LENGTH]);
    }

    bytes
        .iter()
        .map(|byte| ALPHABET[usize::from(byte & 0x1F)] as char)
        .collect()
}

/// Expand a normalized code with two FNV-1a passes.
fn expand(code: &[u8]) -> [u8; 16] {
    let mut first = Vec::with_capacity(DOMAIN.len() + code.len());
    first.extend_from_slice(DOMAIN);
    first.extend_from_slice(code);
    let high = fnv1a64(&first);

    let mut second = Vec::with_capacity(first.len() + 8);
    second.extend_from_slice(&first);
    second.extend_from_slice(&high.to_le_bytes());
    let low = fnv1a64(&second);

    let mut token = [0_u8; 16];
    token[..8].copy_from_slice(&high.to_le_bytes());
    token[8..].copy_from_slice(&low.to_le_bytes());
    token
}

/// FNV-1a over the 64-bit variant.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{ALPHABET, CODE_LENGTH, PairingError, generate_code, normalize, token_from_code};

    const VECTORS: &str = include_str!("../../protocol/vectors.json");

    fn pairing_vectors() -> Vec<(String, String, String)> {
        let document: Value = serde_json::from_str(VECTORS).expect("parse vectors");
        let pairing = &document["pairing"];
        assert_eq!(pairing["code_length"], CODE_LENGTH);
        assert_eq!(
            pairing["alphabet"].as_str().expect("alphabet"),
            std::str::from_utf8(ALPHABET).expect("alphabet is ASCII")
        );
        pairing["vectors"]
            .as_array()
            .expect("vectors")
            .iter()
            .map(|vector| {
                (
                    vector["input"].as_str().expect("input").to_owned(),
                    vector["code"].as_str().expect("code").to_owned(),
                    vector["token"].as_str().expect("token").to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn codes_expand_to_the_shared_token_vectors() {
        let vectors = pairing_vectors();
        assert!(vectors.len() >= 3);

        for (input, code, token) in vectors {
            assert_eq!(normalize(&input).expect("normalize"), code);
            let derived = token_from_code(&input).expect("derive");
            assert_eq!(
                crate::config::encode_hex16(&derived),
                token,
                "code {input} derived the wrong token"
            );
        }
    }

    #[test]
    fn normalization_folds_the_confusable_letters() {
        assert_eq!(normalize("0O1IL2").expect("normalize"), "001112");
        assert_eq!(normalize("k7m-2qx").expect("normalize"), "K7M2QX");
        assert_eq!(normalize(" k7m : 2qx ").expect("normalize"), "K7M2QX");
    }

    #[test]
    fn wrong_length_and_unknown_characters_are_rejected() {
        assert_eq!(normalize("ABC"), Err(PairingError::Length(3)));
        assert_eq!(normalize("ABCDEFG"), Err(PairingError::Length(7)));
        assert_eq!(normalize("ABC$EF"), Err(PairingError::Character('$')));
        assert_eq!(normalize("ABCDEÖ"), Err(PairingError::Character('Ö')));
        assert!(token_from_code("K7M2QX0").is_err());
    }

    #[test]
    fn a_generated_code_round_trips() {
        for _ in 0..64 {
            let code = generate_code();
            assert_eq!(code.len(), CODE_LENGTH);
            assert_eq!(normalize(&code).expect("normalize"), code);
            assert!(code.bytes().all(|byte| ALPHABET.contains(&byte)));
            assert!(token_from_code(&code).is_ok());
        }
    }

    #[test]
    fn different_codes_give_different_tokens() {
        let first = token_from_code("K7M2QX").expect("first");
        let second = token_from_code("K7M2QY").expect("second");
        assert_ne!(first, second);
        // The expansion fills every byte, so the token does not look like a
        // padded code.
        assert!(first[8..].iter().any(|byte| *byte != 0));
    }
}
