
mod debug;

use spa::GBA;
use clap::{clap_app, crate_version};

fn main() {
    let app = clap_app!(spa =>
        (version: crate_version!())
        (author: "Simon Cooper")
        (about: "Gameboy Advance emulator.")
        (@arg CART: "The path to the game cart to use.")
        (@arg debug: -d "Enter debug mode.")
        //(@arg save: -s +takes_value "Save file path.")
        (@arg biosrom: -r +takes_value "BIOS ROM path. Needed for certain games.")
    );

    let cmd_args = app.get_matches();

    let cart_path = match cmd_args.value_of("CART") {
        Some(c) => std::path::Path::new(c),
        None => panic!("Usage: spa [cart name]. Run with --help for more options."),
    };

    let bios_path = cmd_args.value_of("biosrom").map(|s| std::path::Path::new(s));

    let mut gba = GBA::new(&cart_path, bios_path);

    if cmd_args.is_present("debug") {
        //#[cfg(feature = "debug")]
        debug::debug_mode(&mut gba);
    } else {
        let mut last_frame_time = chrono::Utc::now();
        let frame_time = chrono::Duration::nanoseconds(1_000_000_000 / 60);

        loop {
            let now = chrono::Utc::now();
            if now.signed_duration_since(last_frame_time) >= frame_time {
                last_frame_time = now;
                gba.frame();
            }
        }
    }
}
