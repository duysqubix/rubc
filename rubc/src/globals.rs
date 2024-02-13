use crate::gameboy::Gameboy;

pub type OpCodeFunc = fn(mb: &mut Gameboy, value: u16) -> OpCycles;

pub type OpCycles = u64;
pub type OpCodeMap = phf::Map<u8, OpCodeFunc>;

pub const ROM_MAX_BANKS: usize = 128;
pub const RAM_MAX_BANKS: usize = 16;
pub const ROM_BANK_SIZE: usize = 0x4000;
pub const RAM_BANK_SIZE: usize = 0x2000;

pub const CYCLE_RETURN_4: OpCycles = 4;
pub const CYCLE_RETURN_8: OpCycles = 8;
pub const CYCLE_RETURN_12: OpCycles = 12;
pub const CYCLE_RETURN_16: OpCycles = 16;
pub const CYCLE_RETURN_20: OpCycles = 20;
pub const CYCLE_RETURN_24: OpCycles = 24;

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
    1, 3, 1, 1, 1, 1, 2, 1, 3, 1, 1, 1, 1, 1, 2, 1, //  $00 - $0F
    1, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, //  $10 - $1F
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, //  $20 - $2F
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, //  $30 - $3F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $40 - $4F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $50 - $5F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $60 - $6F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $70 - $7F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $80 - $8F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $90 - $9F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $A0 - $AF
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //  $B0 - $BF
    1, 1, 3, 3, 3, 1, 2, 1, 1, 1, 3, 1, 3, 3, 2, 1, //  $C0 - $CF
    1, 1, 3, 0, 3, 1, 2, 1, 1, 1, 3, 0, 3, 0, 2, 1, //  $D0 - $DF
    2, 1, 1, 0, 0, 1, 2, 1, 2, 1, 3, 0, 0, 0, 2, 1, //  $E0 - $EF
    2, 1, 1, 1, 0, 1, 2, 1, 2, 1, 3, 1, 0, 0, 2, 1, //  $F0 - $FF
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
pub static ILLEGAL_OPCODES: &[u8] = &[
    0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD,
];
// pub const CART_MBC5_ID: u8 = 0x05;

// IO Registers
pub const IO_START_ADDR: u16 = 0xFF00; // Start of IO addresses
pub const IO_END_ADDR: u16 = 0xFF7F; // End of IO addresses
pub const IO_P1_JOYP: u16 = 0xFF00; // Joypad
pub const IO_SB: u16 = 0xFF01; // Serial transfer data
pub const IO_SC: u16 = 0xFF02; // Serial transfer control
pub const IO_DIV: u16 = 0xFF04; // Divider Register
pub const IO_TIMA: u16 = 0xFF05; // Timer counter
pub const IO_TMA: u16 = 0xFF06; // Timer Modulo
pub const IO_TAC: u16 = 0xFF07; // Timer Control
pub const IO_IF: u16 = 0xFF0F; // Interrupt Flag
pub const IO_NR10: u16 = 0xFF10; // Sound Mode 1 register, Sweep register
pub const IO_NR11: u16 = 0xFF11; // Sound Mode 1 register, Sound length/Wave pattern duty
pub const IO_NR12: u16 = 0xFF12; // Sound Mode 1 register, Envelope
pub const IO_NR13: u16 = 0xFF13; // Sound Mode 1 register, Frequency lo
pub const IO_NR14: u16 = 0xFF14; // Sound Mode 1 register, Frequency hi
pub const IO_NR21: u16 = 0xFF16; // Sound Mode 2 register, Sound length/Wave pattern duty
pub const IO_NR22: u16 = 0xFF17; // Sound Mode 2 register, Envelope
pub const IO_NR23: u16 = 0xFF18; // Sound Mode 2 register, Frequency lo
pub const IO_NR24: u16 = 0xFF19; // Sound Mode 2 register, Frequency hi
pub const IO_NR30: u16 = 0xFF1A; // Sound Mode 3 register, Sound on/off
pub const IO_NR31: u16 = 0xFF1B; // Sound Mode 3 register, Sound length
pub const IO_NR32: u16 = 0xFF1C; // Sound Mode 3 register, Select output level
pub const IO_NR33: u16 = 0xFF1D; // Sound Mode 3 register, Frequency lo
pub const IO_NR34: u16 = 0xFF1E; // Sound Mode 3 register, Frequency hi
pub const IO_NR41: u16 = 0xFF20; // Sound Mode 4 register, Sound length
pub const IO_NR42: u16 = 0xFF21; // Sound Mode 4 register, Envelope
pub const IO_NR43: u16 = 0xFF22; // Sound Mode 4 register, Polynomial counter
pub const IO_NR44: u16 = 0xFF23; // Sound Mode 4 register, Counter/consecutive; Inital
pub const IO_NR50: u16 = 0xFF24; // Channel control / ON-OFF / Volume
pub const IO_NR51: u16 = 0xFF25; // Selection of Sound output terminal
pub const IO_NR52: u16 = 0xFF26; // Sound on/off
pub const IO_WAVE_RAM1: u16 = 0xFF30; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM2: u16 = 0xFF31; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM3: u16 = 0xFF32; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM4: u16 = 0xFF33; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM5: u16 = 0xFF34; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM6: u16 = 0xFF35; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM7: u16 = 0xFF36; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM8: u16 = 0xFF37; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAM9: u16 = 0xFF38; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAMA: u16 = 0xFF39; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAMB: u16 = 0xFF3A; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAMC: u16 = 0xFF3B; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAMD: u16 = 0xFF3C; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAME: u16 = 0xFF3D; // Waveform storage for arbitrary sound data
pub const IO_WAVE_RAMF: u16 = 0xFF3E; // Waveform storage for arbitrary sound data

