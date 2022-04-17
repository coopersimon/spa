use clap::Parser;

use std::{
    io::{
        Read,
        Seek,
        SeekFrom,
        Write
    },
    fs::File,
};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    command: String,

    /// Encrypt the firmware or ROM secure area.
    #[clap(short, long)]
    encrypt: Option<String>,

    /// Decrypt the firmware or ROM secure area.
    #[clap(short, long)]
    decrypt: Option<String>,

    /// Output file location for crypto.
    #[clap(short, long)]
    output: Option<String>,

    /// Logging.
    #[clap(short, long)]
    verbose: bool
}

/// Root key.
const KEY_1: &[u8] = include_bytes!("key.bin");

fn main() {
    let args = Args::parse();

    let verbose = args.verbose;

    if let Some(file_path) = args.encrypt {
        match args.command.as_str() {
            "firmware" => encrypt_firmware(file_path, args.output.expect("specify output path"), verbose),
            "rom" => println!("TODO"),
            _ => panic!("commands firmware + rom supported")
        }
    } else if let Some(file_path) = args.decrypt {
        match args.command.as_str() {
            "firmware" => decrypt_firmware(file_path, args.output.expect("specify output path"), verbose),
            "rom" => println!("TODO"),
            _ => panic!("commands firmware + rom supported")
        }
    } else {
        println!("Specify encrypt or decrypt.");
    }
}

fn encrypt_firmware(file_path: String, out_path: String, verbose: bool) {
    let mut rom_file = File::open(file_path).expect("couldn't open firmware");
    let mut firmware_data = Vec::new();

    rom_file.seek(SeekFrom::Start(0)).expect("couldn't seek");
    rom_file.read_to_end(&mut firmware_data).expect("couldn't read");

    if firmware_data.len() != (256 * 1024) {
        panic!("expected firmware to be 256KiB, was {}B", firmware_data.len());
    }

    let id_bytes = &firmware_data[0x8..0xC];
    let id_code = u32::from_le_bytes(id_bytes.try_into().unwrap());
    if verbose {
        println!("ID Code: ${:X} ({})", id_code, String::from_utf8(id_bytes.to_vec()).unwrap());
    }

    let root_key = KEY_1.chunks(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    // Key: level 2, with modulo 3.
    let key = dscrypto::key1::init(id_code, &root_key, 3, 2);

    // First 0x200 bytes should not be encrypted.
    let mut output_buffer = firmware_data[0..0x200].to_vec();
    
    for encrypted_block in firmware_data[0x200..]
        .chunks(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
        .map(|block| dscrypto::key1::encrypt(block, &key)) {
        
        for byte in encrypted_block.to_le_bytes() {
            output_buffer.push(byte);
        }
    }

    File::create(&out_path).expect("couldn't open output file")
        .write_all(&output_buffer).expect("couldn't write to output file");

    if verbose {
        println!("Encrypted {} bytes to {}", output_buffer.len(), out_path);
    }
}

fn decrypt_firmware(file_path: String, out_path: String, verbose: bool) {
    let mut rom_file = File::open(file_path).expect("couldn't open firmware");
    let mut firmware_data = Vec::new();

    rom_file.seek(SeekFrom::Start(0)).expect("couldn't seek");
    rom_file.read_to_end(&mut firmware_data).expect("couldn't read");

    if firmware_data.len() != (256 * 1024) {
        panic!("expected firmware to be 256KiB, was {}B", firmware_data.len());
    }

    let id_bytes = &firmware_data[0x8..0xC];
    let id_code = u32::from_le_bytes(id_bytes.try_into().unwrap());
    if verbose {
        println!("ID Code: ${:X} ({})", id_code, String::from_utf8(id_bytes.to_vec()).unwrap());
    }

    let root_key = KEY_1.chunks(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    // Key: level 2, with modulo 3.
    let key = dscrypto::key1::init(id_code, &root_key, 3, 2);

    // First 0x200 bytes should not be decrypted.
    let mut output_buffer = firmware_data[0..0x200].to_vec();
    
    for decrypted_block in firmware_data[0x200..]
        .chunks(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
        .map(|block| dscrypto::key1::decrypt(block, &key)) {
        
        for byte in decrypted_block.to_le_bytes() {
            output_buffer.push(byte);
        }
    }

    File::create(&out_path).expect("couldn't open output file")
        .write_all(&output_buffer).expect("couldn't write to output file");

    if verbose {
        println!("Decrypted {} bytes to {}", output_buffer.len(), out_path);
    }
}
