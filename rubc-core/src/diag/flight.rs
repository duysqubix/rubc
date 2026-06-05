//! Flight recorder: a bounded ring buffer holding the last N CPU M-cycles.
//!
//! The recorder is the centrepiece of AFK debugging. It records one compact
//! [`FlightRecord`] per CPU M-cycle, *after* the bus M-cycle has completed, so
//! it observes final state without perturbing timing. On panic or a fatal
//! anomaly it is dumped to `flight.bin` (raw, chronological) plus
//! `flight.tail.txt` (a human/agent-readable decode of the most recent entries).
//!
//! Records are deliberately boring: fixed-size, `Copy`, no heap, no strings, no
//! enum payloads. The on-disk format is defined solely by `write_le` (a safe
//! field-by-field little-endian encoder — no transmute, no `#[repr(C)]`).
//! `record()` runs millions of times per second.

use std::io::Write;
use std::path::Path;

/// What kind of bus access a record captured.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BusKind {
    #[default]
    Idle = 0,
    Read = 1,
    Write = 2,
}

impl BusKind {
    fn as_str(self) -> &'static str {
        match self {
            BusKind::Idle => "idle",
            BusKind::Read => "rd",
            BusKind::Write => "wr",
        }
    }
}

/// Coarse CPU execution tag at the moment of the record.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExecTag {
    #[default]
    Boundary = 0,
    Fetch = 1,
    Execute = 2,
    Halt = 3,
    StopSpeedSwitch = 4,
    InterruptDispatch = 5,
}

/// One CPU M-cycle of recorded state. Fixed-size, `Copy`, no allocation.
///
/// The on-disk format is defined SOLELY by [`FlightRecord::write_le`]
/// (38 bytes, little-endian, field by field). Nothing relies on the in-memory
/// layout, so there is no `#[repr(C)]` and no transmute anywhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlightRecord {
    pub mcycle: u64,

    pub pc: u16,
    pub sp: u16,
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub bus_addr: u16,

    pub opcode: u8,
    pub exec: u8,
    pub phase: u8,
    pub bus_kind: u8,
    pub bus_value: u8,
    pub ie: u8,
    pub if_: u8,
    pub ly: u8,

    pub ppu_mode: u8,
    pub speed: u8,
    pub mem_region: u8,
    pub flags: u8,
    pub div_hi: u8,
    pub tima: u8,
    pub dma: u8,
    pub reserved: u8,
}

impl FlightRecord {
    /// Byte length of one safely-encoded record (sum of all field widths).
    /// 8 (u64) + 7*2 (u16) + 16*1 (u8) = 38 bytes.
    pub const ENCODED_LEN: usize = 8 + 7 * 2 + 16;

    /// Serialize this record into `buf` as little-endian, field by field.
    /// Pure safe Rust: no transmute, no raw pointers.
    pub fn write_le(&self, buf: &mut [u8; Self::ENCODED_LEN]) {
        let mut o = 0usize;
        macro_rules! put {
            ($v:expr) => {{
                let b = $v.to_le_bytes();
                buf[o..o + b.len()].copy_from_slice(&b);
                o += b.len();
            }};
        }
        put!(self.mcycle);
        put!(self.pc);
        put!(self.sp);
        put!(self.af);
        put!(self.bc);
        put!(self.de);
        put!(self.hl);
        put!(self.bus_addr);
        put!(self.opcode);
        put!(self.exec);
        put!(self.phase);
        put!(self.bus_kind);
        put!(self.bus_value);
        put!(self.ie);
        put!(self.if_);
        put!(self.ly);
        put!(self.ppu_mode);
        put!(self.speed);
        put!(self.mem_region);
        put!(self.flags);
        put!(self.div_hi);
        put!(self.tima);
        put!(self.dma);
        put!(self.reserved);
        debug_assert_eq!(o, Self::ENCODED_LEN);
    }

    /// Render a single decoded line for `flight.tail.txt`.
    pub(crate) fn decode_line(&self) -> String {
        let kind = match self.bus_kind {
            1 => BusKind::Read,
            2 => BusKind::Write,
            _ => BusKind::Idle,
        }
        .as_str();
        format!(
            "m={:>10} pc={:04X} op={:02X} ph={} {:<4} af={:04X} bc={:04X} de={:04X} hl={:04X} sp={:04X} \
bus={}@{:04X}={:02X} ie={:02X} if={:02X} ly={:>3} mode={} spd={} div={:02X} tima={:02X} dma={:02X}",
            self.mcycle,
            self.pc,
            self.opcode,
            self.phase,
            self.exec_str(),
            self.af,
            self.bc,
            self.de,
            self.hl,
            self.sp,
            kind,
            self.bus_addr,
            self.bus_value,
            self.ie,
            self.if_,
            self.ly,
            self.ppu_mode,
            self.speed,
            self.div_hi,
            self.tima,
            self.dma,
        )
    }

