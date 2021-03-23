# SPA core
The core of the emulator.

## Test list
Status of selected games:
- Metroid Fusion: Looks good. Some popping in audio.
- Metroid Zero Mission: Looks good.
- Crash Bandicoot XS: Looks good.
- Crash Bandicoot 2: Looks good. Audio sounds completely broken.
- Doom: Looks good.
- Doom II: Loads up ok. Gameplay looks a bit weird. Might be that frame isn't overwritten with blank data?
- Final Fantasy Dawn of Souls:
    - FFI: Setup is OK, intro goes OK for a bit then tries to write to VRAM addr 0x0607_4000
- Final Fantasy IV: Intro and title look good, when game begins scrolling background looks odd (individual tiles aren't scrolling)
- Final Fantasy V: Looks good.
- Final Fantasy VI: Looks good, title screen palette is wrong.
- Final Fantasy Tactics Advance: Looks good.
- Four Swords: Looks mostly OK, LTTP tries to read invalid VRAM addr 0x0750_000A during intro.
- Golden Sun: Looks mostly OK, sprites flicker constantly however.
- Harry Potter 2: Looks good
- Incredibles: Looks good
- LEGO Star Wars: Looks good
- Mario Kart Super Circuit: Title, intro, selection looks good. Affine bg in gameplay looks wrong.
- Mother 1+2: Loads up ok. Cart select looks good.
    - Mother 1: Looks good.
    - Mother 2: Title, character naming, and intro works. Demo crashes trying to read invalid VRAM addr 0x05DC_E0B4
- Mother 3: Works ok.
- Pokemon Emerald: Loads up OK. RTC not implemented yet.
- Pokemon FireRed: Loads up OK. RTC not implemented yet.
- Pokemon Mystery Dungeon (red): Looks good.
- Super Mario Bros (NES): Black screen. Might be EEPROM trouble.
- Super Mario Bros 3 (Advance 4): Looks good
- Super Mario World: Looks ok. Colours seem very washed out (green swap?)
- The Minish Cap: Looks good.
- Yoshi's Island: Intro looks good. Title screen affine background looks wrong. Works OK.
- Advance Wars: Looks good.
- F-Zero Maximum Velocity: Title and setup works ok, actual game backgrounds perspective is off.
- Pokemon Ruby: Looks good.
- Fire Emblem: Looks good.
- Sonic Advance: Loads up and shows some scrolling backgrounds and doesn't respond. Audio plays OK

## Known Bugs
- Affine backgrounds still have issues.
- Square wave 1 frequency sweep seems to be incorrect.

## Odd things
- DMA seems to use unaligned addresses in both metroids. It also seems to be very much intentional
    - What is the expected behaviour here?
    - Looks like accesses should just be force-aligned to word or halfword addresses.
- Loads of unaligned memory accesses.