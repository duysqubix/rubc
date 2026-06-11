#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GbModel {
    Dmg0,
    DmgA,
    DmgB,
    Mgb,
    Sgb,
    Sgb2,
    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    CgbE,
    Agb,
}

impl GbModel {
    pub const ALL: [GbModel; 13] = [
        GbModel::Dmg0,
        GbModel::DmgA,
        GbModel::DmgB,
        GbModel::Mgb,
        GbModel::Sgb,
        GbModel::Sgb2,
        GbModel::Cgb0,
        GbModel::CgbA,
        GbModel::CgbB,
        GbModel::CgbC,
        GbModel::CgbD,
        GbModel::CgbE,
        GbModel::Agb,
    ];

    pub const fn is_cgb(self) -> bool {
        matches!(
            self,
            GbModel::Cgb0
                | GbModel::CgbA
                | GbModel::CgbB
                | GbModel::CgbC
                | GbModel::CgbD
                | GbModel::CgbE
                | GbModel::Agb
        )
    }

    pub const fn is_dmg_family(self) -> bool {
        !self.is_cgb()
    }

    pub const fn is_sgb(self) -> bool {
        matches!(self, GbModel::Sgb | GbModel::Sgb2)
    }

    pub const fn priority_name(self) -> &'static str {
        match self {
            GbModel::Dmg0 => "DMG-0",
            GbModel::DmgA => "DMG-A",
            GbModel::DmgB => "DMG-B",
            GbModel::Mgb => "MGB",
            GbModel::Sgb => "SGB",
            GbModel::Sgb2 => "SGB2",
            GbModel::Cgb0 => "CGB-0",
            GbModel::CgbA => "CGB-A",
            GbModel::CgbB => "CGB-B",
            GbModel::CgbC => "CGB-C",
            GbModel::CgbD => "CGB-D",
            GbModel::CgbE => "CGB-E",
            GbModel::Agb => "AGB",
        }
    }
}
