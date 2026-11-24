# JuegoBoy

Toy Gameboy emulator written in Rust.
I wanted to try writing my own emulator as the chip-8 project was fun and wanted to see how these
low-level emulators worked on a bigger scale.o

I'm working on getting it M-cycle accurate on timing, but it does use scanline-based rendering instead.

## Running

It uses the copywright-free bootix_dmg Bootrom, so none is required.

### Native

To build it,

`cargo build --release -p gui`

To run:

`./target/release/gui <path/to/rom>`

### Web

To build,

`wasm-pack build packages/web --target web --release --out-dir www/pkg --no-typescript`

To run:

You can serve the generated web package wherever, or run a test server via

`python3 -m http.server 8080 --directory packages/web/www`

The web site will have to provide it a file, but it will handle the timing and input/output separately as long as a canvas is provided

## General

I wanted to emulate the actual hardware, and originally wanted to implement each component fully independent of one another.
The idea would then assemble them in `gameboy.rs`, and it would just work.
This proved a little harder than I thought, due to Rust's ownership rules and circular references.

So I ended up having them be as separate as reasonably possible,
but the CPU owns the MMU, which owns the other components.
But it also meant that things like vram actually moved into the PPU so it could block reads correctly,
even though that's not actually how the hardware works.

As this was really just to work on emulating the hardware,
I tried to keep abstractions and structs as minimal as possible.
This is why I don't use structs for things like the LCD or tiles/tilemaps.
The project was more focused on emulating the bits and bit manipulations as the hardware intended.

T-cycle accuracy seemed like a big leap forward, so I settled on M-cycle accuracy.

The actual gameboy emulation is in the `core` package, so it can be wired up to different UIs independently.

## Gaps

As stated, it is not T-cycle accurate.
That may happen in the future, but a fully functioning M-cycle accurate emulator is first.

I am also not emulating the Pixel FIFO, and instead using a scanline-based render cycle.
Even with the scanline-based implementation, the gameboy's step method is based on frames.
This means, even though I am drawing the scanline to the LCD,
the emulator will step until the next frame is fully rendered before giving control back to the caller.

## Resources Used

* [Pandocs](https://gbdev.io/pandocs/Specifications.html)
* [Optable](https://gbdev.io/gb-opcodes/optables/)
* [Opcode Reference](https://rgbds.gbdev.io/docs/v1.0.4/gbz80.7)
* [DAA Operation Breakdown](https://blog.ollien.com/posts/gb-daa/)
* [Joypad Input](http://www.codeslinger.co.uk/pages/projects/gameboy/joypad.html)
* [Window and PPU](https://jsgroth.dev/blog/posts/gb-rewrite-pixel-fifo)
* [Audio/APU - jsgroth](https://jsgroth.dev/blog/posts/gb-rewrite-apu/)
* [APU GBDev](https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware)
