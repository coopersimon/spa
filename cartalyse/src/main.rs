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

    /// Print key header info.
    #[clap(short, long)]
    header: Option<String>,

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

    if let Some(file_path) = args.header {
        match args.command.as_str() {
            "firmware" => println!("TODO"),
            "rom" => rom_header_info(file_path),
            _ => panic!("commands firmware + rom supported")
        }
    } else if let Some(file_path) = args.encrypt {
        match args.command.as_str() {
            "firmware" => encrypt_firmware(file_path, args.output.expect("specify output path"), verbose),
            "rom" => encrypt_rom(file_path, args.output.expect("specify output path"), verbose),
            _ => panic!("commands firmware + rom supported")
        }
    } else if let Some(file_path) = args.decrypt {
        match args.command.as_str() {
            "firmware" => decrypt_firmware(file_path, args.output.expect("specify output path"), verbose),
            "rom" => decrypt_rom(file_path, args.output.expect("specify output path"), verbose),
            _ => panic!("commands firmware + rom supported")
        }
    } else {
        println!("Specify encrypt or decrypt.");
    }
}

fn rom_header_info(file_path: String) {
    let mut rom_file = File::open(file_path).expect("couldn't open ROM");
    let mut header_data = vec![0; 0x200];

    rom_file.seek(SeekFrom::Start(0)).expect("couldn't seek");
    rom_file.read_exact(&mut header_data).expect("couldn't read");

    println!("Name: {}", String::from_utf8(header_data[0..0xC].to_vec()).unwrap());
    println!("ARM9 Entry:        ${:08X}", get_u32(&header_data, 0x24));
    println!("ARM9 ROM:          ${:08X}", get_u32(&header_data, 0x20));
    println!("ARM9 RAM:          ${:08X}", get_u32(&header_data, 0x28));
    println!("ARM9 Program Size: ${:08X}", get_u32(&header_data, 0x2C));
    println!("ARM7 Entry:        ${:08X}", get_u32(&header_data, 0x34));
    println!("ARM7 ROM:          ${:08X}", get_u32(&header_data, 0x30));
    println!("ARM7 RAM:          ${:08X}", get_u32(&header_data, 0x38));
    println!("ARM7 Program Size: ${:08X}", get_u32(&header_data, 0x3C));
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

fn encrypt_rom(file_path: String, out_path: String, verbose: bool) {
    const PRE_ENCRYPT_MSG: [u8; 8] = [0x65, 0x6E, 0x63, 0x72, 0x79, 0x4F, 0x62, 0x6A];    // "encryObj"
    const NEW_ENCRYPT_MSG: [u8; 8] = [0xFF, 0xDE, 0xFF, 0xE7, 0xFF, 0xDE, 0xFF, 0xE7];

    let mut rom_file = File::open(file_path).expect("couldn't open firmware");
    let mut rom_data = Vec::new();

    rom_file.seek(SeekFrom::Start(0)).expect("couldn't seek");
    rom_file.read_to_end(&mut rom_data).expect("couldn't read");

    if verbose {
        println!("Name: {}", String::from_utf8(rom_data[0..0xC].to_vec()).unwrap());
    }

    let id_bytes = &rom_data[0xC..=0xF];
    let id_code = u32::from_le_bytes(id_bytes.try_into().unwrap());
    if verbose {
        println!("ID Code: ${:X} ({})", id_code, String::from_utf8(id_bytes.to_vec()).unwrap());
    }

    // Verify encrypted segment:
    let pre_encrypt = rom_data[0x4000..0x4008].iter().zip(&PRE_ENCRYPT_MSG)
        .fold(true, |acc, (a, b)| acc && (*a == *b));
    
    if !pre_encrypt {
        let after_encrypt = rom_data[0x4000..0x4008].iter().zip(&NEW_ENCRYPT_MSG)
            .fold(true, |acc, (a, b)| acc && (*a == *b));
        if !after_encrypt {
            println!("Invalid encryption prologue. Bailing!");
            return;
        }

        if verbose {
            println!("This ROM has been decrypted before.");
        }
        // Copy encrypt ID if it has been overwritten.
        for (dst, src) in rom_data[0x4000..0x4008].iter_mut().zip(&PRE_ENCRYPT_MSG) {
            *dst = *src;
        }
    } else if verbose {
        println!("Already encrypted with \"{}\"", String::from_utf8(PRE_ENCRYPT_MSG.to_vec()).unwrap());
    }

    // TODO: validate fixed section at start and end + CRC

    let root_key = KEY_1.chunks(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();

    // Key: level 1, with modulo 2.
    let header_key = dscrypto::key1::init(id_code, &root_key, 2, 1);

    // Encrypt the header secure area segment.
    // TODO: validate initial value here.
    let sec_header_entry = u64::from_le_bytes(rom_data[0x78..0x80].try_into().unwrap());
    let encry_sec_header_entry = dscrypto::key1::encrypt(sec_header_entry, &header_key);
    if verbose {
        println!("Encrypted secure area header entry: {:X} => {:X}", sec_header_entry, encry_sec_header_entry);
    }

    for (dst, src) in rom_data[0x78..0x80].iter_mut().zip(&encry_sec_header_entry.to_le_bytes()) {
        *dst = *src;
    }

    // Key: level 2, with modulo 2.
    let sec_init_key = dscrypto::key1::init(id_code, &root_key, 2, 2);

    // Encrypt secure area start section:
    let sec_init = u64::from_le_bytes(rom_data[0x4000..0x4008].try_into().unwrap());
    let encry_sec_init = dscrypto::key1::encrypt(sec_init, &sec_init_key);

    for (dst, src) in rom_data[0x4000..0x4008].iter_mut().zip(&encry_sec_init.to_le_bytes()) {
        *dst = *src;
    }

    // Key: level 3, with modulo 2.
    let sec_area_key = dscrypto::key1::init(id_code, &root_key, 2, 3);

    // First 0x4000 bytes should not be encrypted.
    let mut output_buffer = rom_data[0..0x4008].to_vec();

    for encrypted_block in rom_data[0x4008..0x4800]
        .chunks(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
        .map(|block| dscrypto::key1::encrypt(block, &sec_area_key)) {
        
        for byte in encrypted_block.to_le_bytes() {
            output_buffer.push(byte);
        }
    }

    // Remaining data should not be encrypted.
    output_buffer.extend_from_slice(&rom_data[0x4800..]);

    File::create(&out_path).expect("couldn't open output file")
        .write_all(&output_buffer).expect("couldn't write to output file");

    if verbose {
        println!("Encrypted {} bytes to {}", output_buffer.len(), out_path);
    }
}

fn decrypt_rom(file_path: String, out_path: String, verbose: bool) {
    let mut rom_file = File::open(file_path).expect("couldn't open firmware");
    let mut rom_data = Vec::new();

    rom_file.seek(SeekFrom::Start(0)).expect("couldn't seek");
    rom_file.read_to_end(&mut rom_data).expect("couldn't read");

    if verbose {
        println!("Name: {}", String::from_utf8(rom_data[0..0xC].to_vec()).unwrap());
    }

    let id_bytes = &rom_data[0xC..=0xF];
    let id_code = u32::from_le_bytes(id_bytes.try_into().unwrap());
    if verbose {
        println!("ID Code: ${:X} ({})", id_code, String::from_utf8(id_bytes.to_vec()).unwrap());
    }

    let root_key = KEY_1.chunks(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();

    // Key: level 1, with modulo 2.
    let header_key = dscrypto::key1::init(id_code, &root_key, 2, 1);

    // Check header section:
    let sec_header_entry = u64::from_le_bytes(rom_data[0x78..0x80].try_into().unwrap());
    let decry_sec_header_entry = dscrypto::key1::decrypt(sec_header_entry, &header_key);
    if verbose {
        println!("Decrypted secure area header entry: {:X} => {:X}", sec_header_entry, decry_sec_header_entry);
    }

    for (dst, src) in rom_data[0x78..0x80].iter_mut().zip(&decry_sec_header_entry.to_le_bytes()) {
        *dst = *src;
    }
    // TODO: if decry_sec_header_entry == "NmMdOnly" then do not decrypt further.

    // Key: level 2, with modulo 2.
    let sec_init_key = dscrypto::key1::init(id_code, &root_key, 2, 2);

    // Check secure area start section:
    let sec_init = u64::from_le_bytes(rom_data[0x4000..0x4008].try_into().unwrap());
    let decry_sec_init = dscrypto::key1::decrypt(sec_init, &sec_init_key);
    if verbose {
        println!("Decrypted secure area init value: {:X} => {:X}", sec_init, decry_sec_init);
    }
    let Ok(decry_obj_str) = String::from_utf8(decry_sec_init.to_le_bytes().to_vec()) else {
        println!("Invalid decrypted string");
        return;
    };

    if decry_obj_str != "encryObj" {
        println!("Invalid decrypted string: {}", decry_obj_str);
        return;
    }

    for (dst, src) in rom_data[0x4000..0x4008].iter_mut().zip(&decry_sec_init.to_le_bytes()) {
        *dst = *src;
    }

    // Key: level 3, with modulo 2.
    let sec_area_key = dscrypto::key1::init(id_code, &root_key, 2, 3);

    // First 0x4000 bytes should not be decrypted.
    let mut output_buffer = rom_data[0..0x4008].to_vec();

    for encrypted_block in rom_data[0x4008..0x4800]
        .chunks(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
        .map(|block| dscrypto::key1::decrypt(block, &sec_area_key)) {
        
        for byte in encrypted_block.to_le_bytes() {
            output_buffer.push(byte);
        }
    }

    // Remaining data should not be decrypted.
    output_buffer.extend_from_slice(&rom_data[0x4800..]);

    File::create(&out_path).expect("couldn't open output file")
        .write_all(&output_buffer).expect("couldn't write to output file");

    if verbose {
        println!("Decrypted {} bytes to {}", output_buffer.len(), out_path);
    }
}

fn get_u32(slice: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(slice[at..(at + 4)].try_into().unwrap())
}