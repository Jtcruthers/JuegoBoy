use clap::Parser;
use env_logger;
use std::{fs, time::Instant};

use softbuffer::Context;
use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod user_input;

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

    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

    let context = Context::new(event_loop.owned_display_handle()).unwrap();

    let mut app = app::App::new(rom, context);
    match event_loop.run_app(&mut app) {
        Ok(ok) => {
            log::info!("Application exited successfully: {:?}", ok);
        }
        Err(err) => {
            log::error!("Application exited with error: {:?}", err);
        }
    }
}
