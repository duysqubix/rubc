//! PPU phase sample trace (ADR 0001 stage 3).
//!
//! Records, per BG-fetch sub-step during mode 3, the dot and the register values
//! the fetcher actually sampled. This is the instrument the sub-dot scheduler
//! rearchitecture is calibrated against: the decisive `m3_scy_change` check is
//! "at tile 0x42 on LY 0, the LOW-byte fetch sampled SCY=2 while the HIGH-byte
//! fetch sampled SCY=3". Without a per-sample record that question is unanswerable.
//!
//! Compiled in ONLY under the `trace` feature. When disabled the type, the PPU
//! field, and every push site vanish — zero cost, zero behavior change. The PPU
//! only ever APPENDS to this; it never reads it back to make a decision, so it
//! cannot perturb emulation (diagnostics observe, never participate).

/// The BG-fetch sub-step (or pixel emission) a sample was taken at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PpuPhase {
    /// Tile-number fetch (fetcher step 0): map select + SCX + SCY latched here.
    TileNo,
    /// Low bitplane fetch (step 1): on DMG, SCY is re-sampled here.
    TileDataLow,
    /// High bitplane fetch (step 2).
    TileDataHigh,
    /// 8 pixels pushed to the BG FIFO (step 3).
    Push,
    /// A pixel emitted to the framebuffer (palette applied here).
    Emit,
}

/// Hardware BG fetch stage predicted by the inert sidecar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgFetchStage {
    /// B: tile-number / BG-map fetch.
    TileNo,
    /// 0: low bitplane fetch.
    DataLow,
    /// 1: high bitplane fetch.
    DataHigh,
    /// s: push/sleep slot after bitplanes are fetched.
    Sleep,
}

/// One sidecar prediction for a BG-fetch stage.
///
/// The sidecar is deliberately inert: it is derived from the append-only trace
/// after the fact and is never read by the renderer. `actual_*` records rubc's
/// current fetch geometry; `predicted_*` records SameBoy/hardware geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BgFetchSidecarEvent {
    pub ly: u8,
    pub x: u8,
    pub tile: u8,
    pub stage: BgFetchStage,
    pub actual_phase: PpuPhase,
    pub actual_t1_norm_dot: i32,
    pub predicted_t1_norm_dot: i32,
    pub predicted_t2_norm_dot: i32,
    pub delta_dots: i32,
    pub scy: u8,
    pub scx: u8,
    pub lcdc: u8,
}

/// One sidecar prediction for a CPU write that can affect BG fetching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BgFetchSidecarWrite {
    pub ly: u8,
    pub addr: u16,
    pub value: u8,
    pub actual_norm_dot: i32,
    pub predicted_write_start_norm_dot: i32,
    pub predicted_visible_norm_dot: i32,
}

/// One register-sample record taken during mode 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PpuSample {
    /// Dot within the scanline (0..456) when the sample was taken.
    pub line_dot: u32,
    /// Total PPU dots since power-on -- a monotonic clock for absolute ordering
    /// of this sample vs CPU writes (ADR 0001 stage 5 ordering proof).
    pub dot_ticks: u64,
    /// Dots since mode-3 entry.
    pub drawing_dots: u32,
    /// Current scanline.
    pub ly: u8,
    /// The fetch sub-step (or emission) this sample belongs to.
    pub phase: PpuPhase,
    /// Internal fetcher X (tile column), or `lcd_x` for `Emit`.
    pub x: u8,
    /// Tile id in play at this sample (the BG-map byte), 0 for `Emit`.
    pub tile: u8,
    /// The SCY value the PPU sampled at this point.
    pub scy: u8,
    /// The SCX value the PPU sampled at this point.
    pub scx: u8,
    /// The LCDC value the PPU sampled at this point.
    pub lcdc: u8,
}

