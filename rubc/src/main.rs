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
use rubc_core::bus::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use rubc_core::logger;
use rubc_core::machine::{Machine, RunStop};
use std::time;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod audio;
mod capture;
mod gui;

const WIDTH: u32 = SCREEN_WIDTH as u32;
const HEIGHT: u32 = SCREEN_HEIGHT as u32;
const SCALE: f32 = 3.0;
const TITLE: &str = "rubc";
/// Game Boy frame period: 70224 dots / 4194304 Hz = 16742.7 us (~59.727 FPS).
const FPS_US: u64 = 16_743;
/// Generous instruction budget for headless test-ROM runs.
const HEADLESS_MAX_INSTRUCTIONS: u64 = 250_000_000;

/// Keyboard -> Game Boy controls, shown in `--help` and `rubc controls`.
const CONTROLS_HELP: &str = "\
CONTROLS (windowed mode)\n\
  D-pad .......... Arrow keys (Up / Down / Left / Right)\n\
  A button ....... X\n\
  B button ....... Z\n\
  Start .......... Enter\n\
  Select ......... Right Shift  (or Backspace)\n\
  Quit ........... Esc  (or close the window)\n\
";

#[derive(Parser, Debug)]
#[command(
    name = "rubc",
    version,
    about = "A cycle-accurate Game Boy (DMG/CGB) emulator",
    after_help = CONTROLS_HELP
)]
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
    /// Print the keyboard -> Game Boy control mapping.
    /// Boot a ROM headlessly and write a single-frame PNG screenshot.
    Screenshot {
        /// Path to the `.gb` / `.gbc` ROM image.
        rom: String,
        /// Output PNG path.
        #[arg(long, value_name = "PNG")]
        out: String,
        /// Frames to run before capturing (test ROMs need enough to reach
        /// their result screen; games need enough for a stable frame).
        #[arg(long, default_value_t = 600)]
        frames: u32,
        /// Nearest-neighbour upscale factor (1 = native 160x144).
        #[arg(long, default_value_t = 1)]
        scale: u32,
        /// Force DMG mode regardless of the cartridge CGB flag.
        #[arg(long, conflicts_with = "force_cgb")]
        force_dmg: bool,
        /// Force CGB mode regardless of the cartridge CGB flag.
        #[arg(long)]
        force_cgb: bool,
    },
    Controls,
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
        Some(Command::Screenshot {
            rom,
            out,
            frames,
            scale,
            force_dmg,
            force_cgb,
        }) => {
            let mut machine = boot(&rom, force_dmg, force_cgb)?;
            capture::capture_screenshot(&mut machine, std::path::Path::new(&out), frames, scale)?;
            println!("wrote screenshot to {out} ({frames} frames, scale {scale})");
            Ok(())
        }
        Some(Command::Controls) => {
            print!("{CONTROLS_HELP}");
            Ok(())
        }
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
fn boot(rom_path: &str, force_dmg: bool, force_cgb: bool) -> anyhow::Result<Machine> {
    let rom = std::fs::read(rom_path)
        .map_err(|e| anyhow::anyhow!("failed to read ROM {rom_path:?}: {e}"))?;
    let cgb_flag = rom.get(0x0143).is_some_and(|f| f & 0x80 != 0);
    let cgb = if force_dmg {
        false
    } else if force_cgb {
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
    let mut machine = boot(rom_path, opts.force_dmg, opts.force_cgb)?;
    if opts.no_gui {
        // Headless test-ROM runs never touch the filesystem for saves.
        return run_headless(&mut machine, opts);
    }
    // Windowed: persist battery-backed cart RAM to a `.sav` beside the ROM.
    let save_path = sav_path(rom_path);
    if machine.has_battery() {
        load_save(&mut machine, &save_path);
    }
    run_windowed(machine, save_path)
}

/// The save-file path for a ROM: the ROM path with its extension replaced by
/// `sav` (e.g. `foo/bar.gbc` -> `foo/bar.sav`). A ROM with no extension simply
/// gains a `.sav` one.
fn sav_path(rom_path: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(rom_path);
    p.set_extension("sav");
    p
}

/// Load a battery-backed save file into the machine's cart RAM, if present.
/// Best-effort: a missing file is normal (a fresh save) and any read error is
/// logged but never fatal -- the game still boots with blank RAM.
fn load_save(machine: &mut Machine, save_path: &std::path::Path) {
    match std::fs::read(save_path) {
        Ok(bytes) => {
            machine.load_ram(&bytes);
            log::info!("loaded battery save {save_path:?} ({} bytes)", bytes.len());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!("no battery save at {save_path:?}; starting with blank RAM");
        }
        Err(e) => {
            log::warn!("failed to read battery save {save_path:?}: {e}");
        }
    }
}

/// Write the machine's cart RAM to the `.sav` file. Best-effort: I/O errors are
/// logged, never fatal. No-op when the cart has no battery or no RAM.
fn persist_save(machine: &Machine, save_path: &std::path::Path) {
    if !machine.has_battery() || machine.save_ram().is_empty() {
        return;
    }
    if let Err(e) = std::fs::write(save_path, machine.save_ram()) {
        log::warn!("failed to write battery save {save_path:?}: {e}");
    }
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
        RunStop::BlarggDone => machine.blargg_passed(),
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
fn run_windowed(mut machine: Machine, save_path: std::path::PathBuf) -> anyhow::Result<()> {
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
        // Disable vsync (PresentMode::Immediate): the default AutoVsync/Fifo
        // locks render to the 60 Hz monitor, capping the 59.7 Hz Game Boy frame
        // at ~56 FPS. With vsync off, our own thread::sleep paces the frame.
        let pixels = pixels::PixelsBuilder::new(WIDTH, HEIGHT, surface_texture)
            .present_mode(pixels::wgpu::PresentMode::Immediate)
            .build()?;
        let framework = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            window.scale_factor() as f32,
            &pixels,
        );
        (pixels, framework)
    };

    // Audio: open the default output device and tell the APU to produce samples
    // at the device's native rate (no resampling). If no device is available
    // (headless CI, no soundcard), warn and run silently -- the emulator must
    // still run without audio.
    let audio = match audio::AudioOutput::new() {
        Ok(a) => {
            machine.bus.apu.set_sample_rate(a.sample_rate());
            log::info!("audio: output enabled at {} Hz", a.sample_rate());
            Some(a)
        }
        Err(e) => {
            log::warn!("audio: disabled ({e}); continuing without sound");
            None
        }
    };
    // Scratch buffer reused each frame to drain APU samples without realloc.
    let mut audio_scratch: Vec<f32> = Vec::new();
    // Per-second counter of stereo frames pushed to the device (audio evidence).
    let mut audio_frames_pushed: usize = 0;
    let mut audio_log_start = time::Instant::now();

    let fps_target = time::Duration::from_micros(FPS_US);
    // Tracks the start of the previous frame so the displayed FPS reflects the
    // true frame-to-frame period (emulation + render + sleep), not just the
    // paced work window.
    let mut last_frame = time::Instant::now();
    // Absolute next-frame deadline: advanced by exactly one frame period each
    // iteration so pacing does not drift even if a frame runs long or sleep
    // overshoots (macOS thread::sleep can over-sleep by ~1ms).
    let mut next_deadline = time::Instant::now() + fps_target;
    // Rolling FPS counter: log the measured framerate once per second so it can
    // be monitored headlessly (fern writes to stdout + the log file).
    let mut fps_window_start = time::Instant::now();
    let mut frames_this_window: u32 = 0;
    // Persist battery RAM at most once per second while running, so progress
    // is not lost if the process is killed without a clean exit.
    let mut last_save = time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        // Run the loop continuously (default is Wait, which gates frames on
        // events + the vsync'd render and caps FPS below target); our own
        // thread::sleep paces to the Game Boy frame period.
        control_flow.set_poll();
        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                // Clean exit (Esc or window close): flush the save first.
                persist_save(&machine, &save_path);
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

            // Map keyboard -> Game Boy joypad. Z=B X=A Enter=Start RShift/
            // Backspace=Select, arrow keys=d-pad. key_held drives the active-low
            // register; key_pressed edges raise the joypad interrupt in core.
            use rubc_core::bus::Button;
            for (key, button) in [
                (VirtualKeyCode::Up, Button::Up),
                (VirtualKeyCode::Down, Button::Down),
                (VirtualKeyCode::Left, Button::Left),
                (VirtualKeyCode::Right, Button::Right),
                (VirtualKeyCode::X, Button::A),
                (VirtualKeyCode::Z, Button::B),
                (VirtualKeyCode::Return, Button::Start),
                (VirtualKeyCode::RShift, Button::Select),
                (VirtualKeyCode::Back, Button::Select),
            ] {
                machine.set_button(button, input.key_held(key));
            }

            // One full frame: emulate, draw, and PRESENT inline so the render
            // cost is inside the pacing budget (the old split RedrawRequested
            // render leaked ~1ms past the target and capped FPS at ~56).
            machine.step_frame();
            // Periodic battery-save flush (best-effort, gated inside).
            if last_save.elapsed() >= time::Duration::from_secs(1) {
                persist_save(&machine, &save_path);
                last_save = time::Instant::now();
            }
            // Feed this frame's APU samples to the audio device. The APU was
            // told to emit at the device's native rate, so push them straight
            // through with no resampling.
            if let Some(audio) = &audio {
                audio_scratch.clear();
                machine.bus.apu.drain_samples(&mut audio_scratch);
                audio_frames_pushed += audio_scratch.len() / 2;
                audio.push_samples(&audio_scratch);
            }
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
                return;
            }

            // Pace to an ABSOLUTE per-frame deadline. Sleep until just shy of it
            // (macOS thread::sleep tends to overshoot), then advance the deadline
            // by exactly one frame period so timing never drifts.
            let now = time::Instant::now();
            if now < next_deadline {
                // Leave a 1ms slack and let the loop spin the remainder.
                let slack = time::Duration::from_millis(1);
                if next_deadline - now > slack {
                    std::thread::sleep(next_deadline - now - slack);
                }
                while time::Instant::now() < next_deadline {
                    std::hint::spin_loop();
                }
            }
            let period = last_frame.elapsed();
            last_frame = time::Instant::now();
            // Advance the deadline; if we fell badly behind, resync to now.
            next_deadline += fps_target;
            if next_deadline < last_frame {
                next_deadline = last_frame + fps_target;
            }
            window.set_title(&format!("{TITLE}  {:.3} FPS", 1.0 / period.as_secs_f64()));

            // Once per second, log the average FPS over the window.
            frames_this_window += 1;
            let win = fps_window_start.elapsed();
            if win >= time::Duration::from_secs(1) {
                let fps = frames_this_window as f64 / win.as_secs_f64();
                log::info!(
                    "fps: {fps:.3} ({frames_this_window} frames in {:.3}s)",
                    win.as_secs_f64()
                );
                frames_this_window = 0;
                fps_window_start = time::Instant::now();
            }

            // Once per second, log audio frames pushed to the device + current
            // device-side buffer depth: evidence the APU->device path carries
            // real (non-zero) samples.
            if let Some(audio) = &audio {
                let alog = audio_log_start.elapsed();
                if alog >= time::Duration::from_secs(1) {
                    log::info!(
                        "audio: {audio_frames_pushed} frames pushed in {:.3}s, {} frames buffered",
                        alog.as_secs_f64(),
                        audio.buffered_frames(),
                    );
                    audio_frames_pushed = 0;
                    audio_log_start = time::Instant::now();
                }
            }
        }

        // egui still needs window events; the frame render happens inline above.
        if let Event::WindowEvent { event, .. } = event {
            framework.handle_event(&event);
        }
    });
}

