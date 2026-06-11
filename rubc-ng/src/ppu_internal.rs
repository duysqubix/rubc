use crate::golden::{GoldenInitialState, GoldenV2Reader, GoldenV2Row, GoldenVramState, Vram};
use crate::time::Time;
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PpuRegisterWrite {
    time: Time,
    addr: u16,
    value: u8,
}

#[derive(Clone, Debug)]
pub struct PpuInternal {
    lcdc: u8,
    scy: u8,
    scx: u8,
    wx: u8,
    wy: u8,
    vram: Vram,
    oam: [u8; 0xA0],
    fetcher_x: u16,
    current_tile_data_addr: Option<u16>,
    window_active: bool,
    window_line: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowRestart {
    pub trigger_x: usize,
    pub penalty_dots: u8,
    pub scx_low_bits_after_restart: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedSprite {
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub attr: u8,
    pub oam_index: u8,
    pub palette: SpritePalette,
    pub bg_priority: bool,
    pub size: SpriteSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpritePalette {
    Obp0,
    Obp1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteSize {
    Size8x8,
    Size8x16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpritePriorityMode {
    DmgXOrder,
    OamOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedDmgPixel {
    Bg,
    Obj(SpritePalette),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgFetchSample {
    pub raw_tick: u64,
    pub ly: u16,
    pub stage: String,
    pub fetcher_x: u16,
    pub machine_addr: u16,
    pub machine_byte: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgFetchDivergence {
    rom: String,
    index: usize,
    raw_tick: u64,
    ly: u16,
    stage: String,
    machine_addr: u16,
    golden_addr: u16,
    machine_byte: u8,
    golden_byte: u8,
}

impl fmt::Display for BgFetchDivergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "first BG fetch divergence for {} at fetch #{} tick {} LY {} {}: machine addr {:04X}, machine byte {:02X}, golden addr {:04X}, golden byte {:02X}",
            self.rom,
            self.index,
            self.raw_tick,
            self.ly,
            self.stage,
            self.machine_addr,
            self.machine_byte,
            self.golden_addr,
            self.golden_byte
        )
    }
}

impl std::error::Error for BgFetchDivergence {}

impl PpuInternal {
    pub fn from_golden_state(_initial: GoldenInitialState, state: GoldenVramState) -> Self {
        Self {
            lcdc: state.regs.lcdc,
            scy: state.regs.scy,
            scx: state.regs.scx,
            wx: state.regs.wx,
            wy: state.regs.wy,
            vram: state.vram,
            oam: state.oam,
            fetcher_x: 0,
            current_tile_data_addr: None,
            window_active: false,
            window_line: 0,
        }
    }

    pub fn for_test(
        lcdc: u8,
        scy: u8,
        scx: u8,
        wx: u8,
        wy: u8,
        vram: Vram,
        oam: [u8; 0xA0],
    ) -> Self {
        Self {
            lcdc,
            scy,
            scx,
            wx,
            wy,
            vram,
            oam,
            fetcher_x: 0,
            current_tile_data_addr: None,
            window_active: false,
            window_line: 0,
        }
    }

    pub fn trigger_window_for_test(&mut self, ly: u16, screen_x: usize) -> Option<WindowRestart> {
        if self.window_triggered(ly, screen_x) {
            self.window_active = true;
            self.window_line = ly.saturating_sub(u16::from(self.wy)) as u8;
            self.fetcher_x = 0;
            self.current_tile_data_addr = None;
            Some(WindowRestart {
                trigger_x: screen_x,
                penalty_dots: 6,
                scx_low_bits_after_restart: 0,
            })
        } else {
            None
        }
    }

    pub fn fetch_next_tile_for_test(&mut self, ly: u16) -> Result<BgFetchSample, String> {
        let addr = self.tilemap_addr(ly);
        let byte = self.vram.read(addr, 0)?;
        self.current_tile_data_addr = Some(self.tile_data_addr(byte, ly));
        let sample = BgFetchSample {
            raw_tick: 0,
            ly,
            stage: "GET_TILE_T1".to_owned(),
            fetcher_x: self.fetcher_x,
            machine_addr: addr,
            machine_byte: byte,
        };
        self.fetcher_x = self.fetcher_x.wrapping_add(1);
        Ok(sample)
    }

    pub fn select_sprites_for_test(lcdc: u8, oam: &[u8; 0xA0], ly: u8) -> Vec<SelectedSprite> {
        select_scanline_sprites(lcdc, oam, ly)
    }

    pub fn selected_scanline_sprites_for_test(&self, ly: u8) -> Vec<SelectedSprite> {
        select_scanline_sprites(self.lcdc, &self.oam, ly)
    }

    pub fn selected_sprite_for_test(
        y: u8,
        x: u8,
        tile: u8,
        attr: u8,
        oam_index: u8,
    ) -> SelectedSprite {
        let size = SpriteSize::Size8x8;
        SelectedSprite {
            y,
            x,
            tile,
            attr,
            oam_index,
            palette: if attr & 0x10 != 0 {
                SpritePalette::Obp1
            } else {
                SpritePalette::Obp0
            },
            bg_priority: attr & 0x80 != 0,
            size,
        }
    }

    pub fn resolve_dmg_obj_over_bg_for_test(
        bg_color: u8,
        bg_master_priority: bool,
        obj_color: u8,
        sprite: SelectedSprite,
    ) -> ResolvedDmgPixel {
        if obj_color == 0 || (bg_color != 0 && (bg_master_priority || sprite.bg_priority)) {
            ResolvedDmgPixel::Bg
        } else {
            ResolvedDmgPixel::Obj(sprite.palette)
        }
    }

    pub fn sprite_fetch_order_for_test(
        sprites: &[SelectedSprite],
        mode: SpritePriorityMode,
    ) -> Vec<SelectedSprite> {
        let mut fetch = sprites.to_vec();
        match mode {
            SpritePriorityMode::DmgXOrder => {
                fetch.sort_by_key(|sprite| (sprite.x, sprite.oam_index))
            }
            SpritePriorityMode::OamOrder => fetch.sort_by_key(|sprite| sprite.oam_index),
        }
        fetch
    }

    fn write_register(&mut self, write: PpuRegisterWrite) {
        match write.addr {
            0xFF40 => self.lcdc = write.value,
            0xFF42 => self.scy = write.value,
            0xFF43 => self.scx = write.value,
            0xFF4A => self.wy = write.value,
            0xFF4B => self.wx = write.value,
            _ => {}
        }
    }

    fn sample_bg_fetch(&mut self, row: &GoldenV2Row) -> Result<Option<BgFetchSample>, String> {
        if row.kind != "ppu_internal" || row.event.as_deref() != Some("fetch") {
            return Ok(None);
        }
        let stage = row
            .state
            .as_deref()
            .ok_or_else(|| "fetch row missing state".to_owned())?;
        let ly = row.ly.ok_or_else(|| "fetch row missing LY".to_owned())?;
        let (addr, byte) = match stage {
            "GET_TILE_T1" => {
                let addr = self.tilemap_addr(ly);
                let byte = self.vram.read(addr, 0)?;
                self.current_tile_data_addr = Some(self.tile_data_addr(byte, ly));
                (addr, byte)
            }
            "GET_TILE_DATA_LOWER_T1" => {
                let addr = self
                    .current_tile_data_addr
                    .ok_or_else(|| "lower-data fetch before machine tile-id fetch".to_owned())?;
                (addr, self.vram.read(addr, 0)?)
            }
            "GET_TILE_DATA_HIGH_T1" => {
                let addr = self
                    .current_tile_data_addr
                    .ok_or_else(|| "high-data fetch before machine tile-id fetch".to_owned())?
                    .wrapping_add(1);
                let byte = self.vram.read(addr, 0)?;
                self.fetcher_x = self.fetcher_x.wrapping_add(1);
                self.current_tile_data_addr = None;
                (addr, byte)
            }
            _ => return Ok(None),
        };

        Ok(Some(BgFetchSample {
            raw_tick: row.raw_tick,
            ly,
            stage: stage.to_owned(),
            fetcher_x: self.fetcher_x,
            machine_addr: addr,
            machine_byte: byte,
        }))
    }

    fn window_triggered(&self, ly: u16, screen_x: usize) -> bool {
        self.lcdc & 0x20 != 0
            && ly >= u16::from(self.wy)
            && screen_x.saturating_add(7) >= usize::from(self.wx)
    }

    fn tilemap_addr(&self, ly: u16) -> u16 {
        if self.window_active {
            self.window_tilemap_addr()
        } else {
            self.bg_tilemap_addr(ly)
        }
    }

    fn bg_tilemap_addr(&self, ly: u16) -> u16 {
        let base = if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let y = self.scy.wrapping_add(ly as u8);
        let row = u16::from(y >> 3) * 32;
        let col = (u16::from(self.scx >> 3) + self.fetcher_x) & 31;
        base + row + col
    }

    fn window_tilemap_addr(&self) -> u16 {
        let base = if self.lcdc & 0x40 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let row = u16::from(self.window_line >> 3) * 32;
        let col = self.fetcher_x & 31;
        base + row + col
    }

    fn tile_data_addr(&self, tile_id: u8, ly: u16) -> u16 {
        let fine_source_y = if self.window_active {
            self.window_line
        } else {
            self.scy.wrapping_add(ly as u8)
        };
        let fine_y = u16::from(fine_source_y & 7) * 2;
        if self.lcdc & 0x10 != 0 {
            u16::from(tile_id) * 16 + fine_y
        } else {
            (0x1000_i32 + i32::from(tile_id as i8) * 16 + i32::from(fine_y)) as u16
        }
    }
}

fn select_scanline_sprites(lcdc: u8, oam: &[u8; 0xA0], ly: u8) -> Vec<SelectedSprite> {
    let mut selected = Vec::with_capacity(10);
    let size = if lcdc & 0x04 != 0 {
        SpriteSize::Size8x16
    } else {
        SpriteSize::Size8x8
    };
    let height = match size {
        SpriteSize::Size8x8 => 8,
        SpriteSize::Size8x16 => 16,
    };

    for (index, sprite) in oam.chunks_exact(4).enumerate() {
        let y = sprite[0];
        let ly_plus_16 = ly.wrapping_add(16);
        if ly_plus_16 < y || ly_plus_16 >= y.wrapping_add(height) {
            continue;
        }
        selected.push(SelectedSprite {
            y,
            x: sprite[1],
            tile: if size == SpriteSize::Size8x16 {
                sprite[2] & !1
            } else {
                sprite[2]
            },
            attr: sprite[3],
            oam_index: index as u8,
            palette: if sprite[3] & 0x10 != 0 {
                SpritePalette::Obp1
            } else {
                SpritePalette::Obp0
            },
            bg_priority: sprite[3] & 0x80 != 0,
            size,
        });
        if selected.len() == 10 {
            break;
        }
    }
    selected
}

impl Default for SelectedSprite {
    fn default() -> Self {
        Self {
            y: 0,
            x: 0,
            tile: 0,
            attr: 0,
            oam_index: 0,
            palette: SpritePalette::Obp0,
            bg_priority: false,
            size: SpriteSize::Size8x8,
        }
    }
}

pub fn assert_bg_fetch_golden(path: impl AsRef<Path>) -> Result<(), BgFetchDivergence> {
    assert_bg_fetch_golden_inner(path.as_ref(), None)
}

pub fn assert_bg_fetch_golden_with_perturbation(
    path: impl AsRef<Path>,
    vram_addr: u16,
    value: u8,
) -> Result<(), BgFetchDivergence> {
    assert_bg_fetch_golden_inner(path.as_ref(), Some((vram_addr, value)))
}

fn assert_bg_fetch_golden_inner(
    path: &Path,
    perturb_vram: Option<(u16, u8)>,
) -> Result<(), BgFetchDivergence> {
    let rom = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<golden>")
        .to_owned();
    let initial = GoldenV2Reader::read_initial_state(path).map_err(|err| BgFetchDivergence {
        rom: rom.clone(),
        index: 0,
        raw_tick: 0,
        ly: 0,
        stage: err,
        machine_addr: 0,
        golden_addr: 0,
        machine_byte: 0,
        golden_byte: 0,
    })?;
    let mut vram_state =
        GoldenV2Reader::read_vram_state(path).map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index: 0,
            raw_tick: 0,
            ly: 0,
            stage: err,
            machine_addr: 0,
            golden_addr: 0,
            machine_byte: 0,
            golden_byte: 0,
        })?;
    if let Some((addr, value)) = perturb_vram {
        vram_state
            .vram
            .write_for_test(addr, 0, value)
            .map_err(|err| BgFetchDivergence {
                rom: rom.clone(),
                index: 0,
                raw_tick: 0,
                ly: 0,
                stage: err,
                machine_addr: 0,
                golden_addr: 0,
                machine_byte: 0,
                golden_byte: 0,
            })?;
    }
    let writes = extract_register_writes(path).map_err(|err| BgFetchDivergence {
        rom: rom.clone(),
        index: 0,
        raw_tick: 0,
        ly: 0,
        stage: err,
        machine_addr: 0,
        golden_addr: 0,
        machine_byte: 0,
        golden_byte: 0,
    })?;
    let fetch_rows = GoldenV2Reader::open(path)
        .map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index: 0,
            raw_tick: 0,
            ly: 0,
            stage: err,
            machine_addr: 0,
            golden_addr: 0,
            machine_byte: 0,
            golden_byte: 0,
        })?
        .filter_map(|row| match row {
            Ok(row) if row.kind == "ppu_internal" && row.event.as_deref() == Some("fetch") => {
                Some(Ok(row))
            }
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index: 0,
            raw_tick: 0,
            ly: 0,
            stage: err,
            machine_addr: 0,
            golden_addr: 0,
            machine_byte: 0,
            golden_byte: 0,
        })?;

    let mut ppu = PpuInternal::from_golden_state(initial, vram_state);
    let mut next_write = 0;
    for (index, row) in fetch_rows.iter().enumerate() {
        let now = Time::from_subphases(row.raw_tick);
        while writes
            .get(next_write)
            .is_some_and(|write| write.time.subphases() < now.subphases())
        {
            ppu.write_register(writes[next_write]);
            next_write += 1;
        }
        let Some(sample) = ppu.sample_bg_fetch(row).map_err(|err| BgFetchDivergence {
            rom: rom.clone(),
            index,
            raw_tick: row.raw_tick,
            ly: row.ly.unwrap_or(0),
            stage: err,
            machine_addr: 0,
            golden_addr: row.addr.unwrap_or(0),
            machine_byte: 0,
            golden_byte: row.byte.unwrap_or(0),
        })?
        else {
            continue;
        };
        let golden_addr = row.addr.unwrap_or(u16::MAX);
        let golden_byte = row.byte.unwrap_or(u8::MAX);
        if sample.machine_addr != golden_addr || sample.machine_byte != golden_byte {
            return Err(BgFetchDivergence {
                rom: rom.clone(),
                index,
                raw_tick: sample.raw_tick,
                ly: sample.ly,
                stage: sample.stage,
                machine_addr: sample.machine_addr,
                golden_addr,
                machine_byte: sample.machine_byte,
                golden_byte,
            });
        }
    }
    Ok(())
}

fn extract_register_writes(path: &Path) -> Result<Vec<PpuRegisterWrite>, String> {
    let mut writes = GoldenV2Reader::open(path)?
        .filter_map(|row| match row {
            Ok(row) if row.kind == "cpu" => match (row.addr, row.byte) {
                (Some(addr @ (0xFF40 | 0xFF42 | 0xFF43 | 0xFF4A | 0xFF4B)), Some(value)) => {
                    Some(Ok(PpuRegisterWrite {
                        time: Time::from_subphases(row.write_visible_tick.unwrap_or(row.raw_tick)),
                        addr,
                        value,
                    }))
                }
                _ => None,
            },
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    writes.sort_by_key(|write| write.time);
    Ok(writes)
}
