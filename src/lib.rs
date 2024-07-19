#![no_std]

//! This crate is a pure-Rust reimplementation of [FastLZ](https://github.com/ariya/FastLZ).
//!
//! This crate uses the same fundamental algorithm as the original C code,
//! namely using a hash table keyed off of the next three bytes to try to find backreferences.
//! Just like FastLZ (and unlike "traditional" implementations of DEFLATE such as gzip),
//! no chaining is used in the hashtable, only a single entry per hash key.
//!
//! This crate does not generate bit-identical output, but output should be fully compatible
//! with other decoders, at least for compression level 1.
//!
//! Compression level 2 is not formally documented, but this crate implements it as follows:
//! * If `opc[7:5] == 0b000`, copy `opc[4:0] + 1` of the following literals
//! * Else it is a backreference. Set the initial `len` to `opc[7:5] + 2`
//!   and the initial `disp[12:8]` to `opc[4:0]`
//!     * If `opc[7:5] == 0b111` then there is an extended length.
//!       `len` += all bytes until and including the first non-0xff byte
//!     * Set the initial `disp[7:0]` to the next byte
//!     * If the initial `disp` is all 1 bits, `disp` += the next two bytes as a big-endian integer
//! * A file is, for some reason, not permitted to end on a backreference requiring extended displacement bytes
//!
//! Like the original code, this crate does not support "streaming" compression.
//! It only operates on full input.

mod compress;
pub use compress::{CompressError, CompressState, CompressionLevel};

mod decompress;
#[cfg(feature = "alloc")]
pub use decompress::decompress_to_vec;
pub use decompress::{decompress_to_buf, DecompressError};

mod util;
