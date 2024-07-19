use core::fmt;

use crate::util::*;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompressError {
    OutputTooSmall,
}
impl fmt::Display for CompressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressError::OutputTooSmall => write!(f, "output buffer was insufficient"),
        }
    }
}
#[cfg(feature = "std")]
impl std::error::Error for CompressError {}

trait OutputHelper {
    fn putc(&mut self, c: u8) -> Result<(), CompressError>;
    fn put_buf(&mut self, buf: &[u8]) -> Result<(), CompressError>;
}
impl<'a> OutputHelper for BufOutput<'a> {
    fn putc(&mut self, c: u8) -> Result<(), CompressError> {
        if self.pos + 1 <= self.buf.len() {
            self.buf[self.pos] = c;
            self.pos += 1;
            Ok(())
        } else {
            Err(CompressError::OutputTooSmall)
        }
    }
    fn put_buf(&mut self, buf: &[u8]) -> Result<(), CompressError> {
        let mut len = buf.len();
        let mut did_overflow = false;
        if self.pos + len > self.buf.len() {
            did_overflow = true;
            len = self.buf.len() - self.pos;
        }

        self.buf[self.pos..self.pos + len].copy_from_slice(&buf[..len]);
        self.pos += len;

        if did_overflow {
            Err(CompressError::OutputTooSmall)
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "alloc")]
impl OutputHelper for VecOutput {
    fn putc(&mut self, c: u8) -> Result<(), CompressError> {
        self.vec.push(c);
        Ok(())
    }
    fn put_buf(&mut self, buf: &[u8]) -> Result<(), CompressError> {
        self.vec.extend_from_slice(buf);
        Ok(())
    }
}

struct L1Output<O>(O);
struct L2Output<O>(O);

impl<O: OutputHelper> OutputSink<CompressError> for L1Output<O> {
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), CompressError> {
        let len = lits.len();
        debug_assert!(len >= 1);
        debug_assert!(len <= 32);

        // 1 byte opcode, len bytes literals
        self.0.putc((len - 1) as u8)?;
        self.0.put_buf(lits)?;

        Ok(())
    }

    fn put_backref(&mut self, disp: usize, len: usize) -> Result<(), CompressError> {
        debug_assert!(disp <= 8191);
        debug_assert!((3..=264).contains(&len));

        if len <= 8 {
            // 2 bytes opcode
            let b0 = (((len - 2) << 5) | (disp >> 8)) as u8;
            let b1 = disp as u8;
            self.0.putc(b0)?;
            self.0.putc(b1)?;
        } else {
            // 3 bytes opcode
            let b0 = 0b111_00000 | ((disp >> 8) as u8);
            let b1 = (len - 9) as u8;
            let b2 = disp as u8;
            self.0.putc(b0)?;
            self.0.putc(b1)?;
            self.0.putc(b2)?;
        }

        Ok(())
    }
}

impl<O: OutputHelper> OutputSink<CompressError> for L2Output<O> {
    fn put_lits(&mut self, lits: &[u8]) -> Result<(), CompressError> {
        let len = lits.len();
        debug_assert!(len >= 1);
        debug_assert!(len <= 32);

        // 1 byte opcode, len bytes literals
        self.0.putc((len - 1) as u8)?;
        self.0.put_buf(lits)?;

        Ok(())
    }

    fn put_backref(&mut self, disp: usize, mut len: usize) -> Result<(), CompressError> {
        debug_assert!(disp <= 8191 + 65535);
        debug_assert!(len >= 3);

        let earlydisp = usize::min(disp, 8191);
        len -= 2;
        let earlylen = usize::min(len, 7);

        let b0 = ((earlylen << 5) | (earlydisp >> 8)) as u8;
        self.0.putc(b0)?;

        if earlylen == 7 {
            len -= earlylen;
            loop {
                let blen = usize::min(len, 0xff) as u8;
                self.0.putc(blen)?;
                if blen != 0xff {
                    break;
                }
                len -= blen as usize;
            }
        }

        self.0.putc(earlydisp as u8)?;
        if earlydisp == 8191 {
            let moredisp = disp - earlydisp;
            self.0.putc((moredisp >> 8) as u8)?;
            self.0.putc(moredisp as u8)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    Default,
    Level1,
    Level2,
}
impl Default for CompressionLevel {
    fn default() -> Self {
        Self::Default
    }
}

pub struct CompressState {
    // nothing yet
}
impl CompressState {
    pub fn new() -> Self {
        Self {}
    }
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> alloc::boxed::Box<Self> {
        // *sigh* things that aren't stable, workaround, bleh
        unsafe {
            let self_ = alloc::alloc::alloc(core::alloc::Layout::new::<Self>()) as *mut Self;
            alloc::boxed::Box::from_raw(self_)
        }
    }

    fn compress_impl(
        &mut self,
        inp: &[u8],
        outp: &mut impl OutputSink<CompressError>,
    ) -> Result<(), CompressError> {
        todo!()
    }

    #[allow(private_bounds)]
    pub fn compress_to_buf(
        &mut self,
        inp: &[u8],
        outp: &mut [u8],
        mut level: CompressionLevel,
    ) -> Result<usize, CompressError> {
        if level == CompressionLevel::Default {
            if inp.len() < 65536 {
                level = CompressionLevel::Level1;
            } else {
                level = CompressionLevel::Level2;
            }
        }

        if level == CompressionLevel::Level1 {
            let mut outp: L1Output<BufOutput> = L1Output(outp.into());
            self.compress_impl(inp, &mut outp)?;
            Ok(outp.0.pos)
        } else {
            let mut outp: L2Output<BufOutput> = L2Output(outp.into());
            self.compress_impl(inp, &mut outp)?;
            Ok(outp.0.pos)
        }
    }

    #[cfg(feature = "alloc")]
    #[allow(private_bounds)]
    pub fn compress_to_vec(
        &mut self,
        inp: &[u8],
        mut level: CompressionLevel,
    ) -> Result<alloc::vec::Vec<u8>, CompressError> {
        let ret = alloc::vec::Vec::new();
        if level == CompressionLevel::Default {
            if inp.len() < 65536 {
                level = CompressionLevel::Level1;
            } else {
                level = CompressionLevel::Level2;
            }
        }

        if level == CompressionLevel::Level1 {
            let mut ret: L1Output<VecOutput> = L1Output(ret.into());
            self.compress_impl(inp, &mut ret)?;
            Ok(ret.0.vec)
        } else {
            let mut ret: L2Output<VecOutput> = L2Output(ret.into());
            self.compress_impl(inp, &mut ret)?;
            Ok(ret.0.vec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lv1_encoding_lit() {
        {
            let mut out = [0u8; 3];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_lits(&[1, 2]).unwrap();
            assert_eq!(outbuf.0.buf, [0x01, 1, 2]);
        }

        {
            let mut out = [0u8; 2];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_lits(&[1, 2]).expect_err("");
            assert_eq!(outbuf.0.buf, [0x01, 1]);
        }

        {
            let mut out = [0u8; 0];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_lits(&[0]).expect_err("");
        }
    }

    #[test]
    fn test_lv1_encoding_short() {
        {
            let mut out = [0u8; 2];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_backref(1, 5).unwrap();
            assert_eq!(outbuf.0.buf, [0x60, 0x01]);
        }

        {
            let mut out = [0u8; 1];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_backref(1, 5).expect_err("");
            assert_eq!(outbuf.0.buf, [0x60]);
        }
    }

    #[test]
    fn test_lv1_encoding_long() {
        {
            let mut out = [0u8; 3];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_backref(1, 9).unwrap();
            assert_eq!(outbuf.0.buf, [0xe0, 0x00, 0x01]);
        }

        {
            let mut out = [0u8; 1];
            let mut outbuf: L1Output<BufOutput> = L1Output((&mut out[..]).into());
            outbuf.put_backref(1, 9).expect_err("");
            assert_eq!(outbuf.0.buf, [0xe0]);
        }
    }

    #[test]
    fn test_lv2_encoding_lit() {
        {
            let mut out = [0u8; 3];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_lits(&[1, 2]).unwrap();
            assert_eq!(outbuf.0.buf, [0x01, 1, 2]);
        }

        {
            let mut out = [0u8; 2];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_lits(&[1, 2]).expect_err("");
            assert_eq!(outbuf.0.buf, [0x01, 1]);
        }

        {
            let mut out = [0u8; 0];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_lits(&[0]).expect_err("");
        }
    }

    #[test]
    fn test_lv2_encoding_short() {
        {
            let mut out = [0u8; 2];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(1, 5).unwrap();
            assert_eq!(outbuf.0.buf, [0x60, 0x01]);
        }

        {
            let mut out = [0u8; 1];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(1, 5).expect_err("");
            assert_eq!(outbuf.0.buf, [0x60]);
        }
    }

    #[test]
    fn test_lv2_encoding_longlen() {
        {
            let mut out = [0u8; 3];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(1, 9).unwrap();
            assert_eq!(outbuf.0.buf, [0xe0, 0x00, 0x01]);
        }

        {
            let mut out = [0u8; 4];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(2, 9 + 0xff + 1).unwrap();
            assert_eq!(outbuf.0.buf, [0xe0, 0xff, 0x01, 0x02]);
        }
    }

    #[test]
    fn test_lv2_encoding_longdisp() {
        {
            let mut out = [0u8; 4];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(8191, 3).unwrap();
            assert_eq!(outbuf.0.buf, [0x3f, 0xff, 0x00, 0x00]);
        }

        {
            let mut out = [0u8; 4];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(8192, 3).unwrap();
            assert_eq!(outbuf.0.buf, [0x3f, 0xff, 0x00, 0x01]);
        }
    }

    #[test]
    fn test_lv2_encoding_longboth() {
        {
            let mut out = [0u8; 6];
            let mut outbuf: L2Output<BufOutput> = L2Output((&mut out[..]).into());
            outbuf.put_backref(8192, 9 + 0xff + 1).unwrap();
            assert_eq!(outbuf.0.buf, [0xff, 0xff, 0x01, 0xff, 0x00, 0x01]);
        }
    }
}
