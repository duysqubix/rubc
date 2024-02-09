use crate::gameboy::Motherboard;

pub type OpCodeFunc = fn(mb: &mut Motherboard, value: u16) -> OpCycles;

pub type OpCycles = u64;
pub type OpCodeMap = phf::Map<u8, OpCodeFunc>;

pub const ROM_MAX_BANKS: usize = 128;
pub const RAM_MAX_BANKS: usize = 16;
pub const ROM_BANK_SIZE: usize = 0x4000;
pub const RAM_BANK_SIZE: usize = 0x2000;
pub const CYCLE_RETURN: OpCycles = 4;
pub const BIT_FLAGZ: u8 = 7;
pub const BIT_FLAGN: u8 = 6;
pub const BIT_FLAGH: u8 = 5;
pub const BIT_FLAGC: u8 = 4;

pub const ROM_ADDRESS_START: u16 = 0x0000;
pub const ROM_ADDRESS_END: u16 = 0x3FFF;
pub const ROM1_ADDRESS_START: u16 = 0x4000;
pub const ROM1_ADDRESS_END: u16 = 0x7FFF;
pub const VRAM_ADDRESS_START: u16 = 0x8000;
pub const VRAM_ADDRESS_END: u16 = 0x9FFF;
pub const EXTERNAL_RAM_ADDRESS_START: u16 = 0xA000;
pub const EXTERNAL_RAM_ADDRESS_END: u16 = 0xBFFF;
pub const WRAM_ADDRESS_START: u16 = 0xC000;
pub const WRAM_ADDRESS_END: u16 = 0xCFFF;
pub const WRAM1_ADDRESS_START: u16 = 0xD000;
pub const WRAM1_ADDRESS_END: u16 = 0xDFFF;
pub const ECHO_RAM_ADDRESS_START: u16 = 0xC000;
pub const ECHO_RAM_ADDRESS_END: u16 = 0xDDFF;
pub const OAM_ADDRESS_START: u16 = 0xFE00;
pub const OAM_ADDRESS_END: u16 = 0xFE9F;
pub const IO_ADDRESS_START: u16 = 0xFF00;
pub const IO_ADDRESS_END: u16 = 0xFF7F;
pub const HRAM_ADDRESS_START: u16 = 0xFF80;
pub const HRAM_ADDRESS_END: u16 = 0xFFFE;
pub const INTERRUPT_ENABLE_ADDRESS: u16 = 0xFFFF;

pub const CART_HEADER_START: u16 = 0x0100;
pub const CART_HEADER_END: u16 = 0x014F;

pub const CART_ENTRY_POINT_START: u16 = 0x0100;
pub const CART_ENTRY_POINT_END: u16 = 0x0103;
pub const CART_NINTENDO_LOGO_START: u16 = 0x0104;
pub const CART_NINTENDO_LOGO_END: u16 = 0x0133;
pub const CART_TITLE_START: u16 = 0x0134;
pub const CART_TITLE_END: u16 = 0x0142;
pub const CART_CBG_FLAG: u16 = 0x0143; // $80 = CGB/GB, $C0 = CGB only, other = GB
pub const CART_MANUFACTURER_CODE_START: u16 = 0x013F;
pub const CART_MANUFACTURER_CODE_END: u16 = 0x0142;
pub const CART_NEW_LICENSEE_CODE_START: u16 = 0x0144;
pub const CART_NEW_LICENSEE_CODE_END: u16 = 0x0145;
pub const CART_SGB_FLAG: u16 = 0x0146; // $00 = No SGB functions, $03 = SGB functions
pub const CART_TYPE: u16 = 0x0147;
pub const CART_ROM_SIZE: u16 = 0x0148;
pub const CART_SRAM_SIZE: u16 = 0x0149;
pub const CART_DESTINATION_CODE: u16 = 0x014A;
pub const CART_OLD_LICENSEE_CODE: u16 = 0x014B;
pub const CART_MASK_ROM_VERSION_NUMBER: u16 = 0x014C;
pub const CART_HEADER_CHECKSUM: u16 = 0x014D;
pub const CART_GLOBAL_CHECKSUM_START: u16 = 0x014E;
pub const CART_GLOBAL_CHECKSUM_END: u16 = 0x014F;

pub const CART_MBC0_ID: u8 = 0x00;
pub const CART_MBC1_ID: u8 = 0x01;
pub const CART_MBC2_ID: u8 = 0x02;
pub const CART_MBC3_ID: u8 = 0x03;
pub const CART_MBC4_ID: u8 = 0x04;
pub const CART_MBC5_ID: u8 = 0x05;

pub static OPCODE_LENGTHS: &[u8] = &[
    1, 3, 1, 1, 1, 1, 2, 1, 3, 1, 1, 1, 1, 1, 2, 1, 2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1,
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, 2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 3, 3, 3, 1, 2, 1, 1, 1, 3, 1, 3, 3, 2, 1, 1, 1, 3, 0, 3, 1, 2, 1, 1, 1, 3, 0, 3, 0, 2, 1,
    2, 1, 1, 0, 0, 1, 2, 1, 2, 1, 3, 0, 0, 0, 2, 1, 2, 1, 1, 1, 0, 1, 2, 1, 2, 1, 3, 1, 0, 0, 2, 1,
    // CB prefix instructions do not take any arguments
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

// pub static MEMORY: OnceLock<[u8; 0xFFFF]> = OnceLock::new();
