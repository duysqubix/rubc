#[inline(always)]
pub const fn is_bit_set(value: u8, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}

#[inline(always)]
pub fn set_bit(value: &mut u8, bit: u8) {
    *value |= 1 << bit;
}

#[inline(always)]
pub fn clear_bit(value: &mut u8, bit: u8) {
    *value &= !(1 << bit);
}

#[inline(always)]
pub fn toggle_bit(value: &mut u8, bit: u8) {
    *value ^= 1 << bit;
}

#[inline(always)]
pub fn get_bit(value: u8, bit: u8) -> u8 {
    (value >> bit) & 1
}
