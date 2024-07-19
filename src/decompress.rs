use core::fmt::{self};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecompressError {
    InputTruncated,
    InvalidBackreference,
    OutputTooSmall,
}

impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecompressError::InputTruncated => write!(f, "input was truncated"),
            DecompressError::InvalidBackreference => write!(f, "invalid backreference"),
            DecompressError::OutputTooSmall => write!(f, "output buffer was insufficient"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecompressError {}

/// Internal abstraction for the two different types of outputs
///
/// Note for both functions: we guarantee writing all the way up to the limit
trait OutputSink {
    /// Add the given literal run to the output
    ///
    /// If this would overflow the output, return Err.
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), ()>;
    /// Add a backreference to the output
    ///
    /// A `disp` of 0 means the current position minus 1.
    /// Increasing `disp` means further backwards
    ///
    /// Copy `len` bytes, which as usual for LZ77 may exceed `disp`.
    fn put_backref(&mut self, disp: usize, len: usize) -> Result<(), DecompressError>;
}

struct BufOutput<'a> {
    pos: usize,
    buf: &'a mut [u8],
}
impl<'a> From<&'a mut [u8]> for BufOutput<'a> {
    fn from(buf: &'a mut [u8]) -> Self {
        Self { pos: 0, buf }
    }
}
impl<'a> OutputSink for BufOutput<'a> {
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), ()> {
        let mut len = lits.len();
        let mut did_overflow = false;
        if self.pos + len > self.buf.len() {
            did_overflow = true;
            len = self.buf.len() - self.pos;
        }

        self.buf[self.pos..self.pos + len].copy_from_slice(&lits[..len]);
        self.pos += len;

        if did_overflow {
            Err(())
        } else {
            Ok(())
        }
    }

    fn put_backref(&mut self, disp: usize, mut len: usize) -> Result<(), DecompressError> {
        if disp + 1 > self.pos {
            return Err(DecompressError::InvalidBackreference);
        }

        let mut did_overflow = false;
        if self.pos + len > self.buf.len() {
            did_overflow = true;
            len = self.buf.len() - self.pos;
        }

        for i in 0..len {
            self.buf[self.pos + i] = self.buf[self.pos - disp - 1 + i];
        }
        self.pos += len;

        if did_overflow {
            Err(DecompressError::OutputTooSmall)
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "alloc")]
struct VecOutput {
    vec: alloc::vec::Vec<u8>,
}
#[cfg(feature = "alloc")]
impl From<alloc::vec::Vec<u8>> for VecOutput {
    fn from(vec: alloc::vec::Vec<u8>) -> Self {
        Self { vec }
    }
}
#[cfg(feature = "alloc")]
impl OutputSink for VecOutput {
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), ()> {
        let pos = self.vec.len();
        self.vec.resize(pos + lits.len(), 0);
        self.vec[pos..pos + lits.len()].copy_from_slice(lits);
        Ok(())
    }

    fn put_backref(&mut self, disp: usize, len: usize) -> Result<(), DecompressError> {
        let pos = self.vec.len();
        if disp + 1 > pos {
            return Err(DecompressError::InvalidBackreference);
        }

        self.vec.resize(pos + len, 0);
        for i in 0..len {
            self.vec[pos + i] = self.vec[pos - disp - 1 + i];
        }

        Ok(())
    }
}

fn decompress_impl(inp: &[u8], outp: &mut impl OutputSink) -> Result<(), DecompressError> {
    todo!()
}

pub fn decompress_to_buf(inp: &[u8], outp: &mut [u8]) -> Result<usize, DecompressError> {
    let mut outp: BufOutput = outp.into();
    decompress_impl(inp, &mut outp)?;
    Ok(outp.pos)
}

#[cfg(feature = "alloc")]
pub fn decompress_to_vec(
    inp: &[u8],
    capacity_hint: Option<usize>,
) -> Result<alloc::vec::Vec<u8>, DecompressError> {
    let mut ret: VecOutput = if let Some(capacity_hint) = capacity_hint {
        alloc::vec::Vec::with_capacity(capacity_hint)
    } else {
        alloc::vec::Vec::new()
    }
    .into();
    decompress_impl(inp, &mut ret)?;
    Ok(ret.vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buf_out_lits() {
        {
            let mut out = [0u8; 3];
            let mut outbuf: BufOutput = (&mut out[..]).into();
            outbuf.put_lits(&[1]).unwrap();
            assert_eq!(outbuf.buf, [1, 0, 0]);
            // test overflow
            outbuf.put_lits(&[2, 3, 4]).expect_err("");
            assert_eq!(outbuf.buf, [1, 2, 3]);
        }

        {
            let mut out = [0u8; 3];
            let mut outbuf: BufOutput = (&mut out[..]).into();
            // test exact fit
            outbuf.put_lits(&[1, 2, 3]).unwrap();
            assert_eq!(outbuf.buf, [1, 2, 3]);
            outbuf.put_lits(&[4]).expect_err("");
            assert_eq!(outbuf.buf, [1, 2, 3]);
        }
    }

    #[test]
    fn test_buf_out_backref() {
        {
            let mut out = [0u8; 8];
            let mut outbuf: BufOutput = (&mut out[..]).into();
            outbuf.put_lits(&[1, 2, 3]).unwrap();

            // invalid, before the start
            assert_eq!(
                outbuf.put_backref(3, 5),
                Err(DecompressError::InvalidBackreference)
            );

            // overflow, but should still write up to limit
            assert_eq!(
                outbuf.put_backref(1, 6),
                Err(DecompressError::OutputTooSmall)
            );

            assert_eq!(outbuf.buf, [1, 2, 3, 2, 3, 2, 3, 2])
        }

        {
            let mut out = [0u8; 8];
            let mut outbuf: BufOutput = (&mut out[..]).into();
            outbuf.put_lits(&[1, 2, 3]).unwrap();

            // exact fit
            outbuf.put_backref(2, 5).unwrap();
            assert_eq!(outbuf.buf, [1, 2, 3, 1, 2, 3, 1, 2]);
        }

        // note: we already tested the "hard" case of len > disp
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_out_lits() {
        let out = alloc::vec::Vec::new();
        let mut outbuf: VecOutput = out.into();
        outbuf.put_lits(&[1]).unwrap();
        assert_eq!(outbuf.vec, [1]);
        outbuf.put_lits(&[2, 3, 4]).unwrap();
        assert_eq!(outbuf.vec, [1, 2, 3, 4]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_out_backref() {
        let out = alloc::vec::Vec::new();
        let mut outbuf: VecOutput = out.into();
        outbuf.put_lits(&[1, 2, 3]).unwrap();
        outbuf.put_backref(1, 6).unwrap();
        assert_eq!(outbuf.vec, [1, 2, 3, 2, 3, 2, 3, 2, 3]);
    }
}
