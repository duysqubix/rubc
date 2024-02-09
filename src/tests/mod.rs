use crate::utils::format_binary;
use serde::de;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;

fn from_hex<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    u8::from_str_radix(s.trim_start_matches("0x"), 16).map_err(de::Error::custom)
}

fn from_hex_16<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    u16::from_str_radix(s.trim_start_matches("0x"), 16).map_err(de::Error::custom)
}

#[derive(Serialize, Deserialize)]
pub struct CpuState {
    // #[serde(deserialize_with = "from_hex")]
    pub a: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub b: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub c: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub d: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub e: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub f: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub h: u8,

    // #[serde(deserialize_with = "from_hex")]
    pub l: u8,

    // #[serde(deserialize_with = "from_hex_16")]
    pub sp: u16,

    // #[serde(deserialize_with = "from_hex_16")]
    pub pc: u16,
}

impl fmt::Debug for CpuState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A: `{}` F: `{}` B: `{}` C: `{}` D: `{}` E: `{}` H: `{}` L: `{}` SP: `{:0X}` PC: `{:0X}`",
            format_binary(self.a),
            format_binary(self.f),
            format_binary(self.b),
            format_binary(self.c),
            format_binary(self.d),
            format_binary(self.e),
            format_binary(self.h),
            format_binary(self.l),
            self.sp,
            self.pc,
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Object {
    pub name: String,
    pub initial: CpuState,

    #[serde(rename = "final")]
    pub final_state: CpuState,
    // cycles: Vec<Vec<String>>,
}

pub fn read_test_file(file: &str) -> Vec<Object> {
    let data = fs::read_to_string(file).unwrap();
    let tests: Vec<Object> = serde_json::from_str(&data).unwrap();
    tests
}
