/*!
* This package is for the shared frontend logic that doesn't really fit into the actual gameboy
* emulator. The more I researched handling the audio, the more it seemed I'd have to duplicate it
* for web and native. So I wanted to move it to a centralized location, but wanted to keep core as
* strictly the gameboy only. I then realized I had to port over the catch-up frame rate from web
* to native, so I could just centralize that as well.
*
* So this package contains audio handling, frame rate, and other generic logic for actually
* rendering the gameboy. Then the other packages will be just the glue for either native or wasm.
*/

mod audio;
mod player;

pub use player::Player;