/// A growable log of [`PpuSample`]s for the current run. Cleared per frame by the
/// caller when it wants a single-frame window; otherwise it accumulates.
/// A CPU register write observed at a PPU dot (ADR 0001 stage 5 observation):
/// the dot/line the write landed on, and the new value -- so the calibration can
/// see where a mid-mode-3 SCY/LCDC write falls relative to the fetch steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PpuWrite {
    pub line_dot: u32,
    /// Total PPU dots since power-on at the moment of the write (monotonic clock
    /// for absolute ordering vs PPU samples).
    pub dot_ticks: u64,
    pub drawing_dots: u32,
    pub ly: u8,
    pub mode: u8,
    /// IO register address (e.g. 0xFF42 for SCY).
    pub addr: u16,
    pub value: u8,
}

#[derive(Clone, Debug, Default)]
pub struct PpuPhaseTrace {
    samples: Vec<PpuSample>,
    writes: Vec<PpuWrite>,
    /// When `Some(ly)`, only samples on that scanline are recorded — keeps the
    /// trace tiny when chasing a specific line (e.g. LY 0 for m3_scy_change).
    filter_ly: Option<u8>,
}

impl PpuPhaseTrace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict recording to a single scanline (or `None` for all lines).
    pub fn set_line_filter(&mut self, ly: Option<u8>) {
        self.filter_ly = ly;
    }

    /// Record one sample, honoring the line filter.
    pub fn push(&mut self, s: PpuSample) {
        if let Some(ly) = self.filter_ly {
            if s.ly != ly {
                return;
            }
        }
        self.samples.push(s);
    }

    /// Drop all recorded samples.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.writes.clear();
    }

    /// Record one CPU register write, honoring the line filter.
    pub fn push_write(&mut self, w: PpuWrite) {
        if let Some(ly) = self.filter_ly {
            if w.ly != ly {
                return;
            }
        }
        self.writes.push(w);
    }

    /// All recorded CPU writes in order.
    pub fn writes(&self) -> &[PpuWrite] {
        &self.writes
    }

    /// All recorded samples in order.
    pub fn samples(&self) -> &[PpuSample] {
        &self.samples
    }

    /// The samples for one fetch of a given tile id on a given scanline, in
    /// phase order — the lens the m3_scy_change calibration check uses.
    pub fn samples_for(&self, ly: u8, tile: u8) -> Vec<PpuSample> {
        self.samples
            .iter()
            .copied()
            .filter(|s| s.ly == ly && s.tile == tile)
            .collect()
    }

    /// Predict the hardware/SameBoy BG-fetch schedule for recorded scanlines.
    ///
    /// Normalisation uses rubc's first post-dummy TileNo sample as zero, matching
    /// the Stage-B wall note (`rubc +11` vs `SameBoy +22` for the decisive SCY
    /// HIGH_T1 sample). The constants are the measured Wall-1 geometry profile:
    /// rubc samples TileNo 12 dots early, LOW 10 dots early, and HIGH 11 dots
    /// early. This models where hardware would sample without moving any pixels.
    pub fn bg_fetch_sidecar_events(&self) -> Vec<BgFetchSidecarEvent> {
        let mut out = Vec::new();
        for sample in &self.samples {
            let Some(anchor_dot) = self.post_dummy_tile_no_anchor(sample.ly) else {
                continue;
            };
            let actual = sample.line_dot as i32 - anchor_dot as i32;
            let Some((stage, delta)) = self.bg_fetch_sidecar_stage_delta(sample, actual) else {
                continue;
            };
            let predicted = actual + delta;
            out.push(BgFetchSidecarEvent {
                ly: sample.ly,
                x: sample.x,
                tile: sample.tile,
                stage,
                actual_phase: sample.phase,
                actual_t1_norm_dot: actual,
                predicted_t1_norm_dot: predicted,
                predicted_t2_norm_dot: predicted + 1,
                delta_dots: delta,
                scy: sample.scy,
                scx: sample.scx,
                lcdc: sample.lcdc,
            });
        }
        out
    }

    /// Predict SameBoy write timing for BG-fetch-visible writes.
    ///
    /// For the current Stage-2 contract this carries the SCY `ReadNew` geometry:
    /// the observed rubc FF42<-3 write at actual +11 corresponds to SameBoy
    /// `write_m` start +14 and internal visibility +17. Other writes are carried
    /// through unchanged until a later stage consumes them.
    pub fn bg_fetch_sidecar_writes(&self) -> Vec<BgFetchSidecarWrite> {
        self.writes
            .iter()
            .filter_map(|w| Some((w, self.post_dummy_tile_no_anchor(w.ly)?)))
            .map(|(w, anchor_dot)| {
                let actual = w.line_dot as i32 - anchor_dot as i32;
                let (start_delta, visible_delta) = if w.addr == 0xFF42 { (3, 6) } else { (0, 0) };
                BgFetchSidecarWrite {
                    ly: w.ly,
                    addr: w.addr,
                    value: w.value,
                    actual_norm_dot: actual,
                    predicted_write_start_norm_dot: actual + start_delta,
                    predicted_visible_norm_dot: actual + visible_delta,
                }
            })
            .collect()
    }

    fn post_dummy_tile_no_anchor(&self, ly: u8) -> Option<u32> {
        self.samples
            .iter()
            .filter(|s| s.ly == ly && s.phase == PpuPhase::TileNo)
            .nth(1)
            .map(|s| s.line_dot)
    }
}

