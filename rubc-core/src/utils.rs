pub fn format_binary(value: u8) -> String {
    format!("0b{:04b}_{:04b}", value >> 4, value & 0x0F)
}
