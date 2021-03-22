# SPA core
The core of the emulator.

## Test list
Status of selected games:
- Metroid Fusion: Looks good.
- Metroid Zero Mission: Looks good.
- Crash Bandicoot XS: Looks good.
- Crash Bandicoot 2: Looks good. Audio sounds completely broken.
- Doom: Looks good.
- Doom II: Loads up ok. Gameplay looks a bit weird. Might be that frame isn't overwritten with blank data?
- Final Fantasy Dawn of Souls: Looks good.
- Final Fantasy IV: Intro and title look good, when game begins scrolling background looks odd (individual tiles aren't scrolling)
- Final Fantasy V: Looks good.
- Final Fantasy VI: Looks good, title screen palette is wrong.
- Final Fantasy Tactics Advance: Looks good.
- Four Swords: Looks good.
- Golden Sun: Looks good.
- Harry Potter 2: Looks good
- Incredibles: Looks good
- LEGO Star Wars: Looks good
- Mario Kart Super Circuit: Title, intro, selection looks good. Affine bg in gameplay looks wrong.
- Mother 1+2: Loads up ok. Cart select looks good.
    - Mother 1: Looks good.
    - Mother 2: Title, character naming, and intro works. Demo crashes trying to access incorrect VRAM address.
- Mother 3: Works ok. Some weird scrolling issues in intro.
- Pokemon Emerald: Loads up OK. RTC not implemented yet.
- Pokemon FireRed: Loads up OK. RTC not implemented yet.
- Pokemon Mystery Dungeon (red): Looks good.
- Super Mario Bros (NES): Black screen. Might be EEPROM trouble.
- Super Mario Bros 3 (Advance 4): Looks good
- Super Mario World: Looks ok. Colours seem very washed out (green swap?)
- The Minish Cap: Looks good.
- Yoshi's Island: Intro looks good. Title screen affine background looks wrong. Title also has some strange colour blending effect. When gameplay begins, frozen white screen.
    - Actual code goes beyond the end of cart memory for some reason here.
- Advance Wars: Looks good.
- F-Zero Maximum Velocity: Title and setup works ok, actual game backgrounds perspective is off.
- Pokemon Ruby: Looks good.
- Fire Emblem: Looks good.
- Sonic Advance: Loads up and shows some scrolling backgrounds and doesn't respond. Audio plays OK

## Known Bugs
- Mosaic still not implemented.
- Affine backgrounds still have issues.
- There are some issues with unaligned memory.
- Square wave 1 frequency sweep seems to be incorrect.

## Odd things
- DMA seems to use unaligned addresses in both metroids. It also seems to be very much intentional
    - What is the expected behaviour here?
- Loads of unaligned memory accesses.