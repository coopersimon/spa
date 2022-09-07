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
- Ace Attorney: Crashes due to unimplemented cache command.
- Age of Empires: Age of Kings: Works OK.
- Animal Crossing Wild World: Works mostly OK.
- Brain Training (Brain Age): Seems to run OK.
- Black Ops: Boots mostly OK, unable to find strings to display for some reason.
- Civilization Revolution: Mostly OK, some issues with "3D" graphics in-game.
- Chrono Trigger: Audio skips a little in intro, but mostly ok otherwise.
- Dragon Quest IX: Shows intro cards and video. Shows broken screen and freezes. (Timing issues?)
- Dragon Quest IV: Mostly OK, shows a strange counter in intro
- Dragon Quest V: Mostly OK
- Dragon Quest VI: Slow start, broken intro, mostly OK
- Final Fantasy Tactics A2: Lots of broken graphics and visibility / depth issues. However functionally seems ok now.
- Final Fantasy III: Mostly OK, tries to use mode 6 on engine A and crashes.
- Final Fantasy IV: Mostly OK.
- Final Fantasy: 4 Heroes of Light: Game starts OK, 2D has visibility issues in menu. 3D is pretty good but has some precision issues.
- Fire Emblem: Shadow Dragon: Intro missing some graphics. Also in-game intro. Otherwise looks and sounds fine.
- GTA Chinatown Wars: Titles & menu looks OK, intro 3D is broken
- Hotel Dusk: Room 215: Now crashes with an undefined instruction in ITCM.
- Inazuma Eleven: Seems to work OK.
- Kirby Super Star: Mostly ok, some audio stream issues when playing video.
- Mario Kart DS: Titles + menu work ok, 3D visuals are ok but heavy warping + aggressive clipping near camera (perspective correct textures would help here)
- Mario and Luigi: Bowser's Inside Story: Blank screen
- Mario and Luigi: Partners in Time: Loads up mostly ok, menu title 3D is ok, intro has serious graphical issues
- Mario Party DS: Mostly OK. Some precision issues with 3D.
- Metroid Prime Hunters: Lots of broken graphics in titles and menu, also seems to lock up occasionally.
- New Super Mario Bros: Mostly OK. precision issues with 3D graphics
- LoZ Phantom Hourglass: Just white screen now.
- LoZ Spirit Tracks: White screen.
- LEGO Lord of the Rings: Loads up OK, some visibility issues in menus (2D issues?), now gameplay doesn't load (also crash due to 3D capture overreading)
- LEGO Star Wars II: Mostly OK, some near-plane clipping issues throughout, some graphics have missing pixels on 3D screen. Crashes due to reading beyond end of specular table
- Mario Slam Basketball (Hoops 3-on-3): Mostly OK.
- Nintendogs (Labrador): Intro is mostly OK with clipping errors. Gameplay begins, crashes due to full GPU command fifo. (Now crashes due to unimplemented cache command)
- Pokemon Mystery Dungeon Explorers of Sky: Mostly OK.
- Pokemon Mystery Dungeon Blue Rescue Team: Intro plays, without sprites. Menu text is blocked out. Black screen when game begins.
- Pokemon Diamond: 2d elements of game work ok, 3D in intro looks good, 3D in title looks good
- Pokemon Black: Black screen
- Pokemon HeartGold: 2d elements of game work ok, 3D in intro good, 3D in title is quite broken
- Pokemon Ranger: 2D components are rendered as 3D and there are some precision issues. Audio also has some volume issues.
- Shin Megami Tensei: Strange Journey: Titles OK, menu has visibility issues, freezes when gameplay begins.
- Super Mario 64 DS: Loads up ok, star in intro looks wrong (specular lighting/texture issues). 3D in menu looks good. Actual game has polygons flying everywhere in front of scene + perspective issues
- Super Princess Peach: Mostly OK.
- Super Scribblenauts: Kind of fine, lots of layering issues with 2D in the 3D engine.
- Tony Hawk's Downhill Jam: Shows titles, then black screen
- Tony Hawk's American Sk8land: Shows titles, menus, then gameplay crashes after matrix overflow
- The World Ends With You: Initial titles, then freezes (some sort of sprite visible at bottom of screen)
- Yoshi's Island DS: Mostly OK.

#### NDS TODO

##### Hardware features:
- Microphone
- WiFi
- Local network
- Booting via BIOS

##### Emulator features:
- Save states
- Play without BIOS/Firmware
- Better presentation options (sideways, screen gap)
- Config
    - Time
    - Rendering options (frame skip, filters, etc.)
    - Save type

##### Bugs / Improvements:
- 3D Video:
    - Clip in 3D space?
    - Rasterising precision improvement
    - Lighting issues (?)
    - Post-processing (edge + anti-aliasing)
    - Perspective correct tex mapping needed
    - More precise tex mapping needed
- 2D main RAM video mode
- Engine A video capture & blending fixes
- Card loading still has issues sometimes
- Audio stream bugs
- Performance
    - Memory
    - Threading
    - CPU rendering
    - JIT + GPU rendering
