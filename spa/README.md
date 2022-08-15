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
- Age of Empires: Age of Kings: Loads main menu, freezes
- Animal Crossing Wild World: (FLASH): Loads up titles shows menu and gameplay. Intro rain + glass looks a bit wrong.
- Chrono Trigger: Sets up save RAM, then shows intro video. Game loads up and runs ok!
- Dragon Quest IX: Shows intro cards and video. Magenta screen when game begins.
- Dragon Quest IV: Black screen - getting stuck in halt loop
- Dragon Quest V: Black screen
- Dragon Quest VI: Black screen
- Final Fantasy Tactics A2: Loads up and game starts. Flickering (capture issues?). Some broken graphics.
- Final Fantasy III: Sets up save ram, shows titles, now locks up.
- Final Fantasy IV: Works well (with some 3d visual issues) up until after first battle in gameplay. Cutscene in castle has broken visuals ontop of 3D (2d issue?)
- GTA Chinatown Wars: Shows some broken screens on start, then crashes when gameplay starts.
- Hotel Dusk: Room 215: Shows titles and initial screen with some graphical issues (capture issues?). Needs touchscreen?
- Kirby Super Star: Initialises save data, then shows some screens with incorrect colour. Is it trying to blend stuff here?
- Mario Kart DS: (FLASH) Titles + menu work ok, 3D visuals are ok but heavy warping + aggressive clipping near camera (perspective correct textures would help here)
- Mario and Luigi: Bowser's Inside Story: Blank screen (firmware/save issues)
- Mario and Luigi: Partners in Time: Loads up mostly ok, menu title 3D is ok, intro has serious graphical issues
- Mario Party DS: 2D stuff looks ok. 3D visuals, crash when loading minigame (VRAM ext palette access failed)
- Metroid Prime Hunters: Initial titles are broken. Shows intro video. Gets to menu then locks up.
- New Super Mario Bros: Intro + menu is ok, 3D visuals (very broken - missing graphics + blending issues)
- LoZ Phantom Hourglass: Just white screen now.
- LoZ Spirit Tracks: White screen.
- Nintendogs (Labrador): Intro is mostly OK, top screen flickering (capture issues). Crashes trying to access WRAM (?)
- Pokemon Mystery Dungeon Explorers of Sky: Intro plays. It's super slow. Gameplay starts up ok, with some graphical glitches on lower screen.
- Pokemon Mystery Dungeon Blue Rescue Team: Intro plays, without sprites. Menu text is blocked out. Black screen when game begins.
- Pokemon Diamond: (FLASH) 2d elements of game work ok, 3D in intro looks good, 3D in title looks good
- Pokemon Black: Black screen
- Pokemon HeartGold: 2d elements of game work ok, 3D in intro good, 3D in title is quite broken
- Pokemon Ranger: Crashes due to trying to emit polygon without a primitive type (???)
- Super Mario 64 DS: Loads up ok, star in intro looks wrong (specular lighting/texture issues). 3D in menu looks good. Actual game has polygons flying everywhere in front of scene + perspective issues
- Tony Hawk's Downhill Jam: Shows titles, then black screen
- Tony Hawk's American Sk8land: Shows titles, menus, then gameplay crashes after matrix overflow
- The World Ends With You: Initial titles, then freezes (some sort of sprite visible at bottom of screen)
- Yoshi's Island DS: Shows titles, intro, menu, pre-gameplay video, and gameplay correctly!

#### NDS TODO
- Audio
- 3D Video:
    - Clip in 3D space?
    - Rasterising precision improvement
    - Lighting issues (?)
    - Post-processing (edge + anti-aliasing)
    - Perspective correct tex mapping needed
- 2D main RAM video mode
- Engine A video capture & blending fixes
- Touchscreen precision
- Microphone
- Save RAM
- Booting via BIOS
- Performance
