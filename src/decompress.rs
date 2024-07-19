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
    InvalidCompressionLevel,
    OutputTooSmall,
}

impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecompressError::InputTruncated => write!(f, "input was truncated"),
            DecompressError::InvalidBackreference => write!(f, "invalid backreference"),
            DecompressError::InvalidCompressionLevel => write!(f, "invalid compression level"),
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
    /// If this would overflow the output, return Err(DecompressError::OutputTooSmall).
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), DecompressError>;
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
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), DecompressError> {
        let mut len = lits.len();
        let mut did_overflow = false;
        if self.pos + len > self.buf.len() {
            did_overflow = true;
            len = self.buf.len() - self.pos;
        }

        self.buf[self.pos..self.pos + len].copy_from_slice(&lits[..len]);
        self.pos += len;

        if did_overflow {
            Err(DecompressError::OutputTooSmall)
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
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), DecompressError> {
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

trait InputHelper {
    fn getc(&mut self) -> Result<u8, DecompressError>;
    fn check_len(&mut self, min: usize) -> Result<(), DecompressError>;
}
impl InputHelper for &[u8] {
    fn getc(&mut self) -> Result<u8, DecompressError> {
        if self.len() == 0 {
            return Err(DecompressError::InputTruncated);
        }
        let c = self[0];
        *self = &self[1..];
        Ok(c)
    }

    fn check_len(&mut self, min: usize) -> Result<(), DecompressError> {
        if self.len() < min {
            Err(DecompressError::InputTruncated)
        } else {
            Ok(())
        }
    }
}

fn decompress_lv1(mut inp: &[u8], outp: &mut impl OutputSink) -> Result<(), DecompressError> {
    // special for first control byte
    let mut ctrl = inp.getc().unwrap() & 0b000_11111;
    loop {
        if ctrl >> 5 == 0b000 {
            // literal run
            let len = (ctrl & 0b000_11111) as usize + 1;
            inp.check_len(len)?;
            outp.put_lits(&inp[..len])?;
            inp = &inp[len..];
        } else {
            // backreference
            let mut disp = ((ctrl & 0b000_11111) as usize) << 8;
            let len = if ctrl >> 5 == 0b111 {
                // long match
                inp.getc()? as usize + 9
            } else {
                (ctrl >> 5) as usize + 2
            };
            disp |= inp.getc()? as usize;
            outp.put_backref(disp, len)?;
        }

        if let Ok(c) = inp.getc() {
            ctrl = c;
        } else {
            return Ok(());
        }
    }
}

fn decompress_lv2(mut inp: &[u8], outp: &mut impl OutputSink) -> Result<(), DecompressError> {
    // special for first control byte
    let mut ctrl = inp.getc().unwrap() & 0b000_11111;
    loop {
        if ctrl >> 5 == 0b000 {
            // literal run
            let len = (ctrl & 0b000_11111) as usize + 1;
            inp.check_len(len)?;
            outp.put_lits(&inp[..len])?;
            inp = &inp[len..];
        } else {
            // backreference
            let mut disp = ((ctrl & 0b000_11111) as usize) << 8;

            let mut len = (ctrl >> 5) as usize + 2;
            if ctrl >> 5 == 0b111 {
                // long match
                loop {
                    let morelen = inp.getc()?;
                    len += morelen as usize;
                    if morelen != 0xff {
                        break;
                    }
                }
            }

            disp |= inp.getc()? as usize;
            if disp == 0b11111_11111111 {
                let moredisp = ((inp.getc()? as usize) << 8) | (inp.getc()? as usize);
                disp += moredisp;
            }

            outp.put_backref(disp, len)?;
        }

        if let Ok(c) = inp.getc() {
            ctrl = c;
        } else {
            return Ok(());
        }
    }
}

fn decompress_impl(inp: &[u8], outp: &mut impl OutputSink) -> Result<(), DecompressError> {
    if inp.len() == 0 {
        return Ok(());
    }

    match inp[0] >> 5 {
        0 => decompress_lv1(inp, outp),
        1 => decompress_lv2(inp, outp),
        _ => Err(DecompressError::InvalidCompressionLevel),
    }
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

    #[test]
    fn test_lv1_manual_lits() {
        let mut out = [0u8; 5];
        let len = decompress_to_buf(&[0x01, b'A', b'B', 0x02, b'C', b'D', b'E'], &mut out).unwrap();
        assert_eq!(len, 5);
        assert_eq!(out, [b'A', b'B', b'C', b'D', b'E']);
    }

    #[test]
    fn test_lv1_manual_short_match() {
        let mut out = [0u8; 5];
        let len = decompress_to_buf(&[0x01, b'A', b'B', 0x20, 0x01], &mut out).unwrap();
        assert_eq!(len, 5);
        assert_eq!(out, [b'A', b'B', b'A', b'B', b'A']);
    }

    #[test]
    fn test_lv1_manual_long_match() {
        let mut out = [0u8; 11];
        let len = decompress_to_buf(&[0x01, b'A', b'B', 0xe0, 0x00, 0x01], &mut out).unwrap();
        assert_eq!(len, 11);
        assert_eq!(
            out,
            [b'A', b'B', b'A', b'B', b'A', b'B', b'A', b'B', b'A', b'B', b'A']
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_lv1_against_ref() {
        extern crate std;

        let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let inp_fn = d.join("src/decompress.rs");
        let ref_fn = d.join("temp-lv1-comp.out");
        std::process::Command::new(d.join("./testtool/testtool"))
            .arg("c")
            .arg(inp_fn.to_str().unwrap())
            .arg(ref_fn.to_str().unwrap())
            .status()
            .unwrap();

        let inp = std::fs::read(inp_fn).unwrap();
        let ref_ = std::fs::read(&ref_fn).unwrap();
        let _ = std::fs::remove_file(ref_fn);

        let out = decompress_to_vec(&ref_, None).unwrap();
        assert_eq!(inp, out);
    }

    #[test]
    fn test_lv2_manual_short_match() {
        let mut out = [0u8; 5];
        let len = decompress_to_buf(&[0x21, b'A', b'B', 0x20, 0x01], &mut out).unwrap();
        assert_eq!(len, 5);
        assert_eq!(out, [b'A', b'B', b'A', b'B', b'A']);
    }

    #[test]
    fn test_lv2_manual_long_match() {
        let mut out = [0u8; 11];
        let len = decompress_to_buf(&[0x21, b'A', b'B', 0xe0, 0x00, 0x01], &mut out).unwrap();
        assert_eq!(len, 11);
        assert_eq!(
            out,
            [b'A', b'B', b'A', b'B', b'A', b'B', b'A', b'B', b'A', b'B', b'A']
        );
    }

    #[test]
    fn test_lv2_manual_verylong_match() {
        let mut out = [0u8; 266];
        let len = decompress_to_buf(&[0x21, b'A', b'B', 0xe0, 0xff, 0x00, 0x01], &mut out).unwrap();
        assert_eq!(len, 266);
        for i in 0..(266 / 2) {
            assert_eq!(out[i * 2], b'A');
            assert_eq!(out[i * 2 + 1], b'B');
        }
    }

    #[test]
    fn test_lv2_manual_verylong_disp() {
        let mut out = [0u8; 0x2004];
        let len = decompress_to_buf(
            &[
                0x21, b'A', 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x15, 0x00, 0x3F, 0xFF, 0x00, 0x00,
                0x00, b'Z',
            ],
            &mut out,
        )
        .unwrap();
        assert_eq!(len, 0x2004);
        for i in 0..0x2004 {
            if i == 0 {
                assert_eq!(out[i], b'A');
            } else if i == 0x2000 {
                assert_eq!(out[i], b'A');
            } else if i == 0x2003 {
                assert_eq!(out[i], b'Z');
            } else {
                assert_eq!(out[i], 0);
            }
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_lv2_against_ref() {
        extern crate std;

        let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let inp_fn = d.join("src/decompress.rs");
        let ref_fn = d.join("temp-lv2-comp.out");
        std::process::Command::new(d.join("./testtool/testtool"))
            .arg("C")
            .arg(inp_fn.to_str().unwrap())
            .arg(ref_fn.to_str().unwrap())
            .status()
            .unwrap();

        let inp = std::fs::read(inp_fn).unwrap();
        let ref_ = std::fs::read(&ref_fn).unwrap();
        let _ = std::fs::remove_file(ref_fn);

        let out = decompress_to_vec(&ref_, None).unwrap();
        assert_eq!(inp, out);
    }
}
