//! Mapa de memória do CD³².
//!
//! Layout:
//!   0x0000_0000 – 0x00FF_FFFF   System RAM (16MB)
//!   0x0100_0000 – 0x013F_FFFF   Chip RAM  (4MB)
//!   0x0200_0000 – 0x021F_FFFF   ColdFire Local Memory (2MB)
//!   0x0220_0000 – 0x0220_003F   ColdFire I/O
//!   0x0300_0000 – 0x03FF_FFFF   CDROM
//!   0x0400_0000 – 0x0400_FFFF   GPU Register File (64KB)
//!   0x0401_0000 – 0x04FF_FFFF   VRAM (8MB)
//!   0xFF00_0000 – 0xFF07_FFFF   Boot ROM / Kickstart (512KB)

const SYSTEM_RAM_BASE: u32 = 0x0000_0000;
const SYSTEM_RAM_SIZE: usize = 16 * 1024 * 1024; // 16MB
const CHIP_RAM_BASE: u32 = 0x0100_0000;
const CHIP_RAM_SIZE: usize = 4 * 1024 * 1024;    // 4MB
const COLDFIRE_LOCAL_BASE: u32 = 0x0200_0000;
const COLDFIRE_LOCAL_SIZE: usize = 2 * 1024 * 1024; // 2MB
const COLDFIRE_IO_BASE: u32 = 0x0220_0000;
const COLDFIRE_IO_SIZE: usize = 0x40;
const CDROM_BASE: u32 = 0x0300_0000;
const CDROM_SIZE: usize = 0x1000;
const GPU_REGS_BASE: u32 = 0x0400_0000;
const GPU_REGS_SIZE: usize = 0x10000; // 64KB
const VRAM_BASE: u32 = 0x0401_0000;
const VRAM_SIZE: usize = 8 * 1024 * 1024; // 8MB
const BOOT_ROM_BASE: u32 = 0xFF00_0000;
const BOOT_ROM_SIZE: usize = 512 * 1024;  // 512KB
const MIU_BASE: u32 = 0x0500_0000;
const MAILBOX_BASE: u32 = 0x0100_0000;     // mailbox nos primeiros bytes da Chip RAM

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemRegion {
    SystemRam,
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
    system_ram: Vec<u8>,
    chip_ram: Vec<u8>,
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
            system_ram: vec![0u8; SYSTEM_RAM_SIZE],
            chip_ram: vec![0u8; CHIP_RAM_SIZE],
            coldfire_local: vec![0u8; COLDFIRE_LOCAL_SIZE],
            boot_rom,
            vram: vec![0u8; VRAM_SIZE],
        }
    }

    pub fn region(&self, addr: u32) -> MemRegion {
        match addr {
            0x0000_0000..=0x00FF_FFFF => MemRegion::SystemRam,
            0x0100_0000..=0x013F_FFFF => MemRegion::ChipRam,
            0x0140_0000..=0x01FF_FFFF => MemRegion::Reserved,
            0x0200_0000..=0x021F_FFFF => MemRegion::ColdFireLocal,
            0x0220_0000..=0x0220_003F => MemRegion::ColdFireIo,
            0x0220_0040..=0x02FF_FFFF => MemRegion::Reserved,
            0x0300_0000..=0x030F_FFFF => MemRegion::CdromRegs,
            0x0400_0000..=0x0400_FFFF => MemRegion::GpuRegs,
            0x03D0_0000..=0x03DF_FFFF => MemRegion::AudioDsp,
            0x03E0_0000..=0x03EF_FFFF => MemRegion::DmaRegs,
            0x0401_0000..=0x04FF_FFFF => MemRegion::Vram,
            0x0500_0000..=0x0500_000F => MemRegion::MiuRegs,
            0x0800_0000..=0x0800_FFFF => MemRegion::DvdExpansion,
            0xFF00_0000..=0xFF07_FFFF => MemRegion::BootRom,
            _ => MemRegion::Reserved,
        }
    }

    // ── Acesso a byte ─────────────────────────────────────────────────

    pub fn read_byte(&self, addr: u32) -> Option<u8> {
        let offset = addr as usize;
        match self.region(addr) {
            MemRegion::SystemRam => self.system_ram.get(offset).copied(),
            MemRegion::ChipRam => self.chip_ram.get(offset & (CHIP_RAM_SIZE - 1)).copied(),
            MemRegion::ColdFireLocal => self.coldfire_local.get(offset & (COLDFIRE_LOCAL_SIZE - 1)).copied(),
            MemRegion::BootRom => self.boot_rom.get(offset & (BOOT_ROM_SIZE - 1)).copied(),
            MemRegion::Vram => self.vram.get(offset & (VRAM_SIZE - 1)).copied(),
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
        let offset = addr as usize;
        match self.region(addr) {
            MemRegion::SystemRam => { self.system_ram[offset] = val; Some(()) }
            MemRegion::ChipRam => { self.chip_ram[offset & (CHIP_RAM_SIZE - 1)] = val; Some(()) }
            MemRegion::ColdFireLocal => { self.coldfire_local[offset & (COLDFIRE_LOCAL_SIZE - 1)] = val; Some(()) }
            MemRegion::Vram => { self.vram[offset & (VRAM_SIZE - 1)] = val; Some(()) }
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

    pub fn system_ram(&self) -> &[u8] {
        &self.system_ram
    }

    pub fn system_ram_mut(&mut self) -> &mut [u8] {
        &mut self.system_ram
    }

    pub fn chip_ram(&self) -> &[u8] {
        &self.chip_ram
    }

    pub fn chip_ram_mut(&mut self) -> &mut [u8] {
        &mut self.chip_ram
    }

    pub fn boot_rom(&self) -> &[u8] {
        &self.boot_rom
    }
}
