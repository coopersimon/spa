# spa-bin
An example implementation of the spa core.

To run: `spa-bin [CART_PATH] -s [SAVE_FILE_PATH] -r [GBA_BIOS_PATH]`

The save file is optional. The ROM file can also be omitted but some games may not run.

To run a DS game: `spa-bin [CARD_PATH] -s [SAVE_FILE_PATH] -b [BIOS_FOLDER]`

Buttons:
- X: A
- Z: B
- D: X
- C: Y
- A: L
- S: R
- Space: Select
- Enter: Start
- Arrow Keys: D-Pad
- Click lower screen: Touchscreen

## Debug
Run with `-d` to enter debug mode. Enter `h` for help.