impl PpuPhaseTrace {
    fn bg_fetch_sidecar_stage_delta(
        &self,
        sample: &PpuSample,
        actual_norm_dot: i32,
    ) -> Option<(BgFetchStage, i32)> {
        let line_has_scy_writes = self
            .writes
            .iter()
            .any(|w| w.ly == sample.ly && w.addr == 0xFF42);
        let line_has_lcdc_writes = self
            .writes
            .iter()
            .any(|w| w.ly == sample.ly && w.addr == 0xFF40);
        match sample.phase {
            PpuPhase::TileNo => Some((
                BgFetchStage::TileNo,
                if line_has_lcdc_writes {
                    3
                } else if line_has_scy_writes {
                    12
                } else if actual_norm_dot > 16 {
                    3
                } else {
                    9
                },
            )),
            PpuPhase::TileDataLow => Some((BgFetchStage::DataLow, 10)),
            PpuPhase::TileDataHigh => Some((BgFetchStage::DataHigh, 11)),
            PpuPhase::Push => Some((BgFetchStage::Sleep, 11)),
            PpuPhase::Emit => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ly: u8, phase: PpuPhase, tile: u8, scy: u8) -> PpuSample {
        PpuSample {
            line_dot: 0,
            dot_ticks: 0,
            drawing_dots: 0,
            ly,
            phase,
            x: 0,
            tile,
            scy,
            scx: 0,
            lcdc: 0,
        }
    }

    #[test]
    fn line_filter_drops_other_lines() {
        let mut t = PpuPhaseTrace::new();
        t.set_line_filter(Some(0));
        t.push(sample(0, PpuPhase::TileNo, 0x42, 2));
        t.push(sample(5, PpuPhase::TileNo, 0x42, 2)); // filtered out
        assert_eq!(t.samples().len(), 1);
        assert_eq!(t.samples()[0].ly, 0);
    }

    #[test]
    fn samples_for_tile_returns_phase_sequence() {
        let mut t = PpuPhaseTrace::new();
        t.push(sample(0, PpuPhase::TileNo, 0x42, 2));
        t.push(sample(0, PpuPhase::TileDataLow, 0x42, 2));
        t.push(sample(0, PpuPhase::TileDataHigh, 0x42, 3));
        t.push(sample(0, PpuPhase::TileNo, 0x99, 4)); // different tile
        let seq = t.samples_for(0, 0x42);
        assert_eq!(seq.len(), 3);
        assert_eq!(seq[0].phase, PpuPhase::TileNo);
        assert_eq!(seq[1].scy, 2);
        assert_eq!(seq[2].scy, 3);
    }
}
