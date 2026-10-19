use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub fn bounded_file(path: &Path, maximum: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(io::Error::other("fixture file exceeds bound"));
    }
    Ok(bytes)
}
