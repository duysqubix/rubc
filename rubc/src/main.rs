#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;

use clap::Parser;
use pixels::Pixels;
use rubc_core::logger;
use std::time;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
mod gui;

#[derive(Parser, Debug)]
struct Args {
    rom_file: String,

    #[clap(long, help = "Disassemble the ROM as <ROM_FILE>.txt and exit.")]
    disassemble: bool,

    #[clap(long, help = "Print CPU state between PC addresses.", num_args=1.., value_terminator=";", value_delimiter=',',value_name="PC1,PC2,...PC3;")]
    log_cpu: Vec<u16>,
}

const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;
const SCALE: f32 = 2.0;
const TITLE: &str = "RuBC";
const FPS_US: u64 = 16_740;
const CPU_HZ: u64 = 4_194_304;

fn main() -> rubc_core::Result<()> {
    logger::setup_logger()?;

    let args = Args::parse();
    let mut emulator = Rubc::new(&args.rom_file)?;
    println!("{:?}", args.log_cpu);
    if args.disassemble {
        log::info!("Dumping instruction set");
        let x = rubc_core::utils::disassemble(&emulator.gameboy.cart);
        // print to file
        std::fs::write(format!("{}.txt", args.rom_file), x)?;
        log::debug!("Dumped instruction set to {}.txt", args.rom_file);
        println!("Dumped instruction set to {}.txt", args.rom_file);
        return Ok(());
    }

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let width = WIDTH as f64 * SCALE as f64;
        let height = HEIGHT as f64 * SCALE as f64;
        let size = LogicalSize::new(width, height);
        WindowBuilder::new()
            .with_title(TITLE)
            .with_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let surface_texture =
            pixels::SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
        let framework = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            window.scale_factor() as f32,
            &pixels,
        );

        (pixels, framework)
    };

    // panic!();
    let fps_target = time::Duration::from_micros(FPS_US);

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            let now = time::Instant::now();

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

            let elapsed = now.elapsed();
            if elapsed < fps_target {
                std::thread::sleep(fps_target - elapsed);
            }
            window.set_title(&format!(
                "{} FPS:{:.1}",
                TITLE,
                1.0 / now.elapsed().as_secs_f64()
            ));
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
    fn new(rom_file: &str) -> anyhow::Result<Self> {
        let builder = rubc_core::gameboy::GameboyBuilder::new().with_cart(rom_file);
        Ok(Rubc {
            gameboy: builder?.build(),
        })
    }
    fn update(&mut self) {
        let cycles = CPU_HZ as f64 * ((FPS_US as f64) / 1_000_000.0);
        for _ in 0..cycles as u64 {
            self.gameboy.tick().unwrap();
        }
        log::trace!("processed {} cycles", cycles as u64);
    }
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let color = if i % 2 == 0 { 0 } else { i & 0xFF };
            pixel.copy_from_slice(&[color as u8, color as u8, color as u8, 255]);
        }
    }
}
