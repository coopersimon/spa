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
Note that most of these games have not been properly played for a longer duration. Currently the challenge for most games is to ensure they firstly boot, but also can navigate through intro menus and into gameplay.

- Ace Attorney: Seems ok.
- Advance Wars: Dual Strike: Some visibility issues in menu, flickering in-game.
- Age of Empires: Age of Kings: Works OK.
- Animal Crossing Wild World: Works mostly OK.
- Brain Training (Brain Age): Seems to run OK.
- Black Ops: Mostly OK.
- Civilization Revolution: Mostly OK.
- Chrono Trigger: Audio skips a little in intro, but mostly ok otherwise.
- Dragon Quest Monsters Joker: Seems OK.
- Dragon Quest IX: Shows intro cards and video. Game starts mostly OK (3d model never updates: issue with writing to multiple VRAM blocks at once?), some missing textures, text on 3d screen not visible
- Dragon Quest IV: Mostly OK
- Dragon Quest V: Mostly OK
- Dragon Quest VI: Intro has some glitches, gameplay is mostly OK (some missing geometry)
- Etrian Odyssey: Issues with text.
- Final Fantasy Tactics A2: Mostly OK.
- Final Fantasy III: Mostly OK.
- Final Fantasy IV: Mostly OK.
- Final Fantasy: 4 Heroes of Light: Mostly ok
- Final Fantasy XII: Revenant Wings: Mostly ok, but general frame pacing is completely broken in both videos and (seemingly) gameplay
- Fire Emblem: Shadow Dragon: Intro missing some graphics (less than before). Otherwise looks and sounds fine.
- Front Mission: Mostly ok
- Ghost Trick: Mostly ok
- Golden Sun: Dark Dawn: Kind of OK, some serious flickering and juddering
- GTA Chinatown Wars: Mostly ok
- Hotel Dusk: Room 215: Now crashes with an undefined instruction in ITCM.
- Inazuma Eleven: Mostly ok, text looks borked
- Kirby Super Star: Mostly ok, some audio stream issues when playing video.
- LoZ Phantom Hourglass: Mostly ok, some general 3D issues, outlines going a bit wild on the fairy.
- LoZ Spirit Tracks: Loads up OK.
- LEGO Lord of the Rings: Boots, early images look blurry/mismapped, gameplay doesn't load
- LEGO Star Wars II: Mostly OK. Crashes due to reading beyond end of specular table in lighting calculation.
- Mario Kart DS: Mostly ok.
- Mario and Luigi: Bowser's Inside Story: Blank screen
- Mario and Luigi: Partners in Time: Mostly OK.
- Mario Party DS: Mostly OK.
- Mario Slam Basketball (Hoops 3-on-3): Mostly OK.
- Metroid Prime Hunters: Mostly OK, some depth issues close to camera.
- Metroid Prime Pinball: Right half of the screen is not visible, game crashes in gameplay (with undefined instruction in BIOS??)
- New Super Mario Bros: Mostly OK
- Nintendogs (Labrador): Mostly OK, when dogs load the GPU command buffer overflows
- Okamiden: Mostly OK
- Pokemon Mystery Dungeon Explorers of Sky: Mostly OK.
- Pokemon Mystery Dungeon Blue Rescue Team: Mostly OK, flickering at bottom of text boxes.
- Pokemon Platinum: Mostly OK. Gameplay is fine.
- Pokemon Black: Only boots via firmware. Mostly ok, some large tex mapping issues.
- Pokemon Black 2: Boots, then menu is unresponsive with black screen, although audio plays.
- Pokemon HeartGold: Mostly OK, some gaps in geometry, outline issues
- Pokemon Ranger: Mostly ok.
- Resident Evil: Seems mostly ok. Frame pacing issues. Freezes during intro cutscene, though if skipped gameplay seems ok.
- Sim City DS: Mostly ok.
- Sims 3: Video is slow, gameplay graphics are quite glitched.
- Shin Megami Tensei: Strange Journey: Titles and menu OK, freezes when gameplay begins.
- Super Mario 64 DS: Mostly OK, some lighting issues on Yoshi.
- Super Princess Peach: Mostly OK.
- Super Scribblenauts: Mostly fine.
- Tony Hawk's Downhill Jam: Shows titles, then black screen
- Tony Hawk's American Sk8land: Mostly OK: lighting flickers a lot on skater model.
- Wario: Master of Disguise: Crashes early with undefined instr.
- WarioWare: Touched: Seems OK, brief moment where right side of screen is hidden.
- The World Ends With You: Seems mostly ok.
- Yoshi's Island DS: Mostly OK.

#### NDS TODO

##### Hardware features:
- Microphone
- WiFi
- Local network

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
    - Lighting issues (?)
    - Post-processing (edge + fog + anti-aliasing)
    - Minor texture clip issues
        - Pokemon Ranger 2D stuff
    - Depth issues
- 2D main RAM video mode
    - Still haven't found a game that actually uses this
- Engine A video capture & blending fixes
- Audio stream bugs
- Firmware issues
    - Boot all games
- Performance
    - Memory
    - Threading
    - CPU rendering
    - JIT + GPU rendering