    fn exec_str(&self) -> &'static str {
        match self.exec {
            0 => "bnd",
            1 => "fetch",
            2 => "exec",
            3 => "halt",
            4 => "stop",
            5 => "intr",
            _ => "?",
        }
    }
}

/// Bounded ring buffer of [`FlightRecord`]s. Single-writer (the emulator
/// thread). Capacity is a power of two so the index mask is a single AND.
pub struct FlightRecorder {
    enabled: bool,
    head: u64,
    mask: usize,
    buf: Box<[FlightRecord]>,
}

impl FlightRecorder {
    /// Create a recorder with `capacity` slots (rounded up to a power of two,
    /// minimum 2, clamped so the allocation cannot overflow). `enabled = false`
    /// makes [`record`](Self::record) a no-op.
    pub fn new(capacity: usize, enabled: bool) -> Self {
        // Hard cap on slot count: 2^22 (~4.2M slots, ~170 MiB in memory at the
        // ~40-byte in-memory record). The documented default is 2^20 (~40 MiB);
        // this cap is a defensive ceiling so an absurd `capacity` can neither
        // overflow `next_power_of_two()` nor OOM the process.
        const MAX_CAP: usize = 1 << 22;
        let requested = capacity.clamp(2, MAX_CAP);
        let cap = requested.next_power_of_two();
        Self {
            enabled,
            head: 0,
            mask: cap - 1,
            buf: vec![FlightRecord::default(); cap].into_boxed_slice(),
        }
    }

    /// Total slot capacity.
    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    /// Number of records written so far (saturating at capacity once wrapped).
    pub fn len(&self) -> usize {
        (self.head as usize).min(self.capacity())
    }

