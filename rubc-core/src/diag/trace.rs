//! BGB-compatible instruction trace formatting.
//!
//! Emitting one line per real opcode fetch in the exact format BGB / no$gmb
//! debuggers use lets an AFK agent diff rubc's trace against a known-good
//! emulator and find the FIRST point of divergence on a failing ROM.
//!
//! This module only FORMATS. The caller builds a [`BgbRegs`] from a
//! side-effect-free snapshot (registers + the four bytes at PC), then hands the
//! finished string to `Diagnostics::trace_instr_line`. Nothing here touches the
//! bus or ticks anything.

/// A side-effect-free snapshot of the CPU registers + the four bytes at PC,
/// everything needed to format one BGB trace line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BgbRegs {
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
    /// The four bytes at PC (PC, PC+1, PC+2, PC+3), peeked side-effect-free.
    pub pcmem: [u8; 4],
}

/// Format one BGB-compatible trace line.
///
/// Example: `A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100 PCMEM:00,C3,13,02`
pub fn format_bgb_line(r: &BgbRegs) -> String {
    format!(
        "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} SP:{:04X} PC:{:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
        r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l, r.sp, r.pc,
        r.pcmem[0], r.pcmem[1], r.pcmem[2], r.pcmem[3],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgb_line_byte_exact() {
        // Post-boot DMG state at PC=0x0100, a JP nn (C3 13 02) at PC.
        let r = BgbRegs {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            pcmem: [0x00, 0xC3, 0x13, 0x02],
        };
        assert_eq!(
            format_bgb_line(&r),
            "A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100 PCMEM:00,C3,13,02"
        );
    }

    #[test]
    fn bgb_line_pads_and_uppercases() {
        let r = BgbRegs {
            a: 0x0a,
            sp: 0x000f,
            pc: 0xabcd,
            pcmem: [0xff, 0x00, 0x1a, 0x2b],
            ..Default::default()
        };
        let line = format_bgb_line(&r);
        assert!(line.starts_with("A:0A "));
        assert!(line.contains("SP:000F"));
        assert!(line.contains("PC:ABCD"));
        assert!(line.ends_with("PCMEM:FF,00,1A,2B"));
    }
}
