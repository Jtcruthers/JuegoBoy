use std::fs;
use clap::Parser;
use env_logger;

mod apu;
mod cpu;
mod timer;
mod gameboy;
mod mmu;
mod ppu;
mod registers;
mod cartridge;
mod mbc1;
mod joypad;

#[derive(Parser, Debug)]
#[command(author = "Justin Carruthers", about = "Configurable Gameboy DMG emulator")]
struct Args {
    rom_name: String,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_target(false)
        .format_timestamp(None)
        .init();

    let args = Args::parse();
    let rom = fs::read(&args.rom_name).expect("Cant read the rom");
    let mut gameboy = gameboy::Gameboy::new(rom);

    gameboy.run()
}
