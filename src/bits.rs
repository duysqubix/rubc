pub const fn is_bit_set(value: u8, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}

pub const fn set_bit(value: u8, bit: u8) -> u8 {
    value | (1 << bit)
}

pub const fn clear_bit(value: u8, bit: u8) -> u8 {
    value & !(1 << bit)
}

pub const fn toggle_bit(value: u8, bit: u8) -> u8 {
    value ^ (1 << bit)
}

pub const fn get_bit(value: u8, bit: u8) -> u8 {
    (value >> bit) & 1
}
