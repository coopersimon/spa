# SPA core
The core of the emulator.

## Test list
Status of selected games:
- Metroid Fusion: Starts up, shows intro, shows menu. Background graphics are partially corrupted, colours wrong
- Metroid Zero Mission: As above
- Crash Bandicoot XS / 2: Need bitmap support
- Doom 1 / 2: Need bitmap support
- Final Fantasy Dawn of Souls: Tries to read video address below VRAM
- Final Fantasy IV: Tries to branch to unaligned address (very alarming... CPU bug)
- Final Fantasy V: White screen, no response
- Final Fantasy VI: Tries to read unaligned ROM value. (Used to show language select menu?)
- Final Fantasy Tactics Advance: Shows language select ok, fades in intro and then crashes due to selecting mode 6 or 7 for video
- Four Swords: Tries to read unaligned ROM value (in LDM)
- Golden Sun: Corrupted intro, shows character name input, then black screen & no response.
- Harry Potter 2: Tries to read unaligned ROM value.
- Incredibles: Shows initial screen (wrong palette) then tries to write to invalid OAM address
- LEGO Star Wars: Need bitmap support
- Mario Kart Super Circuit: Tries to write unaligned halfword to ROM.
- Mother 1+2: Loads up ok. Cart select has some graphical corruption but otherwise looks mostly ok.
    - Mother 1: Title, character naming, and intro works. Palette is wrong. Then when gameplay begins it tries to read an unaligned halfword
    - Mother 2: Title, character naming, and intro works. When gameplay begins it tries a DMA word transfer from a halfword address
- Pokemon Emerald: Tries to read unaligned word from RAM
- Pokemon FireRed: As above.
- Pokemon Mystery Dungeon (red): Tries to write beyond the end of VRAM.
- Super Mario Bros (NES): EEPROM issues.
- Super Mario Bros 3 (Advance 4): Shows some sort of corrupted title, no response.
- Super Mario World: Shows intro (palette wrong), shows title, game select, then it tries to execute an ARM instruction at 0x36
- The Minish Cap: White screen, no response
- Yoshi's Island: Tries to load unaligned ROM word.

## Odd things
- DMA seems to use unaligned addresses in both metroids. It also seems to be very much intentional
    - What is the expected behaviour here?