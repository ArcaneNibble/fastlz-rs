#[cfg(feature = "alloc")]
extern crate alloc;

/// Internal abstraction for types of outputs (slice vs Vec)
///
/// Note for all functions: we guarantee writing all the way up to the limit
pub trait OutputSink<ErrTy> {
    /// Add the given literal run to the output
    ///
    /// If this would overflow the output, return Err(ErrTy::OutputTooSmall).
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), ErrTy>;
    /// Add a backreference to the output
    ///
    /// A `disp` of 0 means the current position minus 1.
    /// Increasing `disp` means further backwards
    ///
    /// Copy `len` bytes, which as usual for LZ77 may exceed `disp`.
    fn put_backref(&mut self, disp: usize, len: usize) -> Result<(), ErrTy>;
}

pub struct BufOutput<'a> {
    pub pos: usize,
    pub buf: &'a mut [u8],
}
impl<'a> From<&'a mut [u8]> for BufOutput<'a> {
    fn from(buf: &'a mut [u8]) -> Self {
        Self { pos: 0, buf }
    }
}

#[cfg(feature = "alloc")]
pub struct VecOutput {
    pub vec: alloc::vec::Vec<u8>,
}
#[cfg(feature = "alloc")]
impl From<alloc::vec::Vec<u8>> for VecOutput {
    fn from(vec: alloc::vec::Vec<u8>) -> Self {
        Self { vec }
    }
}
