//! Room Code Generation and Parsing
//!
//! Generates human-friendly room codes that encode peer connection info.

use libp2p::PeerId;
use std::fmt;

/// Characters used in room codes (unambiguous, uppercase)
/// Excludes: 0/O, 1/I/L, 5/S, 2/Z to avoid confusion
const ALPHABET: &[u8] = b"346789ABCDEFGHJKMNPQRTUVWXY";

/// A room code that can be shared to join a room
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomCode(String);

impl RoomCode {
    /// Generate a room code from a peer ID
    ///
    /// Takes the first N bytes of the peer ID and encodes them
    /// in a human-friendly format.
    pub fn from_peer_id(peer_id: &PeerId) -> Self {
        let bytes = peer_id.to_bytes();
        // Take bytes 2-8 (skip the multicodec prefix) and encode them
        let code = encode_bytes(&bytes[2..8]);
        RoomCode(code)
    }

    /// Generate a random room code (for testing)
    pub fn random() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut code = String::with_capacity(6);
        let mut n = seed;
        for _ in 0..6 {
            let idx = (n % ALPHABET.len() as u64) as usize;
            code.push(ALPHABET[idx] as char);
            n /= ALPHABET.len() as u64;
        }
        RoomCode(code)
    }

    /// Get the room code as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse a room code from user input
    ///
    /// Normalizes to uppercase and validates format.
    pub fn parse(input: &str) -> Option<Self> {
        let normalized: String = input
            .chars()
            .filter(|c| c.is_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect();

        if normalized.len() != 6 {
            return None;
        }

        // Validate all characters are in our alphabet
        if normalized.bytes().all(|b| ALPHABET.contains(&b)) {
            Some(RoomCode(normalized))
        } else {
            None
        }
    }
}

impl fmt::Display for RoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format as XXX-XXX for readability
        if self.0.len() == 6 {
            write!(f, "{}-{}", &self.0[..3], &self.0[3..])
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Encode bytes to room code characters
fn encode_bytes(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(6);
    let mut accumulator: u64 = 0;

    for (i, &byte) in bytes.iter().take(6).enumerate() {
        accumulator |= (byte as u64) << (i * 8);
    }

    for _ in 0..6 {
        let idx = (accumulator % ALPHABET.len() as u64) as usize;
        result.push(ALPHABET[idx] as char);
        accumulator /= ALPHABET.len() as u64;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_code_parse() {
        let code = RoomCode::parse("ABC-DEF").unwrap();
        assert_eq!(code.as_str(), "ABCDEF");

        let code = RoomCode::parse("abc def").unwrap();
        assert_eq!(code.as_str(), "ABCDEF");

        assert!(RoomCode::parse("ABC").is_none()); // Too short
        assert!(RoomCode::parse("ABCDEFG").is_none()); // Too long
    }

    #[test]
    fn test_room_code_display() {
        let code = RoomCode("ABCDEF".to_string());
        assert_eq!(format!("{}", code), "ABC-DEF");
    }

    #[test]
    fn test_random_code() {
        let code1 = RoomCode::random();
        let code2 = RoomCode::random();
        // Very unlikely to be equal
        assert_ne!(code1, code2);
        assert_eq!(code1.as_str().len(), 6);
    }
}
