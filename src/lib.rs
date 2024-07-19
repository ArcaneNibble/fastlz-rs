#![no_std]

mod compress;
pub use compress::{CompressError, CompressState, CompressionLevel};

mod decompress;
#[cfg(feature = "alloc")]
pub use decompress::decompress_to_vec;
pub use decompress::{decompress_to_buf, DecompressError};

mod util;
