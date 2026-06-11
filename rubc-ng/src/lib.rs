#![forbid(unsafe_code)]

pub mod bus_intent;
pub mod golden;
pub mod machine;
pub mod model;
pub mod time;
pub mod timing;

pub use bus_intent::{CpuBusIntent, CpuIntentSource, IntentOutcome};
pub use golden::{GoldenRow, GoldenTrace};
pub use machine::{MachineNg, StepRecord};
pub use model::GbModel;
pub use time::{ClockPhase, ClockSpine, Time};
pub use timing::{
    Anchor, Observable, PhaseRule, TimingDomain, TimingEntry, TimingProfile, TimingTable,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn gb_model_exposes_full_hardware_set_and_predicates() {
        assert_eq!(GbModel::ALL.len(), 13);
        assert!(GbModel::DmgB.is_dmg_family());
        assert!(GbModel::Mgb.is_dmg_family());
        assert!(GbModel::Sgb2.is_dmg_family());
        assert!(GbModel::CgbE.is_cgb());
        assert!(GbModel::Agb.is_cgb());
        assert!(!GbModel::Dmg0.is_cgb());
    }

    #[test]
    fn timing_table_lookup_is_declarative_and_model_selected() {
        let dmg = TimingTable::for_model(GbModel::DmgB);
        let cgb = TimingTable::for_model(GbModel::CgbE);

        let dmg_entry = dmg
            .lookup(TimingDomain::PpuInternal, "bg_fetch_tile_high_t1")
            .expect("DMG table must expose named BG fetch high T1 entry");
        assert_eq!(dmg_entry.anchor, Anchor::PpuMode3Start);
        assert_eq!(dmg_entry.observable, Observable::PpuFetchSample);
        assert_eq!(dmg_entry.offset.subphases(), 22 * 4);

        let cgb_dot = cgb
            .lookup(TimingDomain::PpuPublic, "ppu_dot")
            .expect("CGB table must expose PPU public dot cadence");
        assert_eq!(cgb_dot.phase, PhaseRule::EveryCpuT { divisor: 1 });
    }

    #[test]
    fn machine_ng_nop_rom_is_deterministic() {
        let rom = vec![0x00; 0x8000];
        let mut a = MachineNg::from_rom(GbModel::DmgB, &rom).expect("valid ROM loads");
        let mut b = MachineNg::from_rom(GbModel::DmgB, &rom).expect("valid ROM loads");

        let trace_a = a.run_steps(32);
        let trace_b = b.run_steps(32);

        assert_eq!(
            trace_a, trace_b,
            "same ROM/model must produce identical Time sequence"
        );
        assert_eq!(trace_a.first().map(|r| r.time), Some(Time::ZERO));
        assert_eq!(trace_a.last().map(|r| r.time.subphases()), Some(31));
    }

    #[test]
    fn golden_reader_loads_sameboy_tsv_schema_as_typed_rows() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rubc-ng has workspace parent")
            .join("reference/goldens/m3_scy_change_ly000.tsv");
        let trace = GoldenTrace::read_tsv(path).expect("golden TSV parses");

        assert_eq!(trace.rows.len(), 86);
        let write = trace
            .rows
            .iter()
            .find(|row| row.kind == "write" && row.addr == Some(0xFF42) && row.byte == Some(0x03))
            .expect("SCY=03 write row is typed");
        assert_eq!(write.norm_dot, Some(14.0));
        assert_eq!(write.conflict.as_deref(), Some("READ_NEW"));

        let fetch = trace
            .rows
            .iter()
            .find(|row| {
                row.kind == "fetch"
                    && row.state.as_deref() == Some("GET_TILE_DATA_HIGH_T1")
                    && row.io_scy == Some(0x03)
            })
            .expect("HIGH_T1 fetch with new SCY is typed");
        assert_eq!(fetch.norm_dot, Some(22.0));
    }
}
