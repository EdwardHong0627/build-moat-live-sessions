//! NEWTYPE + TRAIT-OBJECT PATTERNS
//!
//! `Token` is a newtype around `String`. The point isn't behaviour — it's *meaning*:
//! a function that takes a `Token` can only be called with something that went through
//! token generation, so "is this string a real token?" becomes a compile-time fact
//! instead of a runtime convention.
//!
//! `TokenGenerator` is a trait so the generation *strategy* is decoupled from the
//! callers. The app uses `RandomTokenGenerator`; a test could inject a deterministic
//! generator. The collision-retry loop lives in `repo`, which just keeps asking the
//! generator for a fresh token until the DB accepts one.

use rand::Rng;

/// Base62 alphabet (A–Z a–z 0–9): URL-safe and case-sensitive for maximum density.
const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// 7 chars of base62 => 62^7 ≈ 3.5e12 possible tokens.
pub const TOKEN_LENGTH: usize = 7;

#[derive(Debug, Clone)]
pub struct Token(String);

impl Token {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Strategy interface. `Send + Sync` so it can live inside the shared `AppState`.
pub trait TokenGenerator: Send + Sync {
    fn generate(&self) -> Token;
}

/// Default strategy: uniformly random base62. Random (not hash-of-URL) means tokens
/// are unguessable and don't leak whether a URL was already shortened.
pub struct RandomTokenGenerator;

impl TokenGenerator for RandomTokenGenerator {
    fn generate(&self) -> Token {
        let mut rng = rand::thread_rng();
        let s: String = (0..TOKEN_LENGTH)
            .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
            .collect();
        Token(s)
    }
}
