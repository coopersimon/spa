# SPA: GameBoy Advance emulator
A GameBoy Advance emulator written in rust. See "spa" for the crate, and "spa-bin" for the runnable example implementation.

## Status
Lots of games work! See spa/README.md for the full list of tested games.

### TODO
- Audio FIFO - less crackling
- Render thread.
- Frame capture DMA
- Internal "hardware" BIOS
- Video mosaic.
- Deal with rampant unaligned memory accesses.
