use rubc_core::bus::apu::Apu as OldApu;
use rubc_ng::Apu as NewApu;

fn write_both(old: &mut OldApu, new: &mut NewApu, addr: u16, value: u8, cgb: bool) {
    old.write(addr, value, cgb);
    new.write(addr, value, cgb);
}

#[test]
fn new_apu_matches_old_core_registers_and_samples_for_mixed_channel_program() {
    let mut old = OldApu::default();
    let mut new = NewApu::default();
    old.set_sample_rate(48_000);
    new.set_sample_rate(48_000);

    for (addr, value) in [
        (0xFF26, 0x80),
        (0xFF24, 0x77),
        (0xFF25, 0xFF),
        (0xFF10, 0x21),
        (0xFF11, 0x80),
        (0xFF12, 0xF2),
        (0xFF13, 0x00),
        (0xFF14, 0x82),
        (0xFF16, 0x40),
        (0xFF17, 0x84),
        (0xFF18, 0xC0),
        (0xFF19, 0x87),
        (0xFF1A, 0x80),
        (0xFF1C, 0x20),
        (0xFF1D, 0xFF),
        (0xFF1E, 0x87),
        (0xFF20, 0x20),
        (0xFF21, 0xF3),
        (0xFF22, 0x08),
        (0xFF23, 0x80),
    ] {
        write_both(&mut old, &mut new, addr, value, false);
    }
    for i in 0..16u16 {
        write_both(
            &mut old,
            &mut new,
            0xFF30 + i,
            (0x10 + i as u8) ^ 0xA5,
            false,
        );
    }

    for t in 1..=20_000u16 {
        old.tick_t();
        new.tick_spine(t, false);
        let old_high = t & 0x1000 != 0;
        let new_high = t.wrapping_add(1) & 0x1000 != 0;
        if old_high && !new_high {
            old.tick_div_apu();
        }
    }

    for addr in [
        0xFF10, 0xFF12, 0xFF14, 0xFF17, 0xFF19, 0xFF1A, 0xFF1C, 0xFF1E, 0xFF21, 0xFF22, 0xFF23,
        0xFF24, 0xFF25, 0xFF26,
    ] {
        assert_eq!(
            new.read(addr),
            old.read(addr),
            "register {addr:#06X} diverged"
        );
    }

    let mut old_samples = Vec::new();
    let mut new_samples = Vec::new();
    old.drain_samples(&mut old_samples);
    new.drain_samples(&mut new_samples);
    assert_eq!(
        new_samples.len(),
        old_samples.len(),
        "sample count diverged"
    );
    for (idx, (new_sample, old_sample)) in new_samples.iter().zip(old_samples.iter()).enumerate() {
        assert_eq!(
            new_sample.to_bits(),
            old_sample.to_bits(),
            "sample {idx} diverged"
        );
    }
}
