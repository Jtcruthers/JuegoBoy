use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

use softbuffer::{Context, Surface};

use gameboy_core::Gameboy;

use crate::user_input;

// const WIDTH: u32 = 160;
// const HEIGHT: u32 = 144;

pub struct App {
    gameboy: Gameboy,
    context: Context<OwnedDisplayHandle>,
    state: AppState,
}

#[derive(Debug)]
enum AppState {
    Initial,
    Suspended {
        window: Rc<Window>,
    },
    Running {
        surface: Surface<OwnedDisplayHandle, Rc<Window>>,
    },
}

impl App {
    pub fn new(rom: Vec<u8>, context: Context<OwnedDisplayHandle>) -> Self {
        let gameboy = Gameboy::new(rom);
        Self {
            gameboy: gameboy,
            context: context,
            state: AppState::Initial,
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::Init = cause {
            // Create window on startup.
            let window_attrs = Window::default_attributes().with_title("JuegoBoy");
            let window = event_loop
                .create_window(window_attrs)
                .expect("failed creating window");
            self.state = AppState::Suspended {
                window: Rc::new(window),
            };
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        let AppState::Suspended { window } = &mut self.state else {
            unreachable!("got resumed event while not suspended");
        };
        let mut surface =
            Surface::new(&self.context, window.clone()).expect("failed creating surface");

        // TODO: https://github.com/rust-windowing/softbuffer/issues/106
        let size = window.inner_size();
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            // Resize surface
            surface.resize(width, height).unwrap();
        }

        self.state = AppState::Running { surface };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Drop the surface.
        let AppState::Running { surface } = &mut self.state else {
            unreachable!("got resumed event while not running");
        };
        let window = surface.window().clone();
        self.state = AppState::Suspended { window };
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(10) {
            for _ in 0..100000 {
                self.gameboy.step();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let AppState::Running { surface } = &mut self.state else {
            unreachable!("got window event while suspended");
        };

        if surface.window().id() != window_id {
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    // Resize surface
                    surface.resize(width, height).unwrap();
                }
            }
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                user_input::handle_user_input(&mut self.gameboy, event)
            }
            WindowEvent::RedrawRequested => {
                let width = surface.window().inner_size().width;

                let mut buffer = surface.buffer_mut().unwrap();

                for (index, pixel) in buffer.iter_mut().enumerate() {
                    let x = index as u32 % width;
                    let y = index as u32 / width;
                    let red = x % 255;
                    let green = y % 255;
                    let blue = (x * y) % 255;

                    *pixel = (red << 16) | (green << 8) | blue;
                }

                buffer.present().unwrap();
            }
            _ => (),
        }
    }
}
