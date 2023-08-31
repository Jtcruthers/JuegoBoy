use std::fs;
use clap::Parser;

mod apu;
mod cpu;
mod gameboy;
mod mmu;
mod ppu;
mod registers;

#[derive(Parser, Debug)]
#[command(author = "Justin Carruthers", about = "Configurable CHIP-8 (and variants) emulator")]
struct Args {
    rom_name: String,
}

fn main() {
    let args = Args::parse();
    let rom = fs::read(&args.rom_name).expect("Cant read the rom");
    let mut gameboy = gameboy::Gameboy::new();
    gameboy.run(rom);
}
