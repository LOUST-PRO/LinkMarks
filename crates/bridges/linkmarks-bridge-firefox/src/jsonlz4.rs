//! Firefox jsonlz4 backup decoder.

use std::path::Path;

use crate::errors::BridgeError;
use crate::tree::FirefoxTree;

const MAGIC: &[u8; 8] = b"mozLz40\0";

/// Parse a Firefox `bookmarks-backups/*.jsonlz4` file.
pub fn parse_file(path: &Path) -> Result<FirefoxTree, BridgeError> {
    parse_bytes(&std::fs::read(path)?)
}

/// Decode a complete Firefox jsonlz4 payload.
pub fn parse_bytes(bytes: &[u8]) -> Result<FirefoxTree, BridgeError> {
    if bytes.len() < 12 {
        return Err(BridgeError::Jsonlz4Header(
            "payload is shorter than the 12-byte header".to_string(),
        ));
    }
    if &bytes[..8] != MAGIC {
        return Err(BridgeError::Jsonlz4Header(
            "missing mozLz40\\0 magic".to_string(),
        ));
    }

    let expected = u32::from_le_bytes(bytes[8..12].try_into().expect("four-byte slice")) as usize;
    let json = lz4_flex::block::decompress(&bytes[12..], expected)
        .map_err(|error| BridgeError::Jsonlz4Decompress(error.to_string()))?;
    if json.len() != expected {
        return Err(BridgeError::Jsonlz4Decompress(format!(
            "size mismatch: header says {expected}, decoded {}",
            json.len()
        )));
    }
    serde_json::from_slice(&json).map_err(BridgeError::JsonParse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_magic() {
        let error = parse_bytes(b"not-lz4-data").expect_err("must reject bad header");
        assert!(matches!(error, BridgeError::Jsonlz4Header(_)));
    }
}