/// Map the PPU's resolved framebuffer into the RGBA `pixels` buffer. Shares the
/// per-pixel shade->RGB mapping with the headless capture path so the window and
/// screenshots/GIFs render identically.
fn draw_framebuffer(machine: &Machine, frame: &mut [u8]) {
    let fb = &machine.bus.ppu.framebuffer;
    for (px, &pixel) in frame.chunks_exact_mut(4).zip(fb.iter()) {
        px.copy_from_slice(&capture::frame_pixel_rgba(pixel));
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
    out.push_str(&format!(
        "size:      {} bytes ({} KiB)\n",
        rom.len(),
        rom.len() / 1024
    ));
    out.push_str(&format!("title:     {title}\n"));
    out.push_str(&format!(
        "cgb flag:  {cgb:#04X} ({})\n",
        match cgb & 0xC0 {
            0x80 => "CGB-enhanced",
            0xC0 => "CGB-only",
            _ => "DMG",
        }
    ));
    out.push_str(&format!(
        "cart type: {cart_type:#04X} ({})\n",
        cart_type_name(cart_type)
    ));
    out.push_str(&format!(
        "rom size:  {rom_code:#04X} ({})\n",
        rom_size_str(rom_code)
    ));
    out.push_str(&format!(
        "ram size:  {ram_code:#04X} ({})\n",
        ram_size_str(ram_code)
    ));

    if raw {
        out.push_str("\nheader bytes 0x0100-0x014F:\n");
        for (i, chunk) in rom
            .get(0x0100..=0x014F)
            .unwrap_or(&[])
            .chunks(16)
            .enumerate()
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sav_path_replaces_rom_extension() {
        assert_eq!(sav_path("foo/bar.gbc"), PathBuf::from("foo/bar.sav"));
        assert_eq!(sav_path("game.gb"), PathBuf::from("game.sav"));
        assert_eq!(
            sav_path("/abs/path/poke.gbc"),
            PathBuf::from("/abs/path/poke.sav")
        );
    }

    #[test]
    fn sav_path_appends_when_no_extension() {
        assert_eq!(sav_path("dir/sub/rom"), PathBuf::from("dir/sub/rom.sav"));
    }
}
