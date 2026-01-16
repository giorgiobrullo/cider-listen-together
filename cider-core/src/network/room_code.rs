//! Room Code Generation and Parsing
//!
//! Generates human-friendly room codes that encode peer connection info.

use libp2p::PeerId;
use std::fmt;

/// Characters used in room codes (unambiguous, uppercase)
/// Excludes: 0/O, 1/I/L, 5/S, 2/Z to avoid confusion
const ALPHABET: &[u8] = b"346789ABCDEFGHJKMNPQRTUVWXY";

/// Room code length (8 chars = ~282 trillion combinations with 27-char alphabet)
const CODE_LENGTH: usize = 8;

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
        // Take bytes 2-10 (skip the multicodec prefix) and encode them
        let code = encode_bytes(&bytes[2..10]);
        RoomCode(code)
    }

    /// Generate a random room code using cryptographically secure RNG
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let mut code = String::with_capacity(CODE_LENGTH);
        for _ in 0..CODE_LENGTH {
            let idx = rng.gen_range(0..ALPHABET.len());
            code.push(ALPHABET[idx] as char);
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

        if normalized.len() != CODE_LENGTH {
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
        // Format as XXXX-XXXX for readability
        if self.0.len() == CODE_LENGTH {
            write!(f, "{}-{}", &self.0[..4], &self.0[4..])
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Encode bytes to room code characters
fn encode_bytes(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(CODE_LENGTH);
    let mut accumulator: u128 = 0;

    for (i, &byte) in bytes.iter().take(CODE_LENGTH).enumerate() {
        accumulator |= (byte as u128) << (i * 8);
    }

    for _ in 0..CODE_LENGTH {
        let idx = (accumulator % ALPHABET.len() as u128) as usize;
        result.push(ALPHABET[idx] as char);
        accumulator /= ALPHABET.len() as u128;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_code_parse() {
        let code = RoomCode::parse("ABCD-EFGH").unwrap();
        assert_eq!(code.as_str(), "ABCDEFGH");

        let code = RoomCode::parse("abcd efgh").unwrap();
        assert_eq!(code.as_str(), "ABCDEFGH");

        assert!(RoomCode::parse("ABC").is_none()); // Too short
        assert!(RoomCode::parse("ABCDEFGHI").is_none()); // Too long (9 chars)
    }

    #[test]
    fn test_room_code_display() {
        let code = RoomCode("ABCDEFGH".to_string());
        assert_eq!(format!("{}", code), "ABCD-EFGH");
    }

    #[test]
    fn test_random_code() {
        let code1 = RoomCode::random();
        let code2 = RoomCode::random();
        // Very unlikely to be equal
        assert_ne!(code1, code2);
        assert_eq!(code1.as_str().len(), 8);
    }
}
