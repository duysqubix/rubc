use crate::{cartridge::Cartridge, globals::*, opcodes::op_code_names};
use prettytable::{format, row, Table};

pub fn disassemble(cart: &Cartridge) -> String {
    let rom = cart.rom();

    let rel_addr = |bank: usize, idx: usize| bank * ROM_BANK_SIZE + idx;

    let mut table = Table::new();
    let header = row![
        "ADDRESS",
        "OPCODE",
        "VALUE",
        "DESCRIPTION",
        "NOTES",
        "OPLEN"
    ];
    table.set_titles(header);

    let mut opcode: u8;
    let mut notes: &str = "";

    for bank in 0..(rom.len() / ROM_BANK_SIZE) {
        let mut i = 0;
        while i < ROM_BANK_SIZE {
            let mut cb_mode = false;
            let address = rel_addr(bank, i);
            let oplen = OPCODE_LENGTHS[rom[address] as usize] as usize;
            if rom[address] == 0x00 {
                i += 1;
                continue;
            }

            opcode = rom[address];
            if opcode == 0xCB {
                i += 1;
                opcode = rom[rel_addr(bank, i)];
                notes = "CB Prefix";
                cb_mode = true;
            }

            match oplen {
                2 => {
                    let orig_addr = address;
                    notes = "8bit Immediate";
                    opcode = rom[address];
                    i += 1;
                    let value = rom[rel_addr(bank, i)];
                    let opcode_name = op_code_names(opcode, cb_mode);

                    table.add_row(row![
                        format!("{:02X}:{:02X}", bank, orig_addr),
                        format!("${:02X}", opcode),
                        format!("${:04X}", value),
                        opcode_name,
                        notes,
                        oplen
                    ]);
                    i += 1;
                }
                3 => {
                    let orig_addr = address;
                    notes = "16bit Immediate";
                    opcode = rom[address];
                    i += 1;
                    let h = rom[rel_addr(bank, i)] as u16;
                    i += 1;
                    let l = rom[rel_addr(bank, i)] as u16;
                    let value = (l << 8) | h;
                    let opcode_name = op_code_names(opcode, cb_mode);
                    table.add_row(row![
                        format!("{:02X}:{:02X}", bank, orig_addr),
                        format!("${:02X}", opcode),
                        format!("${:04X}", value),
                        opcode_name,
                        notes,
                        oplen
                    ]);
                    i += 1;
                }
                _ => {
                    let orig_addr = address;
                    let opcode_name = op_code_names(opcode, cb_mode);
                    table.add_row(row![
                        format!("{:02X}:{:02X}", bank, orig_addr),
                        format!("${:02X}", opcode),
                        "",
                        opcode_name,
                        notes,
                        oplen
                    ]);
                    i += 1;
                }
            }
        }
    }
    table.set_format(*format::consts::FORMAT_BORDERS_ONLY);

    table.to_string()
}

pub fn get_metadata(cart: &Cartridge) -> String {
    let rom = cart.rom();

    let header_checksum = rom[CART_HEADER_CHECKSUM as usize];
    let checksum =
        calculate_checksum(&rom[CART_TITLE_START as usize..CART_GLOBAL_CHECKSUM_END as usize]);

    let mut table = Table::new();
    table.set_titles(row!["Attribute", "Value"]);
    table.add_row(row![
        "TITLE",
        &rom[CART_TITLE_START as usize..CART_TITLE_END as usize]
            .iter()
            .map(|x| *x as char)
            .collect::<String>()
    ]);
    table.add_row(row!["CGB Flag", cbg_flag_map(rom[CART_TYPE as usize])]);
    table.add_row(row!["SGB Flag", rom[CART_SGB_FLAG as usize]]);
    table.add_row(row![
        "Cartridge Type",
        cart_type_map(rom[CART_TYPE as usize])
    ]);
    table.add_row(row!["ROM Size", rom_size_map(rom[CART_ROM_SIZE as usize])]);
    table.add_row(row!["RAM Size", ram_size_map(rom[CART_SRAM_SIZE as usize])]);
    table.add_row(row!["Header Checksum", format!("{:#x}", header_checksum)]);
    table.add_row(row!["Checksum Valid", header_checksum == checksum]);
    table.add_row(row![
        "Global Checksum",
        format!("{:#x}", {
            let hi_byte = rom[CART_GLOBAL_CHECKSUM_START as usize];
            let lo_byte = rom[CART_GLOBAL_CHECKSUM_END as usize + 1];
            (hi_byte as u16) << 8 | (lo_byte as u16)
        })
    ]);

    table.set_format(*format::consts::FORMAT_BORDERS_ONLY);

    table.to_string()
}

pub fn calculate_checksum(mem: &[u8]) -> u8 {
    mem.iter().fold(0, |acc: u8, x: &u8| {
        // asdf
        let y = x + 1;
        acc.wrapping_sub(y)
    }) - 1
}

#[inline]
pub const fn interrupt_address(val: u8) -> u16 {
    match val {
        0 => INTR_VBLANK,
        1 => INTR_LCD_STAT,
        2 => INTR_TIMER,
        3 => INTR_SERIAL,
        4 => INTR_HIGH_TO_LOW,
        _ => {
            panic!("Invalid interrupt address");
        }
    }
}
