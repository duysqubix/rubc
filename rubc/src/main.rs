#![deny(clippy::all)]
#![forbid(unsafe_code)]

//! rubc command-line interface.
//!
//! Subcommands (clap):
//! - `run ROM [opts]` boots a ROM on the M-cycle `Machine` core; windowed by
//!   default, `--no-gui` for headless test ROMs.
//! - `cartdump ROM [opts]` prints the cartridge header + a few derived facts.
//! - bare `ROM [opts]` is shorthand for `run ROM`.

use crate::gui::Gui;

use clap::{Parser, Subcommand};
use eframe::egui;
use rubc_core::bus::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use rubc_core::logger;
use rubc_core::machine::{Machine, RunStop};
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod audio;
mod capture;
mod gui;
mod vramview;

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
    /// Boot a ROM headlessly and record an animated, looping GIF.
    Gif {
        /// Path to the `.gb` / `.gbc` ROM image.
        rom: String,
        /// Output GIF path.
        #[arg(long, value_name = "GIF")]
        out: String,
        /// Number of GIF frames to capture.
        #[arg(long, default_value_t = 120)]
        frames: u32,
        /// Capture one GIF frame every M emulator frames (2 = ~30fps).
        #[arg(long, default_value_t = 2)]
        every: u32,
        /// Nearest-neighbour upscale factor.
        #[arg(long, default_value_t = 3)]
        scale: u32,
        /// Skip this many warm-up frames (boot logos) before recording.
        #[arg(long, default_value_t = 0)]
        skip: u32,
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
        Some(Command::Gif {
            rom,
            out,
            frames,
            every,
            scale,
            skip,
            force_dmg,
            force_cgb,
        }) => {
            let mut machine = boot(&rom, force_dmg, force_cgb)?;
            capture::capture_gif(
                &mut machine,
                std::path::Path::new(&out),
                frames,
                every,
                scale,
                skip,
            )?;
            println!(
                "wrote gif to {out} ({frames} frames, every {every}, scale {scale}, skip {skip})"
            );
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

/// Windowed: run the emulator inside an `eframe` native window, rendering the
/// PPU framebuffer as a texture and driving input/audio/saves each frame.
///
/// `eframe` owns the winit event loop + wgpu surface; we supply an
/// [`eframe::App`] ([`RubcApp`]) whose `update` runs exactly one Game Boy frame
/// per repaint. The emulator is paced to the GB frame period by an absolute
/// per-frame deadline (identical algorithm to the previous manual loop), and
/// `request_repaint` is called every frame so eframe drives the loop
/// continuously rather than waiting on input.
fn run_windowed(mut machine: Machine, save_path: PathBuf) -> anyhow::Result<()> {
    // Audio: open the default output device and tell the APU to produce samples
    // at the device's native rate (no resampling). If no device is available
    // (headless CI, no soundcard), warn and run silently -- the emulator must
    // still run without audio. Created here (before the eframe loop) and moved
    // into the app; the cpal stream runs on its own realtime callback thread,
    // independent of eframe's repaint cadence.
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

    // Disable vsync so eframe's present does not lock to the 60 Hz monitor and
    // cap the 59.7 Hz Game Boy frame at ~56 FPS; our own deadline + sleep paces
    // the frame (matching the old `PresentMode::Immediate` behavior).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([WIDTH as f32 * SCALE, HEIGHT as f32 * SCALE])
            .with_min_inner_size([WIDTH as f32, HEIGHT as f32]),
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        TITLE,
        options,
        Box::new(move |_cc| Ok(Box::new(RubcApp::new(machine, save_path, audio)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe run_native failed: {e}"))
}

/// The windowed application state, driven by `eframe`. Holds the emulator, the
/// save path, the audio output, the egui UI ([`Gui`]), the screen texture, and
/// the frame-pacing / periodic-logging timers.
struct RubcApp {
    machine: Machine,
    save_path: PathBuf,
    audio: Option<audio::AudioOutput>,
    /// Scratch buffer reused each frame to drain APU samples without realloc.
    audio_scratch: Vec<f32>,
    /// Per-second counter of stereo frames pushed to the device (audio evidence).
    audio_frames_pushed: usize,
    audio_log_start: Instant,
    gui: Gui,
    /// The 160x144 framebuffer uploaded as a NEAREST-filtered texture; updated
    /// in place each frame (no per-frame texture allocation).
    screen_tex: Option<egui::TextureHandle>,
    fps_target: Duration,
    /// Start of the previous frame, so the displayed FPS reflects the true
    /// frame-to-frame period (emulation + render + sleep).
    last_frame: Instant,
    /// Absolute next-frame deadline, advanced by exactly one frame period each
    /// iteration so pacing does not drift even if a frame runs long or sleep
    /// overshoots (macOS thread::sleep can over-sleep by ~1ms).
    next_deadline: Instant,
    fps_window_start: Instant,
    frames_this_window: u32,
    /// Persist battery RAM at most once per second while running.
    last_save: Instant,
}

impl RubcApp {
    fn new(machine: Machine, save_path: PathBuf, audio: Option<audio::AudioOutput>) -> Self {
        let now = Instant::now();
        let fps_target = Duration::from_micros(FPS_US);
        Self {
            machine,
            save_path,
            audio,
            audio_scratch: Vec::new(),
            audio_frames_pushed: 0,
            audio_log_start: now,
            gui: Gui::new(),
            screen_tex: None,
            fps_target,
            last_frame: now,
            next_deadline: now + fps_target,
            fps_window_start: now,
            frames_this_window: 0,
            last_save: now,
        }
    }
}

impl eframe::App for RubcApp {
    // All emulation (input, stepping, audio, saves, framebuffer upload, frame
    // pacing) lives in `logic`, which eframe calls EVERY frame regardless of
    // window visibility -- so the emulator keeps running (and audio keeps
    // flowing) even when minimized, matching the old continuous winit loop.
    // `ui` (visible-only) just paints the already-uploaded screen texture.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Map keyboard -> Game Boy joypad. Z=B X=A Enter=Start, Backspace or
        // Shift=Select, arrow keys=d-pad. `key_down` drives the active-low
        // register from the held state; key edges raise the joypad interrupt in
        // core. NOTE: egui exposes no distinct Right-Shift key, so EITHER shift
        // key now maps to Select (the old code documented Right Shift; its
        // RShift branch was in any case overwritten by the Backspace branch, so
        // Backspace was already the effective Select key).
        use rubc_core::bus::Button;
        let esc = ctx.input(|i| {
            self.machine
                .set_button(Button::Up, i.key_down(egui::Key::ArrowUp));
            self.machine
                .set_button(Button::Down, i.key_down(egui::Key::ArrowDown));
            self.machine
                .set_button(Button::Left, i.key_down(egui::Key::ArrowLeft));
            self.machine
                .set_button(Button::Right, i.key_down(egui::Key::ArrowRight));
            self.machine.set_button(Button::A, i.key_down(egui::Key::X));
            self.machine.set_button(Button::B, i.key_down(egui::Key::Z));
            self.machine
                .set_button(Button::Start, i.key_down(egui::Key::Enter));
            self.machine.set_button(
                Button::Select,
                i.key_down(egui::Key::Backspace) || i.modifiers.shift,
            );
            i.key_pressed(egui::Key::Escape)
        });
        if esc {
            // Clean exit (Esc): flush the save, then ask eframe to close. The
            // window-close button is covered by `on_exit`.
            persist_save(&self.machine, &self.save_path);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // One full Game Boy frame.
        self.machine.step_frame();

        // Periodic battery-save flush (best-effort, gated inside).
        if self.last_save.elapsed() >= Duration::from_secs(1) {
            persist_save(&self.machine, &self.save_path);
            self.last_save = Instant::now();
        }

        // Feed this frame's APU samples to the audio device. The APU was told to
        // emit at the device's native rate, so push them straight through with
        // no resampling.
        if let Some(audio) = &self.audio {
            self.audio_scratch.clear();
            self.machine.bus.apu.drain_samples(&mut self.audio_scratch);
            self.audio_frames_pushed += self.audio_scratch.len() / 2;
            audio.push_samples(&self.audio_scratch);
        }

        // Upload the resolved framebuffer as a NEAREST-filtered texture (updated
        // in place after the first frame -- no per-frame allocation).
        let image = framebuffer_color_image(&self.machine);
        match &mut self.screen_tex {
            Some(tex) => tex.set(image, egui::TextureOptions::NEAREST),
            None => {
                self.screen_tex =
                    Some(ctx.load_texture("rubc-screen", image, egui::TextureOptions::NEAREST));
            }
        }

        // Hand the debug viewer a read-only VRAM snapshot (only when the window
        // is open). The immutable borrow ends before the egui closures in `ui`.
        if self.gui.debug_open() {
            self.gui
                .set_vram_snapshot(vramview::VramDebugSnapshot::capture(&self.machine));
        }

        // Pace to an ABSOLUTE per-frame deadline. Sleep until just shy of it
        // (macOS thread::sleep tends to overshoot), then spin the remainder so
        // timing never drifts.
        let now = Instant::now();
        if now < self.next_deadline {
            let slack = Duration::from_millis(1);
            if self.next_deadline - now > slack {
                std::thread::sleep(self.next_deadline - now - slack);
            }
            while Instant::now() < self.next_deadline {
                std::hint::spin_loop();
            }
        }
        let period = self.last_frame.elapsed();
        self.last_frame = Instant::now();
        // Advance the deadline; if we fell badly behind, resync to now.
        self.next_deadline += self.fps_target;
        if self.next_deadline < self.last_frame {
            self.next_deadline = self.last_frame + self.fps_target;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{TITLE}  {:.3} FPS",
            1.0 / period.as_secs_f64()
        )));

        // Once per second, log the average FPS over the window.
        self.frames_this_window += 1;
        let win = self.fps_window_start.elapsed();
        if win >= Duration::from_secs(1) {
            let fps = self.frames_this_window as f64 / win.as_secs_f64();
            log::info!(
                "fps: {fps:.3} ({} frames in {:.3}s)",
                self.frames_this_window,
                win.as_secs_f64()
            );
            self.frames_this_window = 0;
            self.fps_window_start = Instant::now();
        }

        // Once per second, log audio frames pushed to the device + current
        // device-side buffer depth: evidence the APU->device path carries real
        // (non-zero) samples.
        if let Some(audio) = &self.audio {
            let alog = self.audio_log_start.elapsed();
            if alog >= Duration::from_secs(1) {
                log::info!(
                    "audio: {} frames pushed in {:.3}s, {} frames buffered",
                    self.audio_frames_pushed,
                    alog.as_secs_f64(),
                    audio.buffered_frames(),
                );
                self.audio_frames_pushed = 0;
                self.audio_log_start = Instant::now();
            }
        }

        // eframe is repaint-driven: request the next frame immediately so the
        // loop runs at the Game Boy cadence set by the deadline pacing above.
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The egui UI: menubar (File -> Debug / About), About window, and the
        // embedded live VRAM viewer. Drawn before the CentralPanel so the game
        // screen fills the remaining space below the menubar.
        self.gui.ui(ui);

        // The game screen: the framebuffer texture, aspect-preserved, centered
        // on a black field, NEAREST-filtered (no blur).
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show_inside(ui, |ui| {
                if let Some(tex) = &self.screen_tex {
                    let avail = ui.available_size();
                    let scale = (avail.x / WIDTH as f32)
                        .min(avail.y / HEIGHT as f32)
                        .max(0.0);
                    let size = egui::vec2(WIDTH as f32 * scale, HEIGHT as f32 * scale);
                    ui.centered_and_justified(|ui| {
                        ui.add(
                            egui::Image::new(egui::load::SizedTexture::new(tex.id(), size))
                                .texture_options(egui::TextureOptions::NEAREST),
                        );
                    });
                }
            });
    }

    fn on_exit(&mut self) {
        // Window-close (or any shutdown) save flush.
        persist_save(&self.machine, &self.save_path);
    }
}

/// Map the PPU's resolved framebuffer into an egui [`egui::ColorImage`]. Shares
/// the per-pixel shade->RGB mapping with the headless capture path so the window
/// and screenshots/GIFs render identically.
fn framebuffer_color_image(machine: &Machine) -> egui::ColorImage {
    let fb = &machine.bus.ppu.framebuffer;
    let pixels: Vec<egui::Color32> = fb
        .iter()
        .map(|&pixel| {
            let [r, g, b, a] = capture::frame_pixel_rgba(pixel);
            // Alpha is always 0xFF, so premultiplied == unmultiplied here.
            egui::Color32::from_rgba_premultiplied(r, g, b, a)
        })
        .collect();
    egui::ColorImage::new([WIDTH as usize, HEIGHT as usize], pixels)
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
