//! Full machine-state snapshot for post-mortem inspection.
//!
//! On panic / stuck / illegal-opcode / bus-invariant violation, dump the entire
//! machine state to `snapshot.json` so an AFK agent can load "the state at the
//! moment it broke". Large memory regions (VRAM/WRAM/OAM) are base64-encoded to
//! keep the JSON compact and agent-readable.
//!
//! Pure-Rust: the base64 encoder is implemented here (no `base64` crate),
//! observe-only, no `unsafe`.

/// CPU register file at the snapshot instant.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpuRegs {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
}

/// A full machine snapshot. Memory regions are owned byte vectors so the
/// snapshot can outlive the emulator and be serialized lazily.
#[derive(Clone, Debug, Default)]
pub struct MachineSnapshot {
    pub reason: String,
    pub run_id: String,
    pub mcycle: u64,
    pub tcycle: u64,
    pub frame: u64,

    pub cpu: CpuRegs,
    pub ie: u8,
    pub if_: u8,

    /// IO register block FF00..FF7F as raw bytes (hex-encoded in JSON).
    pub io: Vec<u8>,
    pub hram: Vec<u8>,
    pub wram: Vec<u8>,
    pub vram: Vec<u8>,
    pub oam: Vec<u8>,
    pub cart_ram: Option<Vec<u8>>,

    pub last_trace_line: Option<String>,
    pub last_anomaly: Option<String>,
}

impl MachineSnapshot {
    /// Write `snapshot.json` into `dir`.
    pub fn write_json(&self, dir: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(dir.join("snapshot.json"))?;
        f.write_all(self.to_json().as_bytes())
    }

    /// Render the snapshot as a JSON object string.
    pub fn to_json(&self) -> String {
        let mut s = String::with_capacity(1024);
        s.push_str("{\n");
        s.push_str("  \"schema\": \"rubc.diag.snapshot.v1\",\n");
        s.push_str(&format!("  \"reason\": \"{}\",\n", esc(&self.reason)));
        s.push_str(&format!("  \"run_id\": \"{}\",\n", esc(&self.run_id)));
        s.push_str(&format!("  \"mcycle\": {},\n", self.mcycle));
        s.push_str(&format!("  \"tcycle\": {},\n", self.tcycle));
        s.push_str(&format!("  \"frame\": {},\n", self.frame));
        s.push_str("  \"cpu\": {");
        s.push_str(&format!(
            "\"a\":{},\"f\":{},\"b\":{},\"c\":{},\"d\":{},\"e\":{},\"h\":{},\"l\":{},\"sp\":{},\"pc\":{},\"ime\":{}",
            self.cpu.a, self.cpu.f, self.cpu.b, self.cpu.c, self.cpu.d, self.cpu.e,
            self.cpu.h, self.cpu.l, self.cpu.sp, self.cpu.pc, self.cpu.ime
        ));
        s.push_str("},\n");
        s.push_str(&format!("  \"ie\": {},\n", self.ie));
        s.push_str(&format!("  \"if\": {},\n", self.if_));
        s.push_str(&format!("  \"io_hex\": \"{}\",\n", hex(&self.io)));
        s.push_str(&format!(
            "  \"hram_b64\": \"{}\",\n",
            base64_encode(&self.hram)
        ));
        s.push_str(&format!(
            "  \"wram_b64\": \"{}\",\n",
            base64_encode(&self.wram)
        ));
        s.push_str(&format!(
            "  \"vram_b64\": \"{}\",\n",
            base64_encode(&self.vram)
        ));
        s.push_str(&format!(
            "  \"oam_b64\": \"{}\",\n",
            base64_encode(&self.oam)
        ));
        match &self.cart_ram {
            Some(r) => s.push_str(&format!("  \"cart_ram_b64\": \"{}\",\n", base64_encode(r))),
            None => s.push_str("  \"cart_ram_b64\": null,\n"),
        }
        match &self.last_trace_line {
            Some(t) => s.push_str(&format!("  \"last_trace_line\": \"{}\",\n", esc(t))),
            None => s.push_str("  \"last_trace_line\": null,\n"),
        }
        match &self.last_anomaly {
            Some(a) => s.push_str(&format!("  \"last_anomaly\": \"{}\"\n", esc(a))),
            None => s.push_str("  \"last_anomaly\": null\n"),
        }
        s.push('}');
        s
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 encoding (with `=` padding). Pure-Rust, no deps.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decode standard base64 (used only in tests to prove round-trip).
#[cfg(test)]
pub fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        let mut count = 0;
        for &c in chunk {
            if let Some(v) = val(c) {
                n = (n << 6) | v;
                count += 1;
            }
        }
        n <<= 6 * (4 - count);
        if count >= 2 {
            out.push((n >> 16) as u8);
        }
        if count >= 3 {
            out.push((n >> 8) as u8);
        }
        if count >= 4 {
            out.push(n as u8);
        }
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_vectors() {
        // RFC 4648 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn vram_b64_roundtrips() {
        // S4: snapshot VRAM bytes -> JSON -> base64 decodes back to original.
        let vram: Vec<u8> = (0..=255u8).cycle().take(8192).collect();
        let snap = MachineSnapshot {
            reason: "stuck".to_string(),
            run_id: "test".to_string(),
            vram: vram.clone(),
            ..Default::default()
        };
        let json = snap.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let b64 = v["vram_b64"].as_str().unwrap();
        assert_eq!(
            base64_decode(b64),
            vram,
            "VRAM must round-trip through base64"
        );
    }

    #[test]
    fn snapshot_json_is_valid_with_all_fields() {
        let snap = MachineSnapshot {
            reason: "illegal_opcode".to_string(),
            run_id: "2026_x".to_string(),
            mcycle: 843210,
            cpu: CpuRegs {
                a: 0x01,
                pc: 0x0150,
                ime: true,
                ..Default::default()
            },
            ie: 0x1F,
            if_: 0xE1,
            io: vec![0xCF; 0x80],
            hram: vec![0; 0x7F],
            oam: vec![0; 0xA0],
            cart_ram: Some(vec![0xAA; 16]),
            last_trace_line: Some("A:01 PC:0150".to_string()),
            last_anomaly: Some("IllegalOpcode".to_string()),
            ..Default::default()
        };
        let json = snap.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(v["schema"], "rubc.diag.snapshot.v1");
        assert_eq!(v["cpu"]["pc"], 0x0150);
        assert_eq!(v["cpu"]["ime"], true);
        assert_eq!(v["ie"], 0x1F);
        assert!(v["io_hex"].as_str().unwrap().starts_with("cfcf"));
        assert!(v["cart_ram_b64"].is_string());
    }
}
