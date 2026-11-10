use log::*;
use std::num::NonZeroU32;
use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

use softbuffer::{Context, Surface};

use gameboy_core::{Gameboy, LCD_HEIGHT, LCD_WIDTH};

use crate::user_input;

const SCALE: usize = 3;

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
            let window_attrs = Window::default_attributes()
                .with_title("JuegoBoy")
                .with_inner_size(LogicalSize::new(
                    (LCD_WIDTH * SCALE) as u32,
                    (LCD_HEIGHT * SCALE) as u32,
                ));
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
                let size = surface.window().inner_size();
                let width = size.width as usize;
                let height = size.height as usize;

                let frame = self.gameboy.get_frame();
                let mut buffer = surface.buffer_mut().unwrap();

                for adjacent_y in 0..height {
                    let src_y = adjacent_y * LCD_HEIGHT / height;
                    for adjacent_x in 0..width {
                        let src_x = adjacent_x * LCD_WIDTH / width;
                        let pixel = frame[src_y * LCD_WIDTH + src_x];
                        let value = match pixel {
                            0b00 => 0xE0E0E0,
                            0b01 => 0x88C070,
                            0b10 => 0x346856,
                            0b11 => 0x081820,
                            _ => {
                                debug_assert!(false, "Invalid pixel value");
                                0x55BBFF
                            }
                        };
                        buffer[adjacent_y * width + adjacent_x] = value;
                    }
                }

                buffer.present().unwrap();
            }
            _ => (),
        }
    }
}
