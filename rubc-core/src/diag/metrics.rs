//! Lightweight emulation metrics.
//!
//! Plain `u64` counters (no atomics — single emulator thread), incremented
//! AFTER state transitions so they never perturb timing. Dumped to
//! `metrics.json` on exit / anomaly / stuck. Useful to answer "is the CPU
//! spinning in HALT?", "which opcode dominates?", "are interrupts firing?"
//! from an AFK run with no live repro.

/// Counters covering CPU, interrupts, PPU, DMA, and memory access by region.
/// Per-opcode arrays are boxed to keep `Metrics` cheap to move.
pub struct Metrics {
    pub cpu_mcycles: u64,
    pub cpu_tcycles: u64,
    pub frames: u64,

    pub opcode: Box<[u64; 256]>,
    pub cb_opcode: Box<[u64; 256]>,

    pub interrupts_requested: [u64; 5],
    pub interrupts_serviced: [u64; 5],

    pub illegal_opcodes: u64,
    pub stuck_events: u64,
    pub halt_mcycles: u64,
    pub stop_mcycles: u64,

    pub ppu_frames_rendered: u64,
    pub ppu_mode_cycles: [u64; 4],

    pub oam_dma_beats: u64,
    pub hdma_bytes: u64,

    pub mem_reads_by_region: [u64; 16],
    pub mem_writes_by_region: [u64; 16],
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            cpu_mcycles: 0,
            cpu_tcycles: 0,
            frames: 0,
            opcode: Box::new([0; 256]),
            cb_opcode: Box::new([0; 256]),
            interrupts_requested: [0; 5],
            interrupts_serviced: [0; 5],
            illegal_opcodes: 0,
            stuck_events: 0,
            halt_mcycles: 0,
            stop_mcycles: 0,
            ppu_frames_rendered: 0,
            ppu_mode_cycles: [0; 4],
            oam_dma_beats: 0,
            hdma_bytes: 0,
            mem_reads_by_region: [0; 16],
            mem_writes_by_region: [0; 16],
        }
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Count one executed main opcode (and advance the M/T-cycle totals).
    #[inline]
    pub fn count_opcode(&mut self, op: u8) {
        self.opcode[op as usize] += 1;
    }

    /// Count one executed CB-prefixed opcode.
    #[inline]
    pub fn count_cb_opcode(&mut self, op: u8) {
        self.cb_opcode[op as usize] += 1;
    }

    /// Serialize to `metrics.json` in `dir`.
    pub fn write_json(&self, dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(dir.join("metrics.json"))?;
        f.write_all(self.to_json().as_bytes())
    }

    /// Render the metrics as a JSON object string. Sparse arrays (opcode counts)
    /// are emitted as `{index: count}` maps to keep the file small and readable.
    pub fn to_json(&self) -> String {
        let mut s = String::with_capacity(512);
        s.push_str("{\n  \"schema\": \"rubc.diag.metrics.v1\",\n");
        s.push_str(&format!("  \"cpu_mcycles\": {},\n", self.cpu_mcycles));
        s.push_str(&format!("  \"cpu_tcycles\": {},\n", self.cpu_tcycles));
        s.push_str(&format!("  \"frames\": {},\n", self.frames));
        s.push_str(&format!(
            "  \"illegal_opcodes\": {},\n",
            self.illegal_opcodes
        ));
        s.push_str(&format!("  \"stuck_events\": {},\n", self.stuck_events));
        s.push_str(&format!("  \"halt_mcycles\": {},\n", self.halt_mcycles));
        s.push_str(&format!("  \"stop_mcycles\": {},\n", self.stop_mcycles));
        s.push_str(&format!(
            "  \"ppu_frames_rendered\": {},\n",
            self.ppu_frames_rendered
        ));
        s.push_str(&format!("  \"oam_dma_beats\": {},\n", self.oam_dma_beats));
        s.push_str(&format!("  \"hdma_bytes\": {},\n", self.hdma_bytes));
        s.push_str(&format!(
            "  \"interrupts_requested\": {},\n",
            arr_json(&self.interrupts_requested)
        ));
        s.push_str(&format!(
            "  \"interrupts_serviced\": {},\n",
            arr_json(&self.interrupts_serviced)
        ));
        s.push_str(&format!(
            "  \"ppu_mode_cycles\": {},\n",
            arr_json(&self.ppu_mode_cycles)
        ));
        s.push_str(&format!(
            "  \"mem_reads_by_region\": {},\n",
            arr_json(&self.mem_reads_by_region)
        ));
        s.push_str(&format!(
            "  \"mem_writes_by_region\": {},\n",
            arr_json(&self.mem_writes_by_region)
        ));
        s.push_str(&format!(
            "  \"opcode\": {},\n",
            sparse_json(self.opcode.as_ref())
        ));
        s.push_str(&format!(
            "  \"cb_opcode\": {}\n",
            sparse_json(self.cb_opcode.as_ref())
        ));
        s.push('}');
        s
    }
}

/// `[a, b, c]` for a slice of counters.
fn arr_json(a: &[u64]) -> String {
    let body = a
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

/// Sparse `{ "index": count, ... }` for a 256-entry counter array (omit zeros).
fn sparse_json(a: &[u64]) -> String {
    let body = a
        .iter()
        .enumerate()
        .filter(|(_, &v)| v != 0)
        .map(|(i, v)| format!("\"{i}\":{v}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_serializes() {
        // S3: record opcodes/cycles -> metrics.json parses, counts correct.
        let mut m = Metrics::new();
        m.cpu_mcycles = 1000;
        m.frames = 3;
        m.count_opcode(0x18); // JR e8
        m.count_opcode(0x18);
        m.count_opcode(0x00); // NOP
        m.count_cb_opcode(0x7C); // BIT 7,H
        m.interrupts_serviced[2] = 5; // timer

        let json = m.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(v["cpu_mcycles"], 1000);
        assert_eq!(v["frames"], 3);
        // 0x18 == 24, counted twice.
        assert_eq!(v["opcode"]["24"], 2);
        assert_eq!(v["opcode"]["0"], 1);
        assert_eq!(v["cb_opcode"]["124"], 1); // 0x7C
        assert_eq!(v["interrupts_serviced"][2], 5);
        // Zero opcodes omitted from the sparse map.
        assert!(v["opcode"].get("1").is_none());
    }

    #[test]
    fn write_json_creates_file() {
        let dir = std::env::temp_dir().join(format!("rubc_metrics_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = Metrics::new();
        m.write_json(&dir).unwrap();
        let content = std::fs::read_to_string(dir.join("metrics.json")).unwrap();
        let _: serde_json::Value = serde_json::from_str(&content).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
