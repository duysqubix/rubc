use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! u8_array_module {
    ($name:ident, $len:expr) => {
        pub mod $name {
            use super::*;

            pub fn serialize<S>(value: &[u8; $len], serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                value.as_slice().serialize(serializer)
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; $len], D::Error>
            where
                D: Deserializer<'de>,
            {
                let bytes = Vec::<u8>::deserialize(deserializer)?;
                if bytes.len() != $len {
                    return Err(D::Error::custom(format!(
                        "expected {} bytes, got {}",
                        $len,
                        bytes.len()
                    )));
                }
                let mut out = [0u8; $len];
                out.copy_from_slice(&bytes);
                Ok(out)
            }
        }
    };
}

macro_rules! nested_u8_array_module {
    ($name:ident, $outer:expr, $inner:expr) => {
        pub mod $name {
            use super::*;

            pub fn serialize<S>(
                value: &[[u8; $inner]; $outer],
                serializer: S,
            ) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let banks: Vec<&[u8]> = value.iter().map(|bank| bank.as_slice()).collect();
                banks.serialize(serializer)
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<[[u8; $inner]; $outer], D::Error>
            where
                D: Deserializer<'de>,
            {
                let banks = Vec::<Vec<u8>>::deserialize(deserializer)?;
                if banks.len() != $outer {
                    return Err(D::Error::custom(format!(
                        "expected {} banks, got {}",
                        $outer,
                        banks.len()
                    )));
                }
                let mut out = [[0u8; $inner]; $outer];
                for (idx, bank) in banks.into_iter().enumerate() {
                    if bank.len() != $inner {
                        return Err(D::Error::custom(format!(
                            "expected bank {} to have {} bytes, got {}",
                            idx,
                            $inner,
                            bank.len()
                        )));
                    }
                    out[idx].copy_from_slice(&bank);
                }
                Ok(out)
            }
        }
    };
}

u8_array_module!(u8_64, 0x40);
u8_array_module!(u8_127, 0x7F);
u8_array_module!(u8_128, 0x80);
u8_array_module!(u8_160, 0xA0);
u8_array_module!(u8_8192, 0x2000);
nested_u8_array_module!(u8_2x8192, 2, 0x2000);
nested_u8_array_module!(u8_8x4096, 8, 0x1000);

pub mod boxed_u8_512 {
    use super::*;

    pub fn serialize<S>(value: &[u8; 512], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Box<[u8; 512]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        if bytes.len() != 512 {
            return Err(D::Error::custom(format!(
                "expected 512 bytes, got {}",
                bytes.len()
            )));
        }
        let mut out = Box::new([0u8; 512]);
        out.copy_from_slice(&bytes);
        Ok(out)
    }
}
