//! Deterministic state hashing for divergence detection.
//!
//! A cheap rolling FNV-1a 64 hash over the canonical machine state lets an AFK
//! agent compare rubc against a reference emulator and bisect to the frame /
//! M-cycle where they first diverge, WITHOUT a full instruction trace.
//!
//! Pure-Rust, no allocation in the hot path, observe-only.

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One-shot FNV-1a 64 of a byte slice.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Streaming FNV-1a 64 hasher. Feed it canonical state region by region
/// (cpu, timer, ppu, wram, hram, vram, oam, io, cart) then `finish()`.
#[derive(Clone, Copy, Debug)]
pub struct StateHasher {
    h: u64,
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHasher {
    pub fn new() -> Self {
        Self { h: FNV_OFFSET }
    }

    /// Mix a byte slice into the running hash.
    #[inline]
    pub fn write(&mut self, bytes: &[u8]) -> &mut Self {
        for &b in bytes {
            self.h ^= b as u64;
            self.h = self.h.wrapping_mul(FNV_PRIME);
        }
        self
    }

    /// Convenience: mix a single byte.
    #[inline]
    pub fn write_u8(&mut self, b: u8) -> &mut Self {
        self.h ^= b as u64;
        self.h = self.h.wrapping_mul(FNV_PRIME);
        self
    }

    /// Convenience: mix a u16 (little-endian).
    #[inline]
    pub fn write_u16(&mut self, v: u16) -> &mut Self {
        self.write(&v.to_le_bytes())
    }

    /// Final 64-bit hash.
    pub fn finish(&self) -> u64 {
        self.h
    }
}

/// Writes the per-sample hash log `hash.csv`.
///
/// One row per sample (per frame by default, or every-K M-cycles for
/// bisection): `frame,mcycle,tcycle,pc,ly,ppu_mode,hash`.
pub struct HashCsv {
    file: std::fs::File,
}

impl HashCsv {
    /// Create `hash.csv` in `dir` and write the header row.
    pub fn create(dir: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Write;
        let mut file = std::fs::File::create(dir.join("hash.csv"))?;
        writeln!(file, "frame,mcycle,tcycle,pc,ly,ppu_mode,hash")?;
        Ok(Self { file })
    }

    /// Append one sample row.
    #[allow(clippy::too_many_arguments)]
    pub fn row(
        &mut self,
        frame: u64,
        mcycle: u64,
        tcycle: u64,
        pc: u16,
        ly: u8,
        ppu_mode: u8,
        hash: u64,
    ) -> std::io::Result<()> {
        use std::io::Write;
        writeln!(
            self.file,
            "{frame},{mcycle},{tcycle},{pc:04X},{ly},{ppu_mode},{hash:016x}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_known_vectors() {
        // FNV-1a 64 reference values.
        assert_eq!(fnv1a64(b""), FNV_OFFSET);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn deterministic_and_sensitive() {
        // S2: same bytes -> same hash; one flipped byte -> different hash.
        let mut a = StateHasher::new();
        a.write(&[1, 2, 3]).write_u16(0x1234).write_u8(0xFF);
        let mut b = StateHasher::new();
        b.write(&[1, 2, 3]).write_u16(0x1234).write_u8(0xFF);
        assert_eq!(a.finish(), b.finish(), "identical state -> identical hash");

        let mut c = StateHasher::new();
        c.write(&[1, 2, 3]).write_u16(0x1234).write_u8(0xFE); // last byte differs
        assert_ne!(a.finish(), c.finish(), "differing state -> differing hash");
    }

    #[test]
    fn streaming_equals_oneshot() {
        let mut s = StateHasher::new();
        s.write(b"foo").write(b"bar");
        assert_eq!(s.finish(), fnv1a64(b"foobar"));
    }

    #[test]
    fn csv_writes_header_and_rows() {
        let dir = std::env::temp_dir().join(format!("rubc_hash_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut csv = HashCsv::create(&dir).unwrap();
        csv.row(0, 70224, 70224, 0x0150, 144, 1, 0x9fc63a9124d6e21a)
            .unwrap();
        let content = std::fs::read_to_string(dir.join("hash.csv")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "frame,mcycle,tcycle,pc,ly,ppu_mode,hash");
        assert!(lines[1].starts_with("0,70224,70224,0150,144,1,"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
