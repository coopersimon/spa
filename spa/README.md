# SPA core
The core of the emulator.

GBA:
- Runs generally pretty well.
- Save games supported.
- Link cable _NOT_ supported.
- Experimental JIT support.
- Experimental no-BIOS support.

DS:
- Very much in development...
- Fast boot (skips over BIOS boot procedure).

## Test list

### GBA Status of selected games:
- Metroid Fusion: Looks good. Some popping in audio.
- Metroid Zero Mission: Looks good.
- Crash Bandicoot XS: Looks good.
- Crash Bandicoot 2: Looks good. Audio sounds completely broken.
- Doom: Looks good.
- Doom II: Loads up ok. Gameplay looks a bit weird. Might be that frame isn't overwritten with blank data?
- Final Fantasy Dawn of Souls:
    - FFI: Setup is OK, intro seems OK. The overworld looks strange. Everything else seems ok
- Final Fantasy IV: Looks good.
- Final Fantasy V: Looks good.
- Final Fantasy VI: Looks good, title screen palette is wrong.
- Final Fantasy Tactics Advance: Looks good.
- Four Swords: Looks OK.
- Golden Sun: Looks mostly OK.
- Harry Potter 2: Looks good
- Incredibles: Looks good
- LEGO Star Wars: Looks good
- Mario Kart Super Circuit: Title, intro, selection looks good.
- Mother 1+2: Loads up ok. Cart select looks good.
    - Mother 1: Looks good.
    - Mother 2: Title, character naming, and intro works.
- Mother 3: Works ok.
- Pokemon Emerald: Loads up OK. RTC not implemented yet.
- Pokemon FireRed: Loads up OK. RTC not implemented yet.
- Pokemon Mystery Dungeon (red): Looks good.
- Super Mario Bros (NES): Black screen. Might be EEPROM trouble.
- Super Mario Bros 3 (Advance 4): Looks good
- Super Mario World: Looks ok. Colours seem very washed out (green swap?)
- The Minish Cap: Looks good.
- Yoshi's Island: Looks and sounds good.
- Advance Wars: Looks good.
- F-Zero Maximum Velocity: Looks good.
- Pokemon Ruby: Looks good.
- Fire Emblem: Looks good.
- Sonic Advance: Loads up and shows some scrolling backgrounds and doesn't respond. Audio plays OK (apparently a link cable issue)
- Wario Land 4: Looks OK.
- Mario and Luigi Superstar Saga: Mostly OK, GB sounds sometimes don't stop when they should.
- Scooby Doo: Black screen.

#### Known Bugs
- GB audio freq clock is incorrect.

#### Odd things
- DMA seems to use unaligned addresses in both metroids. It also seems to be very much intentional
    - What is the expected behaviour here?
    - Looks like accesses should just be force-aligned to word or halfword addresses.
- Loads of unaligned memory accesses.

### DS Status of selected games:
- Animal Crossing Wild World: Tries to access firmware with instr 6
- Chrono Trigger: Shows slightly broken screen claiming to load save RAM, and freezes
- Dragon Quest IX: Shows intro cards (jankily) and then kinda breaks when game begins. Intro video plays but corrupted
- Dragon Quest V: Black screen
- Final Fantasy Tactics A2: Just black screen
- Final Fantasy III: Black screen
- Final Fantasy IV: Crashes when trying to write to VRAM.
- Hotel Dusk: Room 215: Black screen
- Kirby Super Star: Loads up and shows menu background, in pink instead of blue?
- Mario Kart DS: Tries to access firmware with instr 6
- New Super Mario Bros: Shows a title then crashes with rendering
- Phantom Hourglass: Black/white screen.
- Pokemon Mystery Dungeon Explorers of Sky: Black screen
- Pokemon Mystery Dungeon Blue Rescue Team: Black screen (some detectable stuff in debug view)
- Pokemon Diamond: Tries to access firmware with instr 6
- Pokemon Ranger: Slow start but does boot ok and shows menu. Seems to require touchscreen afterwards.
- Super Mario 64 DS: Shows an initial nintendo logo and then freezes
- The World Ends With You: Black screen
- Yoshi's Island DS: Shows initial titles ok, shows a corrupted screen after, then menu. Menu looks very off.

#### NDS TODO
- Audio
- 3D Video
- 2D main RAM video mode
- Engine A video capture & blending
- Touchscreen
- Microphone
- Save RAM
- Booting via BIOS
- Performance
