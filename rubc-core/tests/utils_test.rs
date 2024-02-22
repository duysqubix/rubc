#[cfg(test)]
mod tests {
    use rubc_core::utils::*;

    #[test]
    fn test_checksum() {
        let data = vec![0x01, 0x02, 0x03, 0x04];
        assert_eq!(calculate_checksum(&data), 241);
    }

    #[test]
    fn test_rom_absolute_addr() {
        assert_eq!(rom_absolute_address(0, 0x4000), 0x4000);
        assert_eq!(rom_absolute_address(1, 0x4000), 0x8000);
        assert_eq!(rom_absolute_address(2, 0x4000), 0xC000);
        assert_eq!(rom_absolute_address(2, 0x1223), 0x9223);
    }

    #[test]
    fn test_ram_absolute_addr() {
        assert_eq!(ram_absolute_address(0, 0x0010), 0x0010);
        assert_eq!(ram_absolute_address(1, 0x0010), 0x2010);
        assert_eq!(ram_absolute_address(2, 0x0010), 0x4010);
        assert_eq!(ram_absolute_address(2, 0x1223), 0x5223);
    }
}
