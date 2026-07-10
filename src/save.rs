//! Save States — serialização do estado completo do emulador.
//!
//! Formato binário:
//!   [0..8]   "CD32SAVE" (magic)
//!   [8..12]  u32 version
//!   [12..16] u32 flags
//!   [16.. ]  dados serializados (seções com header próprio)

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::hardware::Cd32Hardware;
use crate::cpu::ppc603e::Ppu;
use crate::cpu::coldfire::ColdFire;
use crate::bus::Bus;
use crate::audio::AudioSubsystem;
use crate::cdrom::CdromDrive;
use crate::dma::DmaController;
use crate::gpu::tbdr::Gpu;

const MAGIC: &[u8; 8] = b"CD32SAVE";
const VERSION: u32 = 1;

// ── Section IDs ──────────────────────────────────────────────────────

const SEC_PPC_REGS: u8 = 1;
const SEC_CF_REGS: u8 = 2;
const SEC_SYSTEM_RAM: u8 = 3;
const SEC_CHIP_RAM: u8 = 4;
const SEC_COLDFIRE_LOCAL: u8 = 5;
const SEC_VRAM: u8 = 6;
const SEC_GPU: u8 = 7;
const SEC_AUDIO: u8 = 8;
const SEC_CDROM: u8 = 9;
const SEC_DMA: u8 = 10;
const SEC_BUS: u8 = 11;
const SEC_BOOTROM: u8 = 12;

// ── Error ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    BadMagic,
    BadVersion(u32),
    MissingSection(u8),
    SizeMismatch { sec: u8, expected: usize, got: usize },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::BadMagic => write!(f, "not a CD32 save state"),
            Self::BadVersion(v) => write!(f, "unsupported version {}", v),
            Self::MissingSection(s) => write!(f, "missing section {}", s),
            Self::SizeMismatch { sec, expected, got } => 
                write!(f, "section {} size mismatch: expected {} got {}", sec, expected, got),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

// ── Section I/O helpers ──────────────────────────────────────────────

fn write_sec<W: std::io::Write>(w: &mut W, id: u8, data: &[u8]) -> std::io::Result<()> {
    let len = data.len() as u32;
    w.write_all(&[id])?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(data)?;
    Ok(())
}

fn read_sec<R: std::io::Read>(r: &mut R) -> std::io::Result<(u8, Vec<u8>)> {
    let mut id = [0u8; 1];
    r.read_exact(&mut id)?;
    let mut len_bytes = [0u8; 4];
    r.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut data = vec![0u8; len];
    r.read_exact(&mut data)?;
    Ok((id[0], data))
}

// ── Section serializers ──────────────────────────────────────────────

fn save_ppc(ppc: &Ppu) -> Vec<u8> {
    let r = &ppc.regs;
    let mut buf = Vec::with_capacity(256);
    for v in &r.gpr { buf.extend_from_slice(&v.to_be_bytes()); }
    buf.extend_from_slice(&r.pc.to_be_bytes());
    buf.extend_from_slice(&r.lr.to_be_bytes());
    buf.extend_from_slice(&r.ctr.to_be_bytes());
    buf.extend_from_slice(&r.cr.to_be_bytes());
    buf.extend_from_slice(&r.xer.to_be_bytes());
    buf.extend_from_slice(&r.msr.to_be_bytes());
    buf.extend_from_slice(&r.srr0.to_be_bytes());
    buf.extend_from_slice(&r.srr1.to_be_bytes());
    for v in &r.sr { buf.extend_from_slice(&v.to_be_bytes()); }
    buf.extend_from_slice(&r.sdr1.to_be_bytes());
    for v in &r.ibat { buf.extend_from_slice(&v.to_be_bytes()); }
    for v in &r.dbat { buf.extend_from_slice(&v.to_be_bytes()); }
    buf.push(ppc.halt as u8);
    buf
}

fn load_ppc(ppc: &mut Ppu, data: &[u8]) -> Result<(), SaveError> {
    let expected = 32*4 + 8*4 + 16*4 + 4 + 8*4 + 8*4 + 1;
    if data.len() < expected {
        return Err(SaveError::SizeMismatch { sec: SEC_PPC_REGS, expected, got: data.len() });
    }
    let mut off = 0;
    for v in &mut ppc.regs.gpr { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    for v in [&mut ppc.regs.pc, &mut ppc.regs.lr, &mut ppc.regs.ctr, &mut ppc.regs.cr,
              &mut ppc.regs.xer, &mut ppc.regs.msr, &mut ppc.regs.srr0, &mut ppc.regs.srr1] {
        *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    }
    for v in &mut ppc.regs.sr { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    ppc.regs.sdr1 = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    for v in &mut ppc.regs.ibat { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    for v in &mut ppc.regs.dbat { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    ppc.halt = data[off] != 0;
    Ok(())
}

fn save_cf(cf: &ColdFire) -> Vec<u8> {
    let r = &cf.regs;
    let mut buf = Vec::with_capacity(64);
    for v in &r.d { buf.extend_from_slice(&v.to_be_bytes()); }
    for v in &r.a { buf.extend_from_slice(&v.to_be_bytes()); }
    buf.extend_from_slice(&r.pc.to_be_bytes());
    buf.extend_from_slice(&r.sr.to_be_bytes());
    buf.push(cf.halt as u8);
    buf
}

fn load_cf(cf: &mut ColdFire, data: &[u8]) -> Result<(), SaveError> {
    let expected = 8*4 + 8*4 + 4 + 2 + 1;
    if data.len() < expected {
        return Err(SaveError::SizeMismatch { sec: SEC_CF_REGS, expected, got: data.len() });
    }
    let mut off = 0;
    for v in &mut cf.regs.d { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    for v in &mut cf.regs.a { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    cf.regs.pc = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    cf.regs.sr = u16::from_be_bytes(data[off..off+2].try_into().unwrap()); off += 2;
    cf.halt = data[off] != 0;
    Ok(())
}

fn save_mem_region(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

fn load_mem_region(dst: &mut [u8], data: &[u8]) {
    let len = dst.len().min(data.len());
    dst[..len].copy_from_slice(&data[..len]);
}

fn save_gpu(gpu: &Gpu) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    for v in &gpu.regs { buf.extend_from_slice(&v.to_be_bytes()); }
    buf.extend_from_slice(&gpu.frame_count.to_be_bytes());
    buf.extend_from_slice(&gpu.fb_addr.to_be_bytes());
    buf.push(gpu.display_enabled as u8);
    buf
}

fn load_gpu(gpu: &mut Gpu, data: &[u8]) -> Result<(), SaveError> {
    let expected = 256*4 + 8 + 4 + 1;
    if data.len() < expected {
        return Err(SaveError::SizeMismatch { sec: SEC_GPU, expected, got: data.len() });
    }
    let mut off = 0;
    for v in &mut gpu.regs { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    gpu.frame_count = u64::from_be_bytes(data[off..off+8].try_into().unwrap()); off += 8;
    gpu.fb_addr = u32::from_be_bytes(data[off..off+4].try_into().unwrap());
    gpu.display_enabled = data[off+4] != 0;
    Ok(())
}

fn save_audio(_audio: &AudioSubsystem) -> Vec<u8> {
    Vec::new() // audio state é recriado na inicialização
}

fn load_audio(_audio: &mut AudioSubsystem, _data: &[u8]) -> Result<(), SaveError> {
    Ok(())
}

fn save_cdrom(cdrom: &CdromDrive) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    for v in &cdrom.regs { buf.extend_from_slice(&v.to_be_bytes()); }
    buf.extend_from_slice(&cdrom.lba.to_be_bytes());
    buf.extend_from_slice(&cdrom.sectors_remaining.to_be_bytes());
    buf.push(cdrom.state as u8);
    buf.push(cdrom.disc_inserted as u8);
    buf
}

fn load_cdrom(cdrom: &mut CdromDrive, data: &[u8]) -> Result<(), SaveError> {
    let expected = 64*4 + 4 + 4 + 1 + 1;
    if data.len() < expected {
        return Err(SaveError::SizeMismatch { sec: SEC_CDROM, expected, got: data.len() });
    }
    let mut off = 0;
    for v in &mut cdrom.regs { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    cdrom.lba = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    cdrom.sectors_remaining = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    cdrom.disc_inserted = data[off+1] != 0;
    Ok(())
}

fn save_dma(dma: &DmaController) -> Vec<u8> {
    let mut buf = Vec::with_capacity(128);
    for ru in &dma.regs { buf.extend_from_slice(&ru.to_be_bytes()); }
    for ch in &dma.channels {
        buf.extend_from_slice(&ch.src_addr.to_be_bytes());
        buf.extend_from_slice(&ch.dst_addr.to_be_bytes());
        buf.extend_from_slice(&ch.transfer_size.to_be_bytes());
        buf.extend_from_slice(&ch.ctrl.to_be_bytes());
        buf.extend_from_slice(&ch.status.to_be_bytes());
    }
    buf
}

fn load_dma(dma: &mut DmaController, data: &[u8]) -> Result<(), SaveError> {
    let expected = 16*4 + 4*(5*4);
    if data.len() < expected {
        return Err(SaveError::SizeMismatch { sec: SEC_DMA, expected, got: data.len() });
    }
    let mut off = 0;
    for v in &mut dma.regs { *v = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4; }
    for ch in &mut dma.channels {
        ch.src_addr = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
        ch.dst_addr = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
        ch.transfer_size = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
        ch.ctrl = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
        ch.status = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    }
    Ok(())
}

fn save_bus(bus: &Bus) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(&bus.mailbox_cmd.to_be_bytes());
    buf.extend_from_slice(&bus.mailbox_resp.to_be_bytes());
    buf.extend_from_slice(&bus.mailbox_status.to_be_bytes());
    buf.extend_from_slice(&bus.mailbox_arg.to_be_bytes());
    buf.extend_from_slice(&bus.miu_cfg.to_be_bytes());
    buf.extend_from_slice(&bus.miu_stat.to_be_bytes());
    buf.extend_from_slice(&bus.miu_arb.to_be_bytes());
    buf.extend_from_slice(&bus.miu_timing.to_be_bytes());
    buf
}

fn load_bus(bus: &mut Bus, data: &[u8]) -> Result<(), SaveError> {
    let expected = 8*4;
    if data.len() < expected {
        return Err(SaveError::SizeMismatch { sec: SEC_BUS, expected, got: data.len() });
    }
    let mut off = 0;
    bus.mailbox_cmd = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.mailbox_resp = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.mailbox_status = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.mailbox_arg = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.miu_cfg = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.miu_stat = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.miu_arb = u32::from_be_bytes(data[off..off+4].try_into().unwrap()); off += 4;
    bus.miu_timing = u32::from_be_bytes(data[off..off+4].try_into().unwrap());
    Ok(())
}

// ── Public API ───────────────────────────────────────────────────────

pub fn save_state(hw: &Cd32Hardware, path: &Path) -> Result<(), SaveError> {
    let mut file = fs::File::create(path)?;
    file.write_all(MAGIC)?;
    file.write_all(&VERSION.to_be_bytes())?;
    file.write_all(&0u32.to_be_bytes())?; // flags

    write_sec(&mut file, SEC_PPC_REGS, &save_ppc(&hw.ppc))?;
    write_sec(&mut file, SEC_CF_REGS, &save_cf(&hw.coldfire))?;
    write_sec(&mut file, SEC_SYSTEM_RAM, &save_mem_region(&hw.bus.mem.system_ram()))?;
    write_sec(&mut file, SEC_CHIP_RAM, &save_mem_region(hw.bus.mem.chip_ram()))?;
    write_sec(&mut file, SEC_VRAM, &save_mem_region(hw.bus.gpu.vram()))?;
    write_sec(&mut file, SEC_GPU, &save_gpu(&hw.bus.gpu))?;
    write_sec(&mut file, SEC_AUDIO, &save_audio(&hw.bus.audio))?;
    write_sec(&mut file, SEC_CDROM, &save_cdrom(&hw.bus.cdrom))?;
    write_sec(&mut file, SEC_DMA, &save_dma(&hw.bus.dma))?;
    write_sec(&mut file, SEC_BUS, &save_bus(&hw.bus))?;

    Ok(())
}

pub fn load_state(hw: &mut Cd32Hardware, path: &Path) -> Result<(), SaveError> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC { return Err(SaveError::BadMagic); }

    let mut version_bytes = [0u8; 4];
    file.read_exact(&mut version_bytes)?;
    let version = u32::from_be_bytes(version_bytes);
    if version != VERSION { return Err(SaveError::BadVersion(version)); }

    let mut _flags = [0u8; 4];
    file.read_exact(&mut _flags)?;

    let mut sections: Vec<(u8, Vec<u8>)> = Vec::new();
    while let Ok((id, data)) = read_sec(&mut file) {
        sections.push((id, data));
    }

    for (sec_id, sec_data) in &sections {
        match *sec_id {
            SEC_PPC_REGS => load_ppc(&mut hw.ppc, sec_data)?,
            SEC_CF_REGS => load_cf(&mut hw.coldfire, sec_data)?,
            SEC_SYSTEM_RAM => load_mem_region(hw.bus.mem.system_ram_mut(), sec_data),
            SEC_CHIP_RAM => load_mem_region(hw.bus.mem.chip_ram_mut(), sec_data),
            SEC_VRAM => hw.bus.gpu.load_vram(sec_data),
            SEC_GPU => load_gpu(&mut hw.bus.gpu, sec_data)?,
            SEC_AUDIO => load_audio(&mut hw.bus.audio, sec_data)?,
            SEC_CDROM => load_cdrom(&mut hw.bus.cdrom, sec_data)?,
            SEC_DMA => load_dma(&mut hw.bus.dma, sec_data)?,
            SEC_BUS => load_bus(&mut hw.bus, sec_data)?,
            _ => log::warn!("Save state: unknown section {}", sec_id),
        }
    }

    log::info!("Save state loaded from {} ({} sections)", path.display(), sections.len());
    Ok(())
}
