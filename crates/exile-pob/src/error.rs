//! Errors from importing a Path of Building build. One focused [`thiserror`] enum; the caller (the
//! Tauri shell) flattens it to a string at the IPC boundary.

use std::path::PathBuf;

/// Everything that can go wrong decoding or parsing a PoB build.
#[derive(Debug, thiserror::Error)]
pub enum PobError {
    /// The import code was not valid base64 (after url-safe normalization).
    #[error("failed to base64-decode the import code: {0}")]
    Base64(#[from] base64::DecodeError),

    /// The decoded bytes were not a valid zlib stream.
    #[error("failed to zlib-inflate the import code: {0}")]
    Inflate(std::io::Error),

    /// The XML was malformed.
    #[error("failed to parse Path of Building XML: {0}")]
    Xml(#[from] quick_xml::Error),

    /// The document parsed, but its root element was not `<PathOfBuilding>`.
    #[error("not a Path of Building document (root element was <{found}>)")]
    NotPob { found: String },

    /// Reading a local PoB file failed.
    #[error("failed to read PoB file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
