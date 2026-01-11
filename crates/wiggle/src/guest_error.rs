use std::fmt::Debug;

use crate::{Region, Width};

#[derive(Debug, thiserror::Error)]
pub enum GuestError<W: Width> {
    #[error("Invalid flag value {0}")]
    InvalidFlagValue(&'static str),
    #[error("Invalid enum value {0}")]
    InvalidEnumValue(&'static str),
    #[error("Pointer overflow")]
    PtrOverflow,
    #[error("Pointer out of bounds: {0:?}")]
    PtrOutOfBounds(Region<W>),
    #[error("Pointer not aligned to {1}: {0:?}")]
    PtrNotAligned(Region<W>, u32),
    #[error("Slice length mismatch")]
    SliceLengthsDiffer,
    #[error("In func {modulename}::{funcname} at {location}: {err}")]
    InFunc {
        modulename: &'static str,
        funcname: &'static str,
        location: &'static str,

        #[source]
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Invalid UTF-8 encountered: {0:?}")]
    InvalidUtf8(#[from] ::std::str::Utf8Error),
    #[error("Int conversion error: {0:?}")]
    TryFromIntError(#[from] ::std::num::TryFromIntError),
}