    pub fn is_empty(&self) -> bool {
        self.head == 0
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Record one entry. Hot path: a disabled recorder returns immediately.
    #[inline(always)]
    pub fn record(&mut self, rec: FlightRecord) {
        if !self.enabled {
            return;
        }
        let idx = (self.head as usize) & self.mask;
        self.buf[idx] = rec;
        self.head = self.head.wrapping_add(1);
    }

    /// Return the recorded entries in chronological (oldest-first) order.
    pub fn snapshot_chronological(&self) -> Vec<FlightRecord> {
        let len = self.len();
        if len == 0 {
            return Vec::new();
        }
        let cap = self.capacity();
        let mut out = Vec::with_capacity(len);
        // Oldest entry index: if we've wrapped, it's head % cap; else 0.
        let start = if self.head as usize >= cap {
            (self.head as usize) & self.mask
        } else {
            0
        };
        for i in 0..len {
            out.push(self.buf[(start + i) & self.mask]);
        }
        out
    }

    /// Dump the recorder: `flight.bin` (raw records, chronological) and
    /// `flight.tail.txt` (decoded last `tail_n` entries).
    pub fn dump(&self, dir: &Path, tail_n: usize) -> std::io::Result<()> {
        let records = self.snapshot_chronological();

        // Binary dump: 16-byte header + safely-encoded records (little-endian,
        // field by field; NO unsafe transmute).
        let bin_path = dir.join("flight.bin");
        let mut bin = std::io::BufWriter::new(std::fs::File::create(&bin_path)?);
        bin.write_all(b"RUBCFR01")?; // magic + version
        bin.write_all(&(records.len() as u32).to_le_bytes())?;
        bin.write_all(&(FlightRecord::ENCODED_LEN as u32).to_le_bytes())?;
        let mut scratch = [0u8; FlightRecord::ENCODED_LEN];
        for r in &records {
            r.write_le(&mut scratch);
            bin.write_all(&scratch)?;
        }
        bin.flush()?;

        // Decoded tail for humans/agents.
        let tail_path = dir.join("flight.tail.txt");
        let mut tail = std::io::BufWriter::new(std::fs::File::create(&tail_path)?);
        let start = records.len().saturating_sub(tail_n);
        for r in &records[start..] {
            writeln!(tail, "{}", r.decode_line())?;
        }
        tail.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(mcycle: u64, pc: u16) -> FlightRecord {
        FlightRecord {
            mcycle,
            pc,
            ..Default::default()
        }
    }

    #[test]
    fn disabled_recorder_is_noop() {
        let mut fr = FlightRecorder::new(4, false);
        for i in 0..10 {
            fr.record(rec(i, i as u16));
        }
        assert!(fr.is_empty());
        assert_eq!(fr.len(), 0);
        assert_eq!(fr.snapshot_chronological().len(), 0);
    }

    #[test]
    fn capacity_rounds_to_power_of_two() {
        assert_eq!(FlightRecorder::new(3, true).capacity(), 4);
        assert_eq!(FlightRecorder::new(4, true).capacity(), 4);
        assert_eq!(FlightRecorder::new(5, true).capacity(), 8);
        assert_eq!(FlightRecorder::new(0, true).capacity(), 2);
    }

    #[test]
    fn capacity_clamps_absurd_input_without_overflow() {
        // usize::MAX would overflow next_power_of_two(); must clamp, not panic.
        let fr = FlightRecorder::new(usize::MAX, true);
        assert_eq!(fr.capacity(), 1 << 22);
    }

    #[test]
    fn write_le_is_exact_38_bytes_little_endian() {
        // Golden test: every field at its byte offset, little-endian.
        let r = FlightRecord {
            mcycle: 0x0102_0304_0506_0708,
            pc: 0x1122,
            sp: 0x3344,
            af: 0x5566,
            bc: 0x7788,
            de: 0x99AA,
            hl: 0xBBCC,
            bus_addr: 0xDDEE,
            opcode: 0x10,
            exec: 0x11,
            phase: 0x12,
            bus_kind: 0x13,
            bus_value: 0x14,
            ie: 0x15,
            if_: 0x16,
            ly: 0x17,
            ppu_mode: 0x18,
            speed: 0x19,
            mem_region: 0x1A,
            flags: 0x1B,
            div_hi: 0x1C,
            tima: 0x1D,
            dma: 0x1E,
            reserved: 0x1F,
        };
        let mut buf = [0u8; FlightRecord::ENCODED_LEN];
        r.write_le(&mut buf);
        assert_eq!(FlightRecord::ENCODED_LEN, 38);
        // mcycle (u64 LE)
        assert_eq!(
            &buf[0..8],
            &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
        // pc (u16 LE) at offset 8
        assert_eq!(&buf[8..10], &[0x22, 0x11]);
        // bus_addr (u16 LE) at offset 20 (8 + 6*2)
        assert_eq!(&buf[20..22], &[0xEE, 0xDD]);
        // first u8 (opcode) at offset 22 (8 + 7*2)
        assert_eq!(buf[22], 0x10);
        // last u8 (reserved) at offset 37
        assert_eq!(buf[37], 0x1F);
    }

    #[test]
    fn wraps_and_keeps_last_n() {
        // S2: capacity 4, record 6 entries -> keeps mcycle 2,3,4,5.
        let mut fr = FlightRecorder::new(4, true);
        for i in 0..6 {
            fr.record(rec(i, i as u16));
        }
        assert_eq!(fr.len(), 4);
        let snap = fr.snapshot_chronological();
        let mcycles: Vec<u64> = snap.iter().map(|r| r.mcycle).collect();
        assert_eq!(
            mcycles,
            vec![2, 3, 4, 5],
            "ring must keep the last N in order"
        );
    }

    #[test]
    fn partial_fill_is_chronological() {
        let mut fr = FlightRecorder::new(8, true);
        for i in 0..3 {
            fr.record(rec(i, i as u16));
        }
        let mcycles: Vec<u64> = fr
            .snapshot_chronological()
            .iter()
            .map(|r| r.mcycle)
            .collect();
        assert_eq!(mcycles, vec![0, 1, 2]);
    }

    #[test]
    fn dump_writes_bin_and_tail() {
        let dir = std::env::temp_dir().join(format!("rubc_fr_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut fr = FlightRecorder::new(4, true);
        for i in 0..6 {
            fr.record(rec(i, 0x100 + i as u16));
        }
        fr.dump(&dir, 2).unwrap();

        let bin = std::fs::read(dir.join("flight.bin")).unwrap();
        assert_eq!(&bin[0..8], b"RUBCFR01");
        let count = u32::from_le_bytes(bin[8..12].try_into().unwrap());
        assert_eq!(count, 4);
        let enc_len = u32::from_le_bytes(bin[12..16].try_into().unwrap());
        assert_eq!(enc_len as usize, FlightRecord::ENCODED_LEN);
        // header(16) + 4 records * ENCODED_LEN
        assert_eq!(bin.len(), 16 + 4 * FlightRecord::ENCODED_LEN);

        let tail = std::fs::read_to_string(dir.join("flight.tail.txt")).unwrap();
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), 2, "tail_n=2 -> 2 lines");
        assert!(lines[0].contains("m=         4"));
        assert!(lines[1].contains("m=         5"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
