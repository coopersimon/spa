# SPA: GameBoy Advance emulator
A GameBoy Advance emulator written in rust. See "spa" for the crate, and "spa-bin" for the runnable example implementation.

## Status
BIOS seems to startup OK, with varying success when actually running games.

See spa/README.md for the full list of tested games.

### TODO
- Cleanup audio
    - Audio should be running a bit faster than it is
- Render thread
- Frame capture DMA
- Internal "hardware" BIOS
- Lots of rendering fixes
- Deal with rampant unaligned memory accesses
- Separate render thread.
