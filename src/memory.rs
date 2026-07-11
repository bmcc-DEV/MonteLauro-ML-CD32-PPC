//! Mapa de memória do CD³² (v0.3 — RAM unificada 24MB).
//!
//! Layout:
//!   0x0000_0000 – 0x017F_FFFF   Unified RAM (24MB)
//!   0x0100_0000 – 0x0100_000F   Mailbox (overlay MMIO, 16 bytes)
//!   0x0200_0000 – 0x021F_FFFF   ColdFire Local Memory (2MB)
//!   0x0220_0000 – 0x0220_003F   ColdFire I/O
//!   0x0300_0000 – 0x030F_FFFF   CDROM
//!   0x03D0_0000 – 0x03DF_FFFF   Audio DSP
//!   0x03E0_0000 – 0x03EF_FFFF   DMA
//!   0x0400_0000 – 0x0400_FFFF   GPU Register File (64KB)
//!   0x0401_0000 – 0x0480_FFFF   VRAM (8MB)
//!   0x0500_0000 – 0x0500_000F   MIU
//!   0xFF00_0000 – 0xFF07_FFFF   Boot ROM / Kickstart (512KB)

pub const UNIFIED_RAM_BASE: u32 = 0x0000_0000;
pub const UNIFIED_RAM_SIZE: usize = 24 * 1024 * 1024; // 24MB
pub const UNIFIED_RAM_END: u32 = (UNIFIED_RAM_BASE as usize + UNIFIED_RAM_SIZE - 1) as u32;

pub const MAILBOX_BASE: u32 = 0x0100_0000;
pub const MAILBOX_END: u32 = 0x0100_000F;

const COLDFIRE_LOCAL_SIZE: usize = 2 * 1024 * 1024; // 2MB
pub const VRAM_BASE: u32 = 0x0401_0000;
pub const VRAM_SIZE: usize = 8 * 1024 * 1024; // 8MB
const BOOT_ROM_SIZE: usize = 512 * 1024; // 512KB

/// Stack pointer default: 64KB abaixo do topo da RAM unificada.
pub const DEFAULT_STACK: u32 = 0x017F_0000;

/// Tamanho total reportado na ABI (24MB).
pub const TOTAL_RAM_BYTES: u32 = UNIFIED_RAM_SIZE as u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemRegion {
    /// RAM unificada 24MB (substitui SysRAM+ChipRAM).
    UnifiedRam,
    /// Alias legado — mesmo backing que UnifiedRam.
    SystemRam,
    /// Alias legado — mesmo backing que UnifiedRam.
    ChipRam,
    ColdFireLocal,
    ColdFireIo,
    CdromRegs,
    GpuRegs,
    AudioDsp,
    DmaRegs,
    Vram,
    BootRom,
    MiuRegs,
    Mailbox,
    DvdExpansion,
    Reserved,
}

pub struct MemoryMap {
    unified_ram: Vec<u8>,
    coldfire_local: Vec<u8>,
    boot_rom: Vec<u8>,
    vram: Vec<u8>,
}

impl MemoryMap {
    pub fn new(bios: Vec<u8>) -> Self {
        let mut boot_rom = vec![0u8; BOOT_ROM_SIZE];
        let copy_len = bios.len().min(BOOT_ROM_SIZE);
        boot_rom[..copy_len].copy_from_slice(&bios[..copy_len]);

        Self {
            unified_ram: vec![0u8; UNIFIED_RAM_SIZE],
            coldfire_local: vec![0u8; COLDFIRE_LOCAL_SIZE],
            boot_rom,
            vram: vec![0u8; VRAM_SIZE],
        }
    }

    pub fn region(&self, addr: u32) -> MemRegion {
        // Mailbox overlay tem prioridade dentro da RAM unificada
        if (MAILBOX_BASE..=MAILBOX_END).contains(&addr) {
            return MemRegion::Mailbox;
        }
        match addr {
            0x0000_0000..=0x017F_FFFF => MemRegion::UnifiedRam,
            0x0180_0000..=0x01FF_FFFF => MemRegion::Reserved,
            0x0200_0000..=0x021F_FFFF => MemRegion::ColdFireLocal,
            0x0220_0000..=0x0220_003F => MemRegion::ColdFireIo,
            0x0220_0040..=0x02FF_FFFF => MemRegion::Reserved,
            0x0300_0000..=0x030F_FFFF => MemRegion::CdromRegs,
            0x03D0_0000..=0x03DF_FFFF => MemRegion::AudioDsp,
            0x03E0_0000..=0x03EF_FFFF => MemRegion::DmaRegs,
            0x0400_0000..=0x0400_FFFF => MemRegion::GpuRegs,
            0x0401_0000..=0x0480_FFFF => MemRegion::Vram,
            0x0500_0000..=0x0500_000F => MemRegion::MiuRegs,
            0x0800_0000..=0x0800_FFFF => MemRegion::DvdExpansion,
            0xFF00_0000..=0xFF07_FFFF => MemRegion::BootRom,
            _ => MemRegion::Reserved,
        }
    }

    fn unified_offset(addr: u32) -> Option<usize> {
        let off = addr as usize;
        if off < UNIFIED_RAM_SIZE {
            Some(off)
        } else {
            None
        }
    }

    // ── Acesso a byte ─────────────────────────────────────────────────