pub const IO_LCDC: u16 = 0xFF40; // LCD Control
pub const IO_STAT: u16 = 0xFF41; // LCD Status
pub const IO_SCY: u16 = 0xFF42; // Scroll Y
pub const IO_SCX: u16 = 0xFF43; // Scroll X
pub const IO_LY: u16 = 0xFF44; // LCDC Y-Coordinate
pub const IO_LYC: u16 = 0xFF45; // LY Compare
pub const IO_DMA: u16 = 0xFF46; // DMA Transfer and Start Address
pub const IO_BGP: u16 = 0xFF47; // BG Palette Data
pub const IO_OBP0: u16 = 0xFF48; // Object Palette 0 Data
pub const IO_OBP1: u16 = 0xFF49; // Object Palette 1 Data
pub const IO_WY: u16 = 0xFF4A; // Window Y Position
pub const IO_WX: u16 = 0xFF4B; // Window X Position
pub const IO_KEY1: u16 = 0xFF4D; // CGB Mode Only - Prepare Speed Switch
pub const IO_VBK: u16 = 0xFF4F; // CGB Mode Only - VRAM Bank
pub const IO_HDMA1: u16 = 0xFF51; // CGB Mode Only - New DMA Source, High
pub const IO_HDMA2: u16 = 0xFF52; // CGB Mode Only - New DMA Source, Low
pub const IO_HDMA3: u16 = 0xFF53; // CGB Mode Only - New DMA Destination, High
pub const IO_HDMA4: u16 = 0xFF54; // CGB Mode Only - New DMA Destination, Low
pub const IO_HDMA5: u16 = 0xFF55; // CGB Mode Only - New DMA Length/Mode/Start
pub const IO_RP: u16 = 0xFF56; // CGB Mode Only - Infrared Communications Port
pub const IO_BCPS: u16 = 0xFF68; // CGB Mode Only - Background Color Palette Specification
pub const IO_BCPD: u16 = 0xFF69; // CGB Mode Only - Background Color Palette Data
pub const IO_OCPS: u16 = 0xFF6A; // CGB Mode Only - Object Color Palette Specification
pub const IO_OCPD: u16 = 0xFF6B; // CGB Mode Only - Object Color Palette Data
pub const IO_OPRI: u16 = 0xFF6C; // CGB Mode Only - Object Priority
pub const IO_SVBK: u16 = 0xFF70; // CGB Mode Only - WRAM Bank
pub const IO_PCM12: u16 = 0xFF76; // CGB Mode Only - PCM Channel 1&2 Control
pub const IO_PCM34: u16 = 0xFF77; // CGB Mode Only - PCM Channel 3&4 Control
