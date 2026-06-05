#![deny(clippy::all)]
#![forbid(unsafe_code)]

//! rubc command-line interface.
//!
//! Subcommands (clap):
//! - `run ROM [opts]` boots a ROM on the M-cycle `Machine` core; windowed by
//!   default, `--no-gui` for headless test ROMs.
//! - `cartdump ROM [opts]` prints the cartridge header + a few derived facts.
//! - bare `ROM [opts]` is shorthand for `run ROM`.

use crate::gui::Framework;

use clap::{Parser, Subcommand};
use pixels::Pixels;
use rubc_core::bus::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use rubc_core::logger;
use rubc_core::machine::{Machine, RunStop};
use std::time;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod gui;

const WIDTH: u32 = SCREEN_WIDTH as u32;
const HEIGHT: u32 = SCREEN_HEIGHT as u32;
const SCALE: f32 = 3.0;
const TITLE: &str = "rubc";
const FPS_US: u64 = 16_740;
/// Generous instruction budget for headless test-ROM runs.
const HEADLESS_MAX_INSTRUCTIONS: u64 = 250_000_000;

#[derive(Parser, Debug)]
#[command(name = "rubc", version, about = "A cycle-accurate Game Boy (DMG/CGB) emulator")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Shorthand: a bare ROM path behaves like `rubc run ROM` (when no
    /// subcommand is given).
    #[arg(value_name = "ROM")]
    rom: Option<String>,

    #[command(flatten)]
    run_opts: RunOpts,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Boot a ROM and run it (windowed unless `--no-gui`).
    Run {
        /// Path to the `.gb` / `.gbc` ROM image.
        rom: String,
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Print the cartridge header (title, MBC, ROM/RAM size, CGB flag).
    Cartdump {
        /// Path to the ROM image.
        rom: String,
        /// Print raw header bytes alongside the decoded fields.
        #[arg(long)]
        raw: bool,
        /// Write the dump to a file instead of stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<String>,
    },
}

/// Options shared by `run` and the bare-ROM shorthand.
#[derive(clap::Args, Debug, Default)]
struct RunOpts {
    /// Run headless (no window) and report the test-ROM result. For Blargg /
    /// Mooneye acceptance ROMs in CI.
    #[arg(long)]
    no_gui: bool,

    /// Force DMG mode regardless of the cartridge CGB flag.
    #[arg(long, conflicts_with = "force_cgb")]
    force_dmg: bool,

    /// Force CGB mode regardless of the cartridge CGB flag.
    #[arg(long)]
    force_cgb: bool,

    /// Headless pass-detection mode for test ROMs.
    #[arg(long, value_enum, default_value_t = TestKind::Auto)]
    test: TestKind,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TestKind {
    /// Pick blargg (serial) or mooneye (register signature) from the result.
    #[default]
    Auto,
    /// Blargg ROMs: pass/fail reported on the serial port.
    Blargg,
    /// Mooneye ROMs: pass = Fibonacci register signature at `LD B,B`.
    Mooneye,
}

fn main() -> anyhow::Result<()> {
    logger::setup_logger().ok();
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run { rom, opts }) => run(&rom, &opts),
        Some(Command::Cartdump { rom, raw, output }) => cartdump(&rom, raw, output.as_deref()),
        None => match cli.rom {
            Some(rom) => run(&rom, &cli.run_opts), // bare-ROM shorthand
            None => {
                eprintln!("error: no ROM given. Try `rubc run <ROM>` or `rubc --help`.");
                std::process::exit(2);
            }
        },
    }
}

/// Build a `Machine` from a ROM file, honoring the DMG/CGB boot override.
fn boot(rom_path: &str, opts: &RunOpts) -> anyhow::Result<Machine> {
    let rom = std::fs::read(rom_path)
        .map_err(|e| anyhow::anyhow!("failed to read ROM {rom_path:?}: {e}"))?;
    let cgb_flag = rom.get(0x0143).is_some_and(|f| f & 0x80 != 0);
    let cgb = if opts.force_dmg {
        false
    } else if opts.force_cgb {
        true
    } else {
        cgb_flag
    };
    Ok(if cgb {
        Machine::boot_cgb(&rom)
    } else {
        Machine::boot_dmg(&rom)
    })
}

fn run(rom_path: &str, opts: &RunOpts) -> anyhow::Result<()> {
    let mut machine = boot(rom_path, opts)?;
    if opts.no_gui {
        return run_headless(&mut machine, opts);
    }
    run_windowed(machine)
}