    pub fn read_byte(&self, addr: u32) -> Option<u8> {
        match self.region(addr) {
            MemRegion::UnifiedRam | MemRegion::SystemRam | MemRegion::ChipRam => {
                self.unified_ram.get(Self::unified_offset(addr)?).copied()
            }
            MemRegion::ColdFireLocal => {
                let off = (addr as usize) & (COLDFIRE_LOCAL_SIZE - 1);
                self.coldfire_local.get(off).copied()
            }
            MemRegion::BootRom => {
                let off = (addr as usize) & (BOOT_ROM_SIZE - 1);
                self.boot_rom.get(off).copied()
            }
            MemRegion::Vram => {
                let off = (addr.wrapping_sub(VRAM_BASE)) as usize;
                self.vram.get(off).copied()
            }
            _ => None,
        }
    }

    pub fn read_half(&self, addr: u32) -> Option<u16> {
        let b0 = self.read_byte(addr)?;
        let b1 = self.read_byte(addr.wrapping_add(1))?;
        Some(u16::from_be_bytes([b0, b1]))
    }

    pub fn read_word(&self, addr: u32) -> Option<u32> {
        let b0 = self.read_byte(addr)?;
        let b1 = self.read_byte(addr.wrapping_add(1))?;
        let b2 = self.read_byte(addr.wrapping_add(2))?;
        let b3 = self.read_byte(addr.wrapping_add(3))?;
        Some(u32::from_be_bytes([b0, b1, b2, b3]))
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) -> Option<()> {
        match self.region(addr) {
            MemRegion::UnifiedRam | MemRegion::SystemRam | MemRegion::ChipRam => {
                let off = Self::unified_offset(addr)?;
                self.unified_ram[off] = val;
                Some(())
            }
            MemRegion::ColdFireLocal => {
                let off = (addr as usize) & (COLDFIRE_LOCAL_SIZE - 1);
                self.coldfire_local[off] = val;
                Some(())
            }
            MemRegion::Vram => {
                let off = (addr.wrapping_sub(VRAM_BASE)) as usize;
                if off < self.vram.len() {
                    self.vram[off] = val;
                    Some(())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn write_half(&mut self, addr: u32, val: u16) -> Option<()> {
        let [b0, b1] = val.to_be_bytes();
        self.write_byte(addr, b0)?;
        self.write_byte(addr.wrapping_add(1), b1)
    }

    pub fn write_word(&mut self, addr: u32, val: u32) -> Option<()> {
        let [b0, b1, b2, b3] = val.to_be_bytes();
        self.write_byte(addr, b0)?;
        self.write_byte(addr.wrapping_add(1), b1)?;
        self.write_byte(addr.wrapping_add(2), b2)?;
        self.write_byte(addr.wrapping_add(3), b3)
    }

    // ── Accessors ─────────────────────────────────────────────────────

    pub fn unified_ram(&self) -> &[u8] {
        &self.unified_ram
    }

    pub fn unified_ram_mut(&mut self) -> &mut [u8] {
        &mut self.unified_ram
    }

    /// Alias legado: SysRAM = RAM unificada.
    pub fn system_ram(&self) -> &[u8] {
        &self.unified_ram
    }

    pub fn system_ram_mut(&mut self) -> &mut [u8] {
        &mut self.unified_ram
    }

    /// Alias legado: Chip RAM = mesma RAM unificada (sem split físico).
    pub fn chip_ram(&self) -> &[u8] {
        &self.unified_ram
    }

    pub fn chip_ram_mut(&mut self) -> &mut [u8] {
        &mut self.unified_ram
    }

    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }

    pub fn boot_rom(&self) -> &[u8] {
        &self.boot_rom
    }

    pub fn coldfire_local(&self) -> &[u8] {
        &self.coldfire_local
    }

    pub fn coldfire_local_mut(&mut self) -> &mut [u8] {
        &mut self.coldfire_local
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_ram_is_24mb() {
        let m = MemoryMap::new(vec![]);
        assert_eq!(m.unified_ram().len(), 24 * 1024 * 1024);
        assert_eq!(TOTAL_RAM_BYTES, 0x0180_0000);
    }

    #[test]
    fn mailbox_overlay_priority() {
        let m = MemoryMap::new(vec![]);
        assert_eq!(m.region(0x0100_0000), MemRegion::Mailbox);
        assert_eq!(m.region(0x0100_000F), MemRegion::Mailbox);
        assert_eq!(m.region(0x0100_0010), MemRegion::UnifiedRam);
        assert_eq!(m.region(0x0000_0000), MemRegion::UnifiedRam);
        assert_eq!(m.region(0x017F_FFFF), MemRegion::UnifiedRam);
        assert_eq!(m.region(0x0180_0000), MemRegion::Reserved);
    }

    #[test]
    fn unified_ram_roundtrip() {
        let mut m = MemoryMap::new(vec![]);
        m.write_word(0x0000_1000, 0xDEAD_BEEF).unwrap();
        assert_eq!(m.read_word(0x0000_1000), Some(0xDEAD_BEEF));
        // high end of 24MB
        m.write_byte(0x017F_FF00, 0xAB).unwrap();
        assert_eq!(m.read_byte(0x017F_FF00), Some(0xAB));
    }

    #[test]
    fn vram_roundtrip_offset() {
        let mut m = MemoryMap::new(vec![]);
        m.write_word(VRAM_BASE, 0x1122_3344).unwrap();
        assert_eq!(m.read_word(VRAM_BASE), Some(0x1122_3344));
        assert_eq!(m.vram()[0], 0x11);
    }
}
