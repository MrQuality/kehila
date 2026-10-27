//! Linux opened-object checks under separately established trusted-root premises.
use std::{ffi::CStr, os::fd::BorrowedFd};

use rustix::{
    fd::OwnedFd,
    fs::{fstat, openat2, FileType, Mode, OFlags, ResolveFlags},
    io::Errno,
};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
pub(super) struct Owner {
    pub uid: u32,
    pub gid: u32,
}

/// Categories and errno only; never retain a configured path or secret bytes.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum FileError {
    Open(Errno),
    Stat(Errno),
    ObjectType,
    Owner,
    Mode,
    Allocation,
    Read(Errno),
    TooLarge,
}

fn open_checked(root: BorrowedFd<'_>, path: &CStr, owner: Owner) -> Result<OwnedFd, FileError> {
    let fd = openat2(
        root,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(FileError::Open)?;
    let metadata = fstat(&fd).map_err(FileError::Stat)?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(FileError::ObjectType);
    }
    if metadata.st_uid != owner.uid || metadata.st_gid != owner.gid {
        return Err(FileError::Owner);
    }
    // Compare the entire raw mode: a truncating bitflags conversion could hide bits.
    if metadata.st_mode != (FileType::RegularFile.as_raw_mode() | 0o400) {
        return Err(FileError::Mode);
    }
    Ok(fd)
}

fn read_bounded(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, Errno>,
) -> Result<Zeroizing<Vec<u8>>, FileError> {
    let cap = super::MAX_ENCODED + 1;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(cap)
        .map_err(|_| FileError::Allocation)?;
    bytes.resize(cap, 0);
    let mut filled = 0;
    loop {
        match read(&mut bytes[filled..]) {
            Ok(0) => {
                bytes.truncate(filled);
                return Ok(bytes);
            }
            Ok(length) => {
                filled += length;
                if filled > super::MAX_ENCODED {
                    return Err(FileError::TooLarge);
                }
            }
            Err(Errno::INTR) => continue,
            Err(error) => return Err(FileError::Read(error)),
        }
    }
}

/// Borrow the established root, own one opened descriptor, and read it once.
/// The byte cap is not an I/O deadline or proof against a writable alias.
pub(super) fn read_bundle(
    root: BorrowedFd<'_>,
    path: &CStr,
    owner: Owner,
) -> Result<Zeroizing<Vec<u8>>, FileError> {
    let fd = open_checked(root, path, owner)?;
    read_bounded(|output| rustix::io::read(&fd, output))
}

#[cfg(test)]
mod tests;
