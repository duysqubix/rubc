//! egui UI for the rubc window, driven by `eframe`.
//!
//! [`Gui`] owns the immediate-mode UI: the File menu (Debug / About), the About
//! window, and the embedded live VRAM debug viewer ([`crate::vramview`]). Under
//! the old manual `pixels` + `egui-wgpu` backend this lived behind a `Framework`
//! glue type; with `eframe` the windowing/rendering is handled by the framework
//! and the app simply calls [`Gui::ui`] from `eframe::App::ui`.

use crate::vramview::{VramDebugSnapshot, VramView};

/// The egui UI state for the rubc window.
pub(crate) struct Gui {
    /// Only show the egui "About" window when true.
    window_open: bool,
    /// File -> Debug VRAM viewer.
    vram_view: VramView,
}

impl Gui {
    /// Create a `Gui`.
    pub(crate) fn new() -> Self {
        Self {
            window_open: false,
            vram_view: VramView::new(),
        }
    }

    /// Hand this frame's read-only VRAM snapshot to the debug viewer. Called
    /// before [`Gui::ui`], so the viewer never borrows the `Machine` across the
    /// egui closure.
    pub(crate) fn set_vram_snapshot(&mut self, snapshot: VramDebugSnapshot) {
        self.vram_view.set_snapshot(snapshot);
    }

    /// Whether the debug window is currently open (lets the caller skip the
    /// per-frame snapshot copy when it is closed).
    pub(crate) fn debug_open(&self) -> bool {
        self.vram_view.open
    }

    /// Create the UI using egui.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        // Cheap (Arc) clone so the floating windows below can borrow the egui
        // context after the menubar panel has finished borrowing `ui`.
        let ctx = ui.ctx().clone();

        egui::Panel::top("menubar_container").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Debug...").clicked() {
                        self.vram_view.toggle();
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
                ui.label("File -> Debug opens the live VRAM viewer.");
            });

        // Live VRAM debug viewer (File -> Debug).
        self.vram_view.ui(&ctx);
    }
}
