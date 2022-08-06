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
- Final Fantasy VI: Looks good.
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
- Animal Crossing Wild World: (FLASH): Loads up titles, locks up
- Chrono Trigger: Sets up save RAM, then shows intro video. Game loads up and runs ok!
- Dragon Quest IX: Shows intro cards and video. Magenta screen when game begins.
- Dragon Quest IV: Black screen - getting stuck in halt loop
- Dragon Quest V: Black screen
- Dragon Quest VI: Black screen
- Final Fantasy Tactics A2: Shows a static screen after loading for a bit. (Now crashes on 3d test)
- Final Fantasy III: Sets up save ram, shows titles, plays intro video, main menu, then tries to access LCDC image and crashes.
- Final Fantasy IV: Sets up save ram, shows titles, plays intro video, main menu, then black screen when game begins.
- Hotel Dusk: Room 215: Shows right/left handed text while flickering.
- Kirby Super Star: Initialises save data, then shows some screens with incorrect colour. Is it trying to blend stuff here?
- Mario Kart DS: (FLASH) Mostly works except no 3D support yet. Actual game runs choppy + seems a little off..?
- Mario and Luigi: Bowser's Inside Story: Blank screen (firmware/save issues)
- Mario and Luigi: Partners in Time: Loads up mostly ok with some glitches, menu title is glitched (3D), intro has serious graphical issues (now crashes on 3d test)
- Mario Party DS: Seems to work OK except no 3D support.
- Metroid Prime Hunters: Initial titles are broken. Shows intro video. No 3D support.
- New Super Mario Bros: Seems to work OK except no 3D support.
- Phantom Hourglass: Shows titles then freezes.
- Pokemon Mystery Dungeon Explorers of Sky: Intro plays. It's super slow. Gameplay starts up ok, with some graphical glitches on lower screen.
- Pokemon Mystery Dungeon Blue Rescue Team: Intro plays, without sprites. Menu text is blocked out. Black screen when game begins.
- Pokemon Diamond: (FLASH) seems to run ok (No 3D support.)
- Pokemon Ranger: Slow start but does boot ok and shows menu. Seems to require touchscreen afterwards.
- Super Mario 64 DS: Shows an initial nintendo logo, takes a long time to load before showing pre-menu screen, shows some more stuff (requires touchscreen)
- The World Ends With You: Initial titles, then freezes (some sort of sprite visible at bottom of screen)
- Yoshi's Island DS: Shows titles, intro, menu, pre-gameplay video, and gameplay correctly!

#### NDS TODO
- Audio
- 3D Video
- 2D main RAM video mode
- Engine A video capture & blending fixes
- Touchscreen precision
- Microphone
- Save RAM
- Booting via BIOS
- Performance
