use crate::cpu::{CpuMode, Regs};

pub const MAGIC: &[u8; 4] = b"RUSV";
pub const VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CpuState {
    pub regs: Regs,
    pub ime: bool,
    pub ime_pending: bool,
    pub ime_delay_boundary: u8,
    pub mode: CpuMode,
    pub exec: crate::cpu::Exec,
    pub halt_bug: bool,
    pub tmp8: u8,
    pub tmp16: u16,
    pub active_cycle: Option<ActiveCpuCycleState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveCpuCycleState {
    pub cycle: crate::cpu::ActiveCpuCycle,
    pub elapsed_t: u8,
}

pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        let mut writer = Self { bytes: Vec::new() };
        writer.bytes.extend_from_slice(MAGIC);
        writer.u16(VERSION);
        writer
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> crate::Result<Self> {
        let mut reader = Self { bytes, pos: 0 };
        let magic = reader.take(4)?;
        anyhow::ensure!(magic == MAGIC, "invalid save-state magic");
        let version = reader.u16()?;
        anyhow::ensure!(version == VERSION, "unsupported save-state version");
        Ok(reader)
    }

    pub fn finish(&self) -> crate::Result<()> {
        anyhow::ensure!(self.pos == self.bytes.len(), "trailing save-state bytes");
        Ok(())
    }

    pub fn u8(&mut self) -> crate::Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> crate::Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => anyhow::bail!("invalid boolean in save state"),
        }
    }

    pub fn u16(&mut self) -> crate::Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn take(&mut self, len: usize) -> crate::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("save-state offset overflow"))?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| anyhow::anyhow!("truncated save state"))?;
        self.pos = end;
        Ok(slice)
    }
}

pub(crate) fn write_regs(writer: &mut Writer, regs: Regs) {
    writer.u8(regs.a);
    writer.u8(regs.f);
    writer.u8(regs.b);
    writer.u8(regs.c);
    writer.u8(regs.d);
    writer.u8(regs.e);
    writer.u8(regs.h);
    writer.u8(regs.l);
    writer.u16(regs.sp);
    writer.u16(regs.pc);
}

pub(crate) fn read_regs(reader: &mut Reader<'_>) -> crate::Result<Regs> {
    Ok(Regs {
        a: reader.u8()?,
        f: reader.u8()? & 0xF0,
        b: reader.u8()?,
        c: reader.u8()?,
        d: reader.u8()?,
        e: reader.u8()?,
        h: reader.u8()?,
        l: reader.u8()?,
        sp: reader.u16()?,
        pc: reader.u16()?,
    })
}
