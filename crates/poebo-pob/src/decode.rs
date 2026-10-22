//! Turn a PoB import code into the underlying XML bytes.
//!
//! A code is `url-safe-base64( zlib( xml ) )`. We normalize the url-safe alphabet to the standard one
//! (a no-op for a code that already uses `+`/`/`), strip any embedded whitespace/newlines, decode
//! with indifference to padding, then zlib-inflate. Downloading a code from a build link is the
//! app's job; this crate stays offline.

use std::io::Read;

use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine;
use flate2::read::ZlibDecoder;

use crate::error::PobError;

/// Decode a PoB import code (as pasted or read from a file) into raw XML bytes.
pub fn decode_import_code(code: &str) -> Result<Vec<u8>, PobError> {
    // Normalize url-safe -> standard and drop whitespace so pasted, line-wrapped codes still decode.
    let normalized: String = code
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();

    // Standard alphabet, indifferent to whether the code carries `=` padding.
    let engine = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    let compressed = engine.decode(normalized)?;

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut xml = Vec::new();
    decoder.read_to_end(&mut xml).map_err(PobError::Inflate)?;
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    /// Encode XML the way PoB does, for round-trip testing (url-safe base64 of a zlib stream).
    fn encode(xml: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(xml.as_bytes()).expect("compress");
        let compressed = encoder.finish().expect("finish");
        base64::engine::general_purpose::URL_SAFE.encode(compressed)
    }

    #[test]
    fn round_trips_url_safe_code() {
        let xml = "<PathOfBuilding><Build level=\"12\"/></PathOfBuilding>";
        let decoded = decode_import_code(&encode(xml)).expect("decode");
        assert_eq!(String::from_utf8(decoded).unwrap(), xml);
    }

    #[test]
    fn tolerates_embedded_whitespace_and_missing_padding() {
        let xml = "<PathOfBuilding/>";
        let code = encode(xml);
        // Simulate a wrapped paste and stripped padding.
        let mangled = format!("{}\n  {}", &code[..4], code[4..].replace('=', ""));
        let decoded = decode_import_code(&mangled).expect("decode");
        assert_eq!(String::from_utf8(decoded).unwrap(), xml);
    }

    #[test]
    fn rejects_non_base64() {
        assert!(matches!(
            decode_import_code("not valid base64 !!!"),
            Err(PobError::Base64(_))
        ));
    }

    #[test]
    fn rejects_base64_that_is_not_zlib() {
        // Valid base64, but the bytes are not a zlib stream.
        let code = base64::engine::general_purpose::URL_SAFE.encode(b"hello world");
        assert!(matches!(
            decode_import_code(&code),
            Err(PobError::Inflate(_))
        ));
    }
}
