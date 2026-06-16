use std::fs;
use std::io;
use std::path::Path;

/// Returns `contents` with a trailing newline appended when non-empty and missing one.
pub fn ensure_trailing_newline(mut contents: String) -> String {
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents
}

/// Writes `contents` to `path`, ensuring non-empty files end with a newline.
pub fn write_text_with_trailing_newline(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, ensure_trailing_newline(contents.to_string()))
}

/// Writes `contents` to `path`, ensuring non-empty files end with a newline.
pub fn write_bytes_with_trailing_newline(path: &Path, contents: &[u8]) -> io::Result<()> {
    let text = String::from_utf8_lossy(contents);
    write_text_with_trailing_newline(path, text.as_ref())
}