/// Headless: run a test ROM to its terminal condition and report pass/fail.
fn run_headless(machine: &mut Machine, opts: &RunOpts) -> anyhow::Result<()> {
    let kind = opts.test;
    let stop = match kind {
        TestKind::Mooneye => machine.run_mooneye(HEADLESS_MAX_INSTRUCTIONS),
        TestKind::Blargg | TestKind::Auto => machine.run_blargg(HEADLESS_MAX_INSTRUCTIONS),
    };

    let serial = machine.serial_text().unwrap_or_default();
    let passed = match stop {
        RunStop::MooneyeBreakpoint => machine.mooneye_passed(),
        RunStop::BlarggDone => serial.contains("Passed"),
        RunStop::Timeout | RunStop::Stuck => false,
    };

    if !serial.is_empty() {
        println!("{}", serial.trim_end());
    }
    println!(
        "result: {} (stop={stop:?})",
        if passed { "PASS" } else { "FAIL" }
    );
    if passed {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

/// Windowed: run the emulator and render the PPU framebuffer to the window.
fn run_windowed(mut machine: Machine) -> anyhow::Result<()> {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let size = LogicalSize::new(WIDTH as f64 * SCALE as f64, HEIGHT as f64 * SCALE as f64);
        WindowBuilder::new()
            .with_title(TITLE)
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(WIDTH as f64, HEIGHT as f64))
            .build(&event_loop)?
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

    let fps_target = time::Duration::from_micros(FPS_US);

    event_loop.run(move |event, _, control_flow| {
        if input.update(&event) {
            let now = time::Instant::now();

            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }
            if let Some(size) = input.window_resized() {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    log::error!("pixels.resize_surface: {err}");
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                framework.resize(size.width, size.height);
            }

            // Advance one full frame of emulation, then present it.
            machine.step_frame();
            window.request_redraw();

            let elapsed = now.elapsed();
            if elapsed < fps_target {
                std::thread::sleep(fps_target - elapsed);
            }
            window.set_title(&format!("{TITLE}  {:.0} FPS", 1.0 / now.elapsed().as_secs_f64()));
        }

        match event {
            Event::WindowEvent { event, .. } => framework.handle_event(&event),
            Event::RedrawRequested(_) => {
                draw_framebuffer(&machine, pixels.frame_mut());
                framework.prepare(&window);
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    context.scaling_renderer.render(encoder, render_target);
                    framework.render(encoder, render_target, context);
                    Ok(())
                });
                if let Err(err) = render_result {
                    log::error!("pixels.render: {err}");
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

/// The 4 DMG shades (lightest -> darkest) as RGBA, applied through BGP ($FF47).
const DMG_SHADES: [[u8; 4]; 4] = [
    [0xE0, 0xF8, 0xD0, 0xFF], // 0: lightest
    [0x88, 0xC0, 0x70, 0xFF], // 1
    [0x34, 0x68, 0x56, 0xFF], // 2
    [0x08, 0x18, 0x20, 0xFF], // 3: darkest
];

/// Map the PPU's raw 2-bit framebuffer through the BG palette (BGP, $FF47) into
/// the RGBA `pixels` buffer.
fn draw_framebuffer(machine: &Machine, frame: &mut [u8]) {
    let bgp = machine.bus.peek(0xFF47);
    let fb = &machine.bus.ppu.framebuffer;
    for (px, &raw) in frame.chunks_exact_mut(4).zip(fb.iter()) {
        // BGP maps each 2-bit color index to a shade: bits [1:0]=idx0, [3:2]=idx1...
        let shade = (bgp >> (raw * 2)) & 0x03;
        px.copy_from_slice(&DMG_SHADES[shade as usize]);
    }
}

/// `cartdump`: decode and print the cartridge header.
fn cartdump(rom_path: &str, raw: bool, output: Option<&str>) -> anyhow::Result<()> {
    let rom = std::fs::read(rom_path)
        .map_err(|e| anyhow::anyhow!("failed to read ROM {rom_path:?}: {e}"))?;

    let mut out = String::new();
    let title: String = rom
        .get(0x0134..=0x0143)
        .unwrap_or(&[])
        .iter()
        .take_while(|&&b| b != 0)
        .filter(|&&b| (0x20..0x7F).contains(&b))
        .map(|&b| b as char)
        .collect();
    let cgb = rom.get(0x0143).copied().unwrap_or(0);
    let cart_type = rom.get(0x0147).copied().unwrap_or(0);
    let rom_code = rom.get(0x0148).copied().unwrap_or(0);
    let ram_code = rom.get(0x0149).copied().unwrap_or(0);

    out.push_str(&format!("file:      {rom_path}\n"));
    out.push_str(&format!("size:      {} bytes ({} KiB)\n", rom.len(), rom.len() / 1024));
    out.push_str(&format!("title:     {title}\n"));
    out.push_str(&format!(
        "cgb flag:  {cgb:#04X} ({})\n",
        match cgb & 0xC0 {
            0x80 => "CGB-enhanced",
            0xC0 => "CGB-only",
            _ => "DMG",
        }
    ));
    out.push_str(&format!("cart type: {cart_type:#04X} ({})\n", cart_type_name(cart_type)));
    out.push_str(&format!("rom size:  {rom_code:#04X} ({})\n", rom_size_str(rom_code)));
    out.push_str(&format!("ram size:  {ram_code:#04X} ({})\n", ram_size_str(ram_code)));

    if raw {
        out.push_str("\nheader bytes 0x0100-0x014F:\n");
        for (i, chunk) in rom.get(0x0100..=0x014F).unwrap_or(&[]).chunks(16).enumerate() {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
            out.push_str(&format!("  {:04X}: {}\n", 0x0100 + i * 16, hex.join(" ")));
        }
    }

    match output {
        Some(path) => {
            std::fs::write(path, &out)?;
            println!("wrote cartridge dump to {path}");
        }
        None => print!("{out}"),
    }
    Ok(())
}

fn cart_type_name(t: u8) -> &'static str {
    match t {
        0x00 => "ROM ONLY",
        0x01 => "MBC1",
        0x02 => "MBC1+RAM",
        0x03 => "MBC1+RAM+BATTERY",
        0x05 => "MBC2",
        0x06 => "MBC2+BATTERY",
        0x08 => "ROM+RAM",
        0x09 => "ROM+RAM+BATTERY",
        0x0F..=0x13 => "MBC3",
        0x19..=0x1E => "MBC5",
        _ => "unknown / unsupported",
    }
}

fn rom_size_str(code: u8) -> String {
    match code {
        0x00..=0x08 => format!("{} KiB, {} banks", 32 << code, 2usize << code),
        _ => "unknown".to_string(),
    }
}

fn ram_size_str(code: u8) -> &'static str {
    match code {
        0x00 => "none",
        0x02 => "8 KiB (1 bank)",
        0x03 => "32 KiB (4 banks)",
        0x04 => "128 KiB (16 banks)",
        0x05 => "64 KiB (8 banks)",
        _ => "unknown",
    }
}
