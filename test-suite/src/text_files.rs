use std::fs;
use std::io;
use std::path::Path;

/// Returns `contents` with a trailing newline appended when non-empty and missing one.
pub fn ensure_trailing_newline(contents: String) -> String {
    if !contents.is_empty() && !contents.ends_with('\n') {
        let mut with_newline = contents;
        with_newline.push('\n');
        with_newline
    } else {
        contents
    }
}

/// Writes `contents` to `path`, ensuring non-empty files end with a newline.
pub fn write_text_with_trailing_newline(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, ensure_trailing_newline(contents.to_string()))
}
