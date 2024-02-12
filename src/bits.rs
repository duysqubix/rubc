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

#[macro_export]
macro_rules! clear_bits {
     ($val:expr, $($bit:expr),+) => {{
         // Start with all bits set
         let mut mask: u8 = 0xFF;
         // Combine masks for each bit to clear
         $(
             mask &= !(1 << $bit);
         )+
         // Clear the bits by ANDing with the negated mask
         $val &= mask
     }};
 }

#[macro_export]
macro_rules! set_bits {
     ($val:expr, $($bit:expr),+) => {{
         // Start with all bits cleared
         let mut mask: u8 = 0x00;
         // Combine masks for each bit to set
         $(
             mask |= 1 << $bit;
         )+
         // Set the bits by ORing with the mask
         $val |= mask
     }};
 }
