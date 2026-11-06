use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use pixels::{Error, Pixels, SurfaceTexture};

use gameboy_core::Gameboy;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

pub struct App {
    window: Option<Arc<Window>>,
    gameboy: Gameboy,
    pixels: Option<Pixels<'static>>,
}

impl App {
    pub fn new(rom: Vec<u8>) -> Self {
        let gameboy = Gameboy::new(rom);
        Self {
            window: None,
            gameboy: gameboy,
            pixels: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("JuegoBoy"))
                .unwrap(),
        );
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(WIDTH, HEIGHT, surface).unwrap();
        self.pixels = Some(pixels);
        self.window = Some(window);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(10) {
            for _ in 0..100000 {
                self.gameboy.step();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { .. } => {
                // Where joystick stuff goes
            }
            WindowEvent::RedrawRequested => {
                if let Some(pixels) = self.pixels.as_mut() {
                    for px in pixels.frame_mut().chunks_exact_mut(4) {
                        px.copy_from_slice(&[0x9b, 0xbc, 0x0f, 0xff]);
                    }
                    pixels.render().unwrap();
                }
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}
