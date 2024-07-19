#![no_std]

mod decompress;
#[cfg(feature = "alloc")]
pub use decompress::decompress_to_vec;
pub use decompress::{decompress_to_buf, DecompressError};
