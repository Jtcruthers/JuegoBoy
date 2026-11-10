use log::*;
use std::num::NonZeroU32;
use std::rc::Rc;

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
        debug!("about to wait started");
        self.gameboy.step_frame();
        debug!("stepped 1 frame");

        let AppState::Running { surface } = &mut self.state else {
            unreachable!("got about to wait event while not running");
        };
        let window = surface.window().clone();
        window.request_redraw();
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

                let frame = self.gameboy.get_frame();

                for (index, pixel) in frame.iter().enumerate() {
                    let x = index as usize % 160;
                    let y = index as usize / 160;
                    let value = match pixel {
                        0b00 => 0xFFFFFF,
                        0b01 => 0xAAAAAA,
                        0b10 => 0x555555,
                        0b11 => 0x0,
                        _ => {
                            panic!("Invalid pixel value");
                        }
                    };

                    buffer[(y * width as usize + x) as usize] = value;
                }

                buffer.present().unwrap();
            }
            _ => (),
        }
    }
}
