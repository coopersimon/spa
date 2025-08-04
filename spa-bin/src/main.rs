
mod debug;
mod run;

use clap::{clap_app, crate_version};

use spa::{gba, ds};

use std::path::PathBuf;

fn main() {
    //env_logger::init();

    let app = clap_app!(spa =>
        (version: crate_version!())
        (author: "Simon Cooper")
        (about: "Gameboy Advance and DS emulator.")
        (@arg ROM: "The path to the game ROM to use.")
        (@arg debug: -d +takes_value "Enter debug mode.")
        (@arg mute: -m "Mute all audio.")
        (@arg save: -s +takes_value "Save file path.")
        (@arg biosrom: -r +takes_value "BIOS ROM path. Needed for certain games.")
        (@arg dsbios: -b +takes_value "BIOS folder for NDS. Inside should be [bios7.bin, bios9.bin, firmware.bin]. Needed for certain games.")
    );

    let cmd_args = app.get_matches();

    let rom_path = match cmd_args.value_of("ROM") {
        Some(c) => PathBuf::from(c),
        None => panic!("Usage: spa [ROM name]. Run with --help for more options."),
    };

    let save_path = cmd_args.value_of("save").map(|s| PathBuf::from(s));
    let bios_path = cmd_args.value_of("biosrom").map(|s| PathBuf::from(s));
    let ds_bios_path = cmd_args.value_of("dsbios").map(|s| PathBuf::from(s));

    if let Some(value) = cmd_args.value_of("debug") {
        if value == "gba" {
            let debug_interface = gba::GBA::new_debug(gba::MemoryConfig{
                rom_path, save_path, bios_path
            });
            debug::debug_mode(debug_interface);
        } else {
            let ds7_bios_path = ds_bios_path.clone().map(|mut p| {
                p.push("bios7.bin");
                p
            });
            let ds9_bios_path = ds_bios_path.clone().map(|mut p| {
                p.push("bios9.bin");
                p
            });
            let firmware_path = ds_bios_path.clone().map(|mut p| {
                p.push("firmware.bin");
                p
            });
            let config = ds::MemoryConfig{
                rom_path, save_path, ds7_bios_path, ds9_bios_path, firmware_path, fast_boot: true
            };
            if value == "ds7" {
                let debug_interface = ds::NDS::new_debug_7(config);
                debug::debug_mode(debug_interface);
            } else if value == "ds9" {
                let debug_interface = ds::NDS::new_debug_9(config);
                debug::debug_mode(debug_interface);
            } else {
                println!("unknown debug mode {}. use gba or ds[7|9]", value);
            }
        }
        return;
    }

    if let Some(ext) = rom_path.extension() {
        match ext.to_str().unwrap() {
            "gba" => run::run_gba(gba::MemoryConfig{
                rom_path, save_path, bios_path
            }, cmd_args.is_present("mute")),
            "nds" => {
                let ds7_bios_path = ds_bios_path.clone().map(|mut p| {
                    p.push("bios7.bin");
                    p
                });
                let ds9_bios_path = ds_bios_path.clone().map(|mut p| {
                    p.push("bios9.bin");
                    p
                });
                let firmware_path = ds_bios_path.clone().map(|mut p| {
                    p.push("firmware.bin");
                    p
                });
                let config = ds::MemoryConfig{
                    rom_path, save_path, ds7_bios_path, ds9_bios_path, firmware_path, fast_boot: true
                };
                run::run_nds(config, cmd_args.is_present("mute"))
            },
            other => panic!("unknown ext '{}'", other)
        }
    }
}
