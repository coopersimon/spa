# SPA core
The core of the emulator.

## Test list
Status of selected games:
- Metroid Fusion: Seems to run ok
- Metroid Zero Mission: As above - in gameplay, one background doesn't go away.
- Crash Bandicoot XS: Looks good.
- Crash Bandicoot 2: Looks good.
- Doom: Intros load up OK.
- Doom II: Loads up ok. Gameplay looks a bit weird. Might be that frame isn't overwritten with blank data?
- Final Fantasy Dawn of Souls: Intro, title, naming and game look mostly ok.
- Final Fantasy IV: Intro and title look good, when game begins scrolling background looks odd (individual tiles aren't scrolling)
- Final Fantasy V: Looks good.
- Final Fantasy VI: Shows square enix logo then fades to black and stops responding
- Final Fantasy Tactics Advance: Looks good.
- Four Swords: Works ok. still probably tries an unaligned load.
- Golden Sun: Looks good.
- Harry Potter 2: Looks good
- Incredibles: Looks good
- LEGO Star Wars: Looks good
- Mario Kart Super Circuit: Title, intro, selection looks good. Affine bg in gameplay looks wrong.
- Mother 1+2: Loads up ok. Cart select looks good.
    - Mother 1: Looks good.
    - Mother 2: Title, character naming, and intro works. Demo crashes trying to access incorrect VRAM address.
- Mother 3: Works ok. Some weird scrolling issues in intro.
- Pokemon Emerald: Loads up OK. "Internal clock battery has run dry".
- Pokemon FireRed: Loads up OK. Mostly looks good.
- Pokemon Mystery Dungeon (red): Seems pretty good.
- Super Mario Bros (NES): Black screen. Might be EEPROM trouble.
- Super Mario Bros 3 (Advance 4): Looks good
- Super Mario World: Intro and startup looks good. Colours seem very washed out (green swap?)
- The Minish Cap: Looks good.
- Yoshi's Island: Intro looks good. Title screen affine background looks wrong. Title also has some strange colour blending effect. When gameplay begins, frozen white screen.
- Advance Wars: Looks good.
- F-Zero Maximum Velocity: Title and setup works ok, actual game backgrounds perspective is off.
- Pokemon Ruby: Intro has some issues. Mostly ok (similar to Emerald)
- Fire Emblem: mostly looks ok.
- Sonic Advance: Loads up and shows some scrolling backgrounds and doesn't respond.

## Known Bugs
- Lots of the above games simply say "looks good" when actually there are still lots of graphical glitches. This is simply because certain things have not been implemented yet.
    - Notably, windows and blending have some issues still. Mosaic is still unimplemented.
- Affine backgrounds still have issues.
- There are some issues with unaligned memory.

## Odd things
- DMA seems to use unaligned addresses in both metroids. It also seems to be very much intentional
    - What is the expected behaviour here?
- Loads of unaligned memory accesses.