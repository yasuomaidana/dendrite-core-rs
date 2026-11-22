//! What `pe.parse` and `bin.functions` share: reading an image from disk and the error and address helpers.

use std::fs;

use crate::error::CoreError;

/// A larger file is refused before it is read.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

pub fn hex(value: u64) -> String {
    format!("{value:#x}")
}

pub fn not_pe(why: impl ToString) -> CoreError {
    CoreError::NotPe(why.to_string())
}

/// Turns an I/O error on `path` into the error of an image that cannot be read.
fn unreadable(path: &str) -> impl Fn(std::io::Error) -> CoreError + '_ {
    move |e| not_pe(format!("cannot read `{path}`: {e}"))
}

/// What the file system says about `path`; only a regular file of at most [`MAX_FILE_BYTES`] passes.
pub fn stat(path: &str) -> Result<fs::Metadata, CoreError> {
    let meta = fs::metadata(path).map_err(unreadable(path))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(not_pe(format!(
            "`{path}` is not a regular file of at most {MAX_FILE_BYTES} bytes"
        )));
    }
    Ok(meta)
}

/// The bytes of the file at `path`, after the checks of [`stat`].
pub fn read_file(path: &str) -> Result<Vec<u8>, CoreError> {
    stat(path)?;
    fs::read(path).map_err(unreadable(path))
}
