#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Time(pub u64);

pub const SUBPHASES_PER_T_U8: u8 = 4;
pub const CPU_ACCESS_END_OFFSET: u8 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuAccessPlan {
    pub write_visible_at: Option<u8>,
    pub end: u8,
}

impl CpuAccessPlan {
    pub const fn read_like() -> Self {
        Self {
            write_visible_at: None,
            end: CPU_ACCESS_END_OFFSET,
        }
    }

    pub const fn idle() -> Self {
        Self {
            write_visible_at: None,
            end: CPU_ACCESS_END_OFFSET,
        }
    }

    pub const fn write(offset: u8) -> Self {
        Self {
            write_visible_at: Some(offset),
            end: CPU_ACCESS_END_OFFSET,
        }
    }
}
