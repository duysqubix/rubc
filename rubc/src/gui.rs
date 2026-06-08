//! egui UI for the rubc window, driven by `eframe`.
//!
//! [`Gui`] owns the immediate-mode UI: the File menu (Debug / About) and the
//! embedded "About" window. The live VRAM debug viewer ([`crate::vramview`]) is
//! no longer embedded here -- it now lives in its OWN detachable OS window, a
//! deferred multi-viewport spawned from `RubcApp::logic` (which holds the egui
//! `Context`). The File -> Debug menu entry just flips the shared
//! [`std::sync::atomic::AtomicBool`] that gates that viewport on/off, keeping
//! the menu and the OS close button in sync.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(crate) enum GuiAction {
    None,
    LoadRom,
}

/// The egui UI state for the rubc window.
pub(crate) struct Gui {
    /// Only show the egui "About" window when true.
    window_open: bool,
    /// Shared File -> Debug toggle for the detached VRAM viewport. Flipped by
    /// the menu; also cleared by the viewport's OS close button (in `logic`).
    debug_open: Arc<AtomicBool>,
}

impl Gui {
    /// Create a `Gui`, sharing the detached-debug-viewport toggle with the app.
    pub(crate) fn new(debug_open: Arc<AtomicBool>) -> Self {
        Self {
            window_open: false,
            debug_open,
        }
    }

    /// Create the UI using egui.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) -> GuiAction {
        let mut action = GuiAction::None;
        // Cheap (Arc) clone so the floating window below can borrow the egui
        // context after the menubar panel has finished borrowing `ui`.
        let ctx = ui.ctx().clone();

        egui::Panel::top("menubar_container").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load ROM...").clicked() {
                        action = GuiAction::LoadRom;
                        ui.close();
                    }
                    if ui.button("Debug...").clicked() {
                        // Toggle the detached VRAM viewport. `logic` spawns the
                        // deferred viewport while this is true and drops it when
                        // false; the OS close button flips it back off.
                        self.debug_open.fetch_xor(true, Ordering::Relaxed);
                        ui.close();
                    }
                    if ui.button("About...").clicked() {
                        self.window_open = true;
                        ui.close();
                    }
                });
            });
        });

        egui::Window::new("About rubc")
            .open(&mut self.window_open)
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                ui.label("rubc -- a safe-Rust Game Boy (DMG/CGB) emulator.");
                ui.label("File -> Debug opens the live VRAM viewer in its own window.");
            });

        action
    }
}
