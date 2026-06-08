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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
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
            None => run_windowed(None, None, &cli.run_opts),
        },
    }
}

/// Build a `Machine` from a ROM file, honoring the DMG/CGB boot override.
fn boot(rom_path: &str, force_dmg: bool, force_cgb: bool) -> anyhow::Result<Machine> {
    let rom = std::fs::read(rom_path)
        .map_err(|e| anyhow::anyhow!("failed to read ROM {rom_path:?}: {e}"))?;
    Ok(boot_bytes(&rom, force_dmg, force_cgb))
}

fn boot_bytes(rom: &[u8], force_dmg: bool, force_cgb: bool) -> Machine {
    let cgb_flag = rom.get(0x0143).is_some_and(|f| f & 0x80 != 0);
    let cgb = if force_dmg {
        false
    } else if force_cgb {
        true
    } else {
        cgb_flag
    };
    if cgb {
        Machine::boot_cgb(rom)
    } else {
        Machine::boot_dmg(rom)
    }
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
    run_windowed(Some(machine), Some(save_path), opts)
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
fn run_windowed(machine: Option<Machine>, save_path: Option<PathBuf>, opts: &RunOpts) -> anyhow::Result<()> {
    // Audio: open the default output device and tell the APU to produce samples
    // at the device's native rate (no resampling). If no device is available
    // (headless CI, no soundcard), warn and run silently -- the emulator must
    // still run without audio. Created here (before the eframe loop) and moved
    // into the app; the cpal stream runs on its own realtime callback thread,
    // independent of eframe's repaint cadence.
    let audio = match audio::AudioOutput::new() {
        Ok(a) => {
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

    let force_dmg = opts.force_dmg;
    let force_cgb = opts.force_cgb;

    eframe::run_native(
        TITLE,
        options,
        Box::new(move |_cc| Ok(Box::new(RubcApp::new(machine, save_path, audio, force_dmg, force_cgb)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe run_native failed: {e}"))
}

/// The windowed application state, driven by `eframe`. Holds the emulator, the
/// save path, the audio output, the egui UI ([`Gui`]), the screen texture, and
/// the frame-pacing / periodic-logging timers.
struct RubcApp {
    machine: Option<Machine>,
    save_path: Option<PathBuf>,
    audio: Option<audio::AudioOutput>,
    /// Scratch buffer reused each frame to drain APU samples without realloc.
    audio_scratch: Vec<f32>,
    /// Per-second counter of stereo frames pushed to the device (audio evidence).
    audio_frames_pushed: usize,
    audio_log_start: Instant,
    gui: Gui,
    /// Read-only VRAM snapshot handed to the detached debug viewport each frame
    /// (None until the viewer is first opened). Written under a brief write lock
    /// in `logic`, cloned out under a read lock by the viewport closure.
    vram_snapshot: Arc<RwLock<Option<vramview::VramDebugSnapshot>>>,
    /// File -> Debug toggle, shared with the menu ([`Gui`]) and the detached
    /// viewport so the menu entry and the OS close button stay consistent.
    debug_open: Arc<AtomicBool>,
    /// Render state for the detached VRAM viewer (view mode, toggles, scale,
    /// cached texture, signature). Owned behind an `Arc<RwLock>` so the
    /// `'static` deferred-viewport closure can mutate it -- the closure cannot
    /// borrow `self`.
    vram_view: Arc<RwLock<vramview::VramView>>,
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
    force_dmg: bool,
    force_cgb: bool,
    logo_tex: Option<egui::TextureHandle>,
    error_msg: Option<String>,
}

impl RubcApp {
    fn new(mut machine: Option<Machine>, save_path: Option<PathBuf>, audio: Option<audio::AudioOutput>, force_dmg: bool, force_cgb: bool) -> Self {
        let now = Instant::now();
        let fps_target = Duration::from_micros(FPS_US);
        let debug_open = Arc::new(AtomicBool::new(false));
        if let (Some(m), Some(a)) = (&mut machine, &audio) {
            m.bus.apu.set_sample_rate(a.sample_rate());
        }
        Self {
            machine,
            save_path,
            audio,
            audio_scratch: Vec::new(),
            audio_frames_pushed: 0,
            audio_log_start: now,
            gui: Gui::new(Arc::clone(&debug_open)),
            screen_tex: None,
            fps_target,
            last_frame: now,
            next_deadline: now + fps_target,
            fps_window_start: now,
            frames_this_window: 0,
            last_save: now,
            vram_snapshot: Arc::new(RwLock::new(None)),
            debug_open,
            vram_view: Arc::new(RwLock::new(vramview::VramView::new())),
            force_dmg,
            force_cgb,
            logo_tex: None,
            error_msg: None,
        }
    }

    /// (Re)spawn the detached VRAM debug viewport. Called every frame while the
    /// `File -> Debug` toggle is on; egui keeps the OS window alive as long as
    /// this is called and closes it once we stop. The deferred closure is
    /// `Fn(&mut Ui, ViewportClass) + Send + Sync + 'static`, so it captures only
    /// `Arc` clones of the shared state -- never `&mut self`.
    fn show_vram_viewport(&self, ctx: &egui::Context) {
        let snapshot = Arc::clone(&self.vram_snapshot);
        let view = Arc::clone(&self.vram_view);
        let debug_open = Arc::clone(&self.debug_open);
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of("rubc-vram-debug"),
            egui::ViewportBuilder::default()
                .with_title("rubc \u{2014} VRAM Debug")
                .with_inner_size([580.0, 640.0])
                .with_min_inner_size([320.0, 240.0]),
            move |ui, class| {
                // Platform without native multi-viewport support: the backend
                // embedded us inside a Window instead of a real OS window, so a
                // detached viewer is impossible here -- show a notice and bail.
                if class == egui::ViewportClass::EmbeddedWindow {
                    ui.label("Detached VRAM viewer needs native multi-viewport support.");
                    return;
                }

                // Repaint the detached window continuously so it shows LIVE VRAM
                // as the game runs; its ctx drives independently of the parent.
                ui.ctx().request_repaint();

                // Clone the freshly-written snapshot out and drop the read lock
                // immediately so the emulator loop never blocks on the viewer.
                let snap = snapshot.read().ok().and_then(|g| g.clone());

                egui::CentralPanel::default().show_inside(ui, |ui| match snap {
                    Some(snap) => {
                        if let Ok(mut view) = view.write() {
                            view.viewport_ui(ui, &snap);
                        }
                    }
                    None => {
                        ui.label("Waiting for VRAM snapshot\u{2026}");
                    }
                });

                // OS close button -> un-toggle File -> Debug so the menu and the
                // window stay consistent (the menu re-opens it next click).
                if ui.input(|i| i.viewport().close_requested()) {
                    debug_open.store(false, Ordering::Relaxed);
                }
            },
        );
    }
    fn draw_idle_screen(&mut self, ui: &mut egui::Ui) {
        let logo_bytes = include_bytes!("../assets/logo.png");
        let tex = self.logo_tex.get_or_insert_with(|| {
            let decoder = png::Decoder::new(std::io::Cursor::new(logo_bytes));
            let mut reader = decoder.read_info().unwrap();
            let mut buf = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut buf).unwrap();
            let size = [info.width as _, info.height as _];
            let color_image = match info.color_type {
                png::ColorType::Rgba => {
                    egui::ColorImage::from_rgba_unmultiplied(size, &buf[..info.buffer_size()])
                }
                png::ColorType::Rgb => {
                    egui::ColorImage::from_rgb(size, &buf[..info.buffer_size()])
                }
                _ => panic!("Unsupported logo color type"),
            };
            ui.ctx().load_texture("rubc-logo", color_image, egui::TextureOptions::LINEAR)
        });

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                let size = egui::vec2(tex.size()[0] as f32, tex.size()[1] as f32);
                // Scale down if needed
                let avail = ui.available_size();
                let scale = (avail.x * 0.5 / size.x).min(avail.y * 0.5 / size.y).min(1.0);
                ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), size * scale)));
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Drop a ROM or use File \u{2192} Load ROM")
                        .color(egui::Color32::from_gray(150))
                        .size(16.0)
                );
            });
        });
    }

    fn load_rom_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Game Boy ROM", &["gb", "gbc", "zip"])
            .pick_file()
        {
            self.load_rom_path(&path);
        }
    }

    fn load_rom_path(&mut self, path: &std::path::Path) {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let bytes = if ext == "zip" {
            match std::fs::File::open(path) {
                Ok(file) => {
                    match zip::ZipArchive::new(file) {
                        Ok(mut archive) => {
                            let mut found = None;
                            for i in 0..archive.len() {
                                if let Ok(file) = archive.by_index(i) {
                                    if file.is_file() {
                                        let name = file.name().to_lowercase();
                                        if !name.contains("__macosx/") && (name.ends_with(".gb") || name.ends_with(".gbc")) {
                                            found = Some(i);
                                            break;
                                        }
                                    }
                                }
                            }
                            if let Some(i) = found {
                                let mut file = archive.by_index(i).unwrap();
                                let mut buf = Vec::new();
                                if let Err(e) = std::io::Read::read_to_end(&mut file, &mut buf) {
                                    self.error_msg = Some(format!("Failed to read ROM from zip: {e}"));
                                    return;
                                }
                                buf
                            } else {
                                self.error_msg = Some("No .gb or .gbc file found in zip".to_string());
                                return;
                            }
                        }
                        Err(e) => {
                            self.error_msg = Some(format!("Failed to open zip archive: {e}"));
                            return;
                        }
                    }
                }
                Err(e) => {
                    self.error_msg = Some(format!("Failed to open zip file: {e}"));
                    return;
                }
            }
        } else {
            match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    self.error_msg = Some(format!("Failed to read ROM file: {e}"));
                    return;
                }
            }
        };

        let mut machine = boot_bytes(&bytes, self.force_dmg, self.force_cgb);
        let save_path = sav_path(path.to_str().unwrap_or(""));
        if machine.has_battery() {
            load_save(&mut machine, &save_path);
        }
        if let Some(audio) = &self.audio {
            machine.bus.apu.set_sample_rate(audio.sample_rate());
        }
        self.machine = Some(machine);
        self.save_path = Some(save_path);
        self.last_frame = Instant::now();
        self.next_deadline = self.last_frame + self.fps_target;
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
            if let Some(m) = &mut self.machine {
                m.set_button(Button::Up, i.key_down(egui::Key::ArrowUp));
                m.set_button(Button::Down, i.key_down(egui::Key::ArrowDown));
                m.set_button(Button::Left, i.key_down(egui::Key::ArrowLeft));
                m.set_button(Button::Right, i.key_down(egui::Key::ArrowRight));
                m.set_button(Button::A, i.key_down(egui::Key::X));
                m.set_button(Button::B, i.key_down(egui::Key::Z));
                m.set_button(Button::Start, i.key_down(egui::Key::Enter));
                m.set_button(
                    Button::Select,
                    i.key_down(egui::Key::Backspace) || i.modifiers.shift,
                );
            }
            i.key_pressed(egui::Key::Escape)
        });
        if esc {
            // Clean exit (Esc): flush the save, then ask eframe to close. The
            // window-close button is covered by `on_exit`.
        if let (Some(m), Some(p)) = (&self.machine, &self.save_path) {
            persist_save(m, p);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }

    if let Some(machine) = &mut self.machine {
        // One full Game Boy frame.
        machine.step_frame();

        // Periodic battery-save flush (best-effort, gated inside).
        if self.last_save.elapsed() >= Duration::from_secs(1) {
            if let Some(p) = &self.save_path {
                persist_save(machine, p);
            }
            self.last_save = Instant::now();
        }

        // Feed this frame's APU samples to the audio device. The APU was told to
        // emit at the device's native rate, so push them straight through with
        // no resampling.
        if let Some(audio) = &self.audio {
            self.audio_scratch.clear();
            machine.bus.apu.drain_samples(&mut self.audio_scratch);
            self.audio_frames_pushed += self.audio_scratch.len() / 2;
            audio.push_samples(&self.audio_scratch);
        }

        // Upload the resolved framebuffer as a NEAREST-filtered texture (updated
        // in place after the first frame -- no per-frame allocation).
        let image = framebuffer_color_image(machine);
        match &mut self.screen_tex {
            Some(tex) => tex.set(image, egui::TextureOptions::NEAREST),
            None => {
                self.screen_tex =
                    Some(ctx.load_texture("rubc-screen", image, egui::TextureOptions::NEAREST));
            }
        }

        // While the detached debug viewer is open: capture a fresh read-only
        // VRAM snapshot into the shared lock (brief write), then (re)spawn the
        // deferred viewport so it renders in its own OS window. The immutable
        // machine borrow ends before the closure runs.
        if self.debug_open.load(Ordering::Relaxed) {
            if let Ok(mut guard) = self.vram_snapshot.write() {
                *guard = Some(vramview::VramDebugSnapshot::capture(machine));
            }
            self.show_vram_viewport(ctx);
        }
    } else {
        // If no machine, just sleep a bit to avoid spinning CPU.
        std::thread::sleep(Duration::from_millis(16));
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
        let action = self.gui.ui(ui);
        if let crate::gui::GuiAction::LoadRom = action {
            self.load_rom_dialog();
        }

        let mut clear_error = false;
        if let Some(err) = &self.error_msg {
            let mut open = true;
            egui::Window::new("Error")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(err);
                    if ui.button("OK").clicked() {
                        clear_error = true;
                    }
                });
            if !open {
                clear_error = true;
            }
        }
        if clear_error {
            self.error_msg = None;
        }
        // The game screen: the framebuffer texture, aspect-preserved, centered
        // on a black field, NEAREST-filtered (no blur).
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show_inside(ui, |ui| {
                if let Some(tex) = &self.screen_tex {
                    if self.machine.is_some() {
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
                }
                if self.machine.is_none() {
                    self.draw_idle_screen(ui);
                }
            });

        // Handle drag-and-drop
        if !ui.ctx().input(|i| i.raw.dropped_files.is_empty()) {
            let dropped_files = ui.ctx().input(|i| i.raw.dropped_files.clone());
            if let Some(file) = dropped_files.first() {
                if let Some(path) = &file.path {
                    self.load_rom_path(path);
                }
            }
        }
    }



    fn on_exit(&mut self) {
        // Window-close (or any shutdown) save flush.
        if let (Some(m), Some(p)) = (&self.machine, &self.save_path) {
            persist_save(m, p);
        }
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
