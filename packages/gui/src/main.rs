use clap::Parser;
use env_logger;
use std::fs;

use gameboy_core::Gameboy;

#[derive(Parser, Debug)]
#[command(
    author = "Justin Carruthers",
    about = "Configurable Gameboy DMG emulator"
)]
struct Args {
    rom_name: String,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .format_target(false)
        .format_timestamp(None)
        .init();

    let args = Args::parse();
    let rom = fs::read(&args.rom_name).expect("Cant read the rom");
    let mut gameboy = Gameboy::new(rom);

    gameboy.run()
}
