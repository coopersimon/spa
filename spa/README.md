# SPA core
The core of the emulator.

## Test list
Status of selected games:
- Metroid Fusion: Seems to run ok
- Metroid Zero Mission: As above - in gameplay, one background doesn't go away.
- Crash Bandicoot XS: Intro has some scrolling issues. Gameplay background graphics look completely wrong.
- Crash Bandicoot 2: Intro looks good, gameplay background graphics look completely wrong.
- Doom: Intros load up OK. Frame rate is terrible. Actual gameplay is flickering all over the place.
- Doom II: Tries to write word to 0x1C06 (ROM ???) almost immediately
- Final Fantasy Dawn of Souls: Intro, title, naming and game look mostly ok. Frame rate is bad. Weird clipping of sprites near bottom of screen.
- Final Fantasy IV: Game loads up ok, title and intro missing affine bgs (temporarily disabled)
- Final Fantasy V: Game loads up ok, title looks a bit corrupted. Intro is ok but veeeerry bad frame rate and sprite clipping visible.
- Final Fantasy VI: Shows square enix logo then fades to black and never returns (could be affine bg or frame rate issues)
- Final Fantasy Tactics Advance: Start of intro OK, rest of intro doesn't work. Title has a lot of corruption.
- Four Swords: Tries to read unaligned ROM value (in LDM)
- Golden Sun: Corrupted intro, shows character name input, shows a tiny amount of intro then tries to read unaligned ROM halfword.
- Harry Potter 2: Corrupted title graphics. Intro also has corrupted backgrounds.
- Incredibles: Loads up and gameplay + sprites seem to be ok. Background graphics are _completely_ broken.
- LEGO Star Wars: Loads up ok. Some background graphics look corrupted.
- Mario Kart Super Circuit: Tries to write unaligned halfword to ROM.
- Mother 1+2: Loads up ok. Cart select looks good.
    - Mother 1: Title, character naming, and intro works. Then when gameplay begins it tries to read an unaligned halfword
    - Mother 2: Title, character naming, and intro works. When gameplay begins it tries a DMA word transfer from a halfword address
- Mother 3: Tries to read unaligned word
- Pokemon Emerald: Tries to read unaligned word from RAM
- Pokemon FireRed: As above.
- Pokemon Mystery Dungeon (red): Seems pretty good.
- Super Mario Bros (NES): Black screen. Might be EEPROM trouble.
- Super Mario Bros 3 (Advance 4): Title graphics are corrupted. Actual game seems to begin ok (level select), but when level begins it tries to write an unaligned halfword
- Super Mario World: Intro and startup looks mostly OK, when gameplay begins it tries to write unaligned VRAM halfword.
- The Minish Cap: Loads up ok, graphics are a bit corrupted.
- Yoshi's Island: Tries to load unaligned ROM word.

## Known Bugs
- It seems 8bpp tiled background tiles are wrong. 8bpp sprites look ok so I think this is to do with picking them.
- Sprites beyond y +128 are being wrapped incorrectly by the looks.
- Affine backgrounds are disabled for now, rendering process is incorrect.
- Performance is terrible across the board - a rendering thread would help, as would better rendering algos, JIT would help a lot more.
- Loads of unaligned memory accesses. Some of these may be bugs, however it's seeming like at least some are intentional.

## Odd things
- DMA seems to use unaligned addresses in both metroids. It also seems to be very much intentional
    - What is the expected behaviour here?