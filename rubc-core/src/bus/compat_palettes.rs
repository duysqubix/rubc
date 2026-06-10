// Palette data transcribed from SameBoy's open-source cgb_boot.asm
// (MIT/Expat, LIJI32/SameBoy); data tables only.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteSet {
    pub obj0: [u16; 4],
    pub obj1: [u16; 4],
    pub bg: [u16; 4],
}

pub const DEFAULT_OBJ: [u16; 4] = [0x7FFF, 0x421F, 0x1CF2, 0x0000];
pub const DEFAULT_BG: [u16; 4] = [0x7FFF, 0x1BEF, 0x6180, 0x0000];
pub const DEFAULT_SET: PaletteSet = PaletteSet {
    obj0: DEFAULT_OBJ,
    obj1: DEFAULT_OBJ,
    bg: DEFAULT_BG,
};

const TITLE_CHECKSUMS: [u8; 94] = [
    0x00, 0x88, 0x16, 0x36, 0xD1, 0xDB, 0xF2, 0x3C, 0x8C, 0x92, 0x3D, 0x5C, 0x58, 0xC9, 0x3E, 0x70,
    0x1D, 0x59, 0x69, 0x19, 0x35, 0xA8, 0x14, 0xAA, 0x75, 0x95, 0x99, 0x34, 0x6F, 0x15, 0xFF, 0x97,
    0x4B, 0x90, 0x17, 0x10, 0x39, 0xF7, 0xF6, 0xA2, 0x49, 0x4E, 0x43, 0x68, 0xE0, 0x8B, 0xF0, 0xCE,
    0x0C, 0x29, 0xE8, 0xB7, 0x86, 0x9A, 0x52, 0x01, 0x9D, 0x71, 0x9C, 0xBD, 0x5D, 0x6D, 0x67, 0x3F,
    0x6B, 0xB3, 0x46, 0x28, 0xA5, 0xC6, 0xD3, 0x27, 0x61, 0x18, 0x66, 0x6A, 0xBF, 0x0D, 0xF4, 0xB3,
    0x46, 0x28, 0xA5, 0xC6, 0xD3, 0x27, 0x61, 0x18, 0x66, 0x6A, 0xBF, 0x0D, 0xF4, 0xB3,
];

const DUP_FOURTH_LETTERS: [u8; 29] = *b"BEFAARBEKEK R-URAR INAILICE R";

const PALETTE_PER_CHECKSUM: [u8; 94] = [
    0x00, 0x04, 0x05, 0x23, 0x22, 0x03, 0x1F, 0x0F, 0x0A, 0x05, 0x13, 0x24, 0x87, 0x25, 0x1E, 0x2C,
    0x15, 0x20, 0x1F, 0x14, 0x05, 0x21, 0x0D, 0x0E, 0x05, 0x1D, 0x05, 0x12, 0x09, 0x03, 0x02, 0x1A,
    0x19, 0x19, 0x29, 0x2A, 0x1A, 0x2D, 0x2A, 0x2D, 0x24, 0x26, 0x9A, 0x2A, 0x1E, 0x29, 0x22, 0x22,
    0x05, 0x2A, 0x06, 0x05, 0x21, 0x19, 0x2A, 0x2A, 0x28, 0x02, 0x10, 0x19, 0x2A, 0x2A, 0x05, 0x00,
    0x27, 0x24, 0x16, 0x19, 0x06, 0x20, 0x0C, 0x24, 0x0B, 0x27, 0x12, 0x27, 0x18, 0x1F, 0x32, 0x11,
    0x2E, 0x06, 0x1B, 0x00, 0x2F, 0x29, 0x29, 0x00, 0x00, 0x13, 0x22, 0x17, 0x12, 0x1D,
];

const PALETTE_COMBINATIONS: [[u8; 3]; 55] = [
    [32, 32, 232],
    [144, 144, 144],
    [160, 160, 160],
    [192, 192, 192],
    [72, 72, 72],
    [0, 0, 0],
    [216, 216, 216],
    [40, 40, 40],
    [96, 96, 96],
    [208, 208, 208],
    [128, 64, 64],
    [32, 224, 224],
    [32, 16, 16],
    [24, 32, 32],
    [32, 232, 232],
    [224, 32, 224],
    [16, 136, 16],
    [128, 128, 64],
    [32, 32, 56],
    [32, 32, 144],
    [32, 32, 160],
    [152, 152, 72],
    [30, 30, 88],
    [136, 136, 16],
    [32, 32, 16],
    [32, 32, 24],
    [224, 224, 0],
    [24, 24, 0],
    [0, 0, 8],
    [144, 176, 144],
    [160, 176, 160],
    [192, 176, 192],
    [128, 176, 64],
    [136, 32, 104],
    [222, 0, 112],
    [222, 32, 120],
    [152, 182, 72],
    [128, 224, 80],
    [32, 184, 224],
    [136, 176, 16],
    [32, 0, 16],
    [32, 224, 24],
    [224, 24, 0],
    [24, 224, 32],
    [168, 224, 32],
    [24, 224, 0],
    [200, 24, 224],
    [0, 224, 64],
    [32, 24, 224],
    [224, 24, 48],
    [32, 224, 232],
    [240, 240, 240],
    [248, 248, 248],
    [224, 32, 8],
    [0, 0, 16],
];

const PALETTES: [[u16; 4]; 32] = [
    [0x7FFF, 0x32BF, 0x00D0, 0x0000],
    [0x639F, 0x4279, 0x15B0, 0x04CB],
    [0x7FFF, 0x6E31, 0x454A, 0x0000],
    [0x7FFF, 0x1BEF, 0x0200, 0x0000],
    [0x7FFF, 0x421F, 0x1CF2, 0x0000],
    [0x7FFF, 0x5294, 0x294A, 0x0000],
    [0x7FFF, 0x03FF, 0x012F, 0x0000],
    [0x7FFF, 0x03EF, 0x01D6, 0x0000],
    [0x7FFF, 0x42B5, 0x3DC8, 0x0000],
    [0x7E74, 0x03FF, 0x0180, 0x0000],
    [0x67FF, 0x77AC, 0x1A13, 0x2D6B],
    [0x7ED6, 0x4BFF, 0x2175, 0x0000],
    [0x53FF, 0x4A5F, 0x7E52, 0x0000],
    [0x4FFF, 0x7ED2, 0x3A4C, 0x1CE0],
    [0x03ED, 0x7FFF, 0x255F, 0x0000],
    [0x036A, 0x021F, 0x03FF, 0x7FFF],
    [0x7FFF, 0x01DF, 0x0112, 0x0000],
    [0x231F, 0x035F, 0x00F2, 0x0009],
    [0x7FFF, 0x03EA, 0x011F, 0x0000],
    [0x299F, 0x001A, 0x000C, 0x0000],
    [0x7FFF, 0x027F, 0x001F, 0x0000],
    [0x7FFF, 0x03E0, 0x0206, 0x0120],
    [0x7FFF, 0x7EEB, 0x001F, 0x7C00],
    [0x7FFF, 0x3FFF, 0x7E00, 0x001F],
    [0x7FFF, 0x03FF, 0x001F, 0x0000],
    [0x03FF, 0x001F, 0x000C, 0x0000],
    [0x7FFF, 0x033F, 0x0193, 0x0000],
    [0x0000, 0x4200, 0x037F, 0x7FFF],
    [0x7FFF, 0x7E8C, 0x7C00, 0x0000],
    [0x7FFF, 0x1BEF, 0x6180, 0x0000],
    [0x7FFF, 0x7FEA, 0x7D5F, 0x0000],
    [0x4778, 0x3290, 0x1D87, 0x0861],
];

pub fn title_checksum_for_boot_regs(rom: &[u8]) -> u8 {
    if nintendo_licensee(rom) {
        title_checksum(rom)
    } else {
        0
    }
}

pub fn compat_hl(title_checksum: u8) -> u16 {
    if matches!(title_checksum, 0x43 | 0x58) {
        0x991A
    } else {
        0x007C
    }
}

pub fn select_palette_set(rom: &[u8]) -> PaletteSet {
    let Some(palette_id) = select_palette_id(rom) else {
        return DEFAULT_SET;
    };
    let combination = PALETTE_COMBINATIONS
        .get(palette_id as usize)
        .copied()
        .unwrap_or(PALETTE_COMBINATIONS[0]);
    PaletteSet {
        obj0: PALETTES[(combination[0] / 8) as usize],
        obj1: PALETTES[(combination[1] / 8) as usize],
        bg: PALETTES[(combination[2] / 8) as usize],
    }
}

pub fn load_compat_palettes(
    rom: &[u8],
    bg_palette_ram: &mut [u8; 64],
    obj_palette_ram: &mut [u8; 64],
) {
    let set = select_palette_set(rom);
    write_palette(&mut bg_palette_ram[0..8], set.bg);
    write_palette(&mut obj_palette_ram[0..8], set.obj0);
    write_palette(&mut obj_palette_ram[8..16], set.obj1);
}

fn select_palette_id(rom: &[u8]) -> Option<u8> {
    if !nintendo_licensee(rom) {
        return None;
    }
    let checksum = title_checksum(rom);
    for (index, candidate) in TITLE_CHECKSUMS.iter().copied().enumerate() {
        if candidate != checksum {
            continue;
        }
        if index <= 64 || rom.get(0x0137).copied() == DUP_FOURTH_LETTERS.get(index - 65).copied() {
            return PALETTE_PER_CHECKSUM.get(index).map(|entry| entry & 0x7F);
        }
    }
    None
}

fn write_palette(dst: &mut [u8], colors: [u16; 4]) {
    for (i, color) in colors.into_iter().enumerate() {
        let [lo, hi] = color.to_le_bytes();
        dst[i * 2] = lo;
        dst[i * 2 + 1] = hi;
    }
}

fn nintendo_licensee(rom: &[u8]) -> bool {
    matches!(rom.get(0x014B), Some(0x01))
        || (matches!(rom.get(0x014B), Some(0x33)) && rom.get(0x0144..0x0146) == Some(b"01"))
}

fn title_checksum(rom: &[u8]) -> u8 {
    rom.get(0x0134..0x0144)
        .unwrap_or(&[])
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_with_title_sum(sum: u8, nintendo: bool) -> Vec<u8> {
        let mut rom = vec![0; 0x8000];
        rom[0x0134] = sum;
        rom[0x014B] = if nintendo { 0x01 } else { 0x00 };
        rom
    }

    #[test]
    fn unknown_nintendo_title_uses_default_palette() {
        let rom = rom_with_title_sum(0x02, true);
        assert_eq!(select_palette_set(&rom), DEFAULT_SET);
    }

    #[test]
    fn non_nintendo_title_uses_default_palette() {
        let rom = rom_with_title_sum(0x88, false);
        assert_eq!(select_palette_set(&rom), DEFAULT_SET);
    }

    #[test]
    fn known_nintendo_checksum_maps_to_its_combination() {
        let rom = rom_with_title_sum(0x16, true);
        assert_eq!(
            select_palette_set(&rom),
            PaletteSet {
                obj0: PALETTES[0],
                obj1: PALETTES[0],
                bg: PALETTES[0]
            }
        );
    }

    #[test]
    fn duplicate_checksum_uses_fourth_title_letter() {
        let mut rom = rom_with_title_sum(0xB3, true);
        rom[0x0134] = 0xB3_u8.wrapping_sub(b'B');
        rom[0x0137] = b'B';
        assert_eq!(select_palette_id(&rom), Some(0x24));
    }
}
