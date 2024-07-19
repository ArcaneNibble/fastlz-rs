use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::File,
    io::{BufWriter, Write},
};

use fastlz_rs::*;

#[cfg(feature = "std")]
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<OsString> = env::args_os().collect();

    if args.len() < 4 {
        println!("Usage: {} c|C|d input output", args[0].to_string_lossy());
        return Ok(());
    }

    let mode = &args[1];
    let inp_fn = &args[2];
    let outp_fn = &args[3];

    let inp = std::fs::read(inp_fn)?;
    let outp;

    match mode.to_str() {
        Some("c") => {
            let mut cmp = CompressState::new();
            outp = cmp.compress_to_vec(&inp, CompressionLevel::Level1).unwrap();
        }
        Some("C") => {
            let mut cmp = CompressState::new();
            outp = cmp.compress_to_vec(&inp, CompressionLevel::Level2).unwrap();
        }
        Some("d") => {
            outp = decompress_to_vec(&inp, None).unwrap();
        }
        _ => {
            println!("Invalid mode {}", mode.to_string_lossy());
            return Ok(());
        }
    }

    let mut outp_f = BufWriter::new(File::create(&outp_fn).unwrap());
    outp_f.write(&outp).unwrap();

    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    println!("Demo requires std feature");
}
