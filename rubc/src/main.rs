#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;

use clap::Parser;
use pixels::{Error, Pixels, SurfaceTexture};
use rubc_core::logger;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod gui;

#[derive(Parser, Debug)]
struct Args {
    rom_file: String,
}

fn main() -> rubc_core::Result<()> {
    logger::setup_logger()?;

    let args = Args::parse();
    println!("Executing file: {}", args.rom_file);

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let size = LogicalSize::new(640.0, 320.0);
        WindowBuilder::new()
            .with_title("RuBC - Rust Boy Color Emulator")
            .with_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture =
            pixels::SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(640, 320, surface_texture)?;
        let framework = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            scale_factor,
            &pixels,
        );
        (pixels, framework)
    };

    let mut emulator = Rubc::new(&args.rom_file);

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    log::error!("Error resizing pixels: {}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                framework.resize(size.width, size.height);
            }

            // Update internal state and request a redraw
            emulator.update();
            window.request_redraw();
        }

        match event {
            Event::WindowEvent { event, .. } => {
                framework.handle_event(&event);
            }
            Event::RedrawRequested(_) => {
                // Draw the world
                emulator.draw(pixels.frame_mut());

                // Prepare egui
                framework.prepare(&window);

                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);

                    // Render egui
                    framework.render(encoder, render_target, context);

                    Ok(())
                });

                // Basic error handling
                if let Err(err) = render_result {
                    // log_error("pixels.render", err);
                    log::error!("Error rendering pixels: {}", err);
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
} // Add a closing parenthesis here

struct Rubc {
    gameboy: rubc_core::gameboy::Gameboy,
}

impl Rubc {
    fn new(rom_file: &str) -> Self {
        let builder = rubc_core::gameboy::GameboyBuilder::new().with_cart(rom_file);
        Rubc {
            gameboy: builder.build(),
        }
    }
    fn update(&mut self) {}
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let color = if i % 2 == 0 { 0 } else { 255 };
            pixel.copy_from_slice(&[color, color, color, 255]);
        }
    }
}
