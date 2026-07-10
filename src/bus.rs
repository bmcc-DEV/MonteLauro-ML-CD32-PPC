//! Bus Interface e arbitragem PPC ↔ ColdFire ↔ GPU.
//!
//! A MIU (Memory Interface Unit) gerencia o acesso concorrente aos 20MB de RAM
//! e periféricos. O barramento ColdFire roda a 140MHz e o PPC a 266MHz.
//! Acessos conflitantes são resolvidos por prioridade fixa (ver memory_map.md).

use std::sync::atomic::{AtomicU32, Ordering};

use crate::memory::{MemRegion, MemoryMap};
use crate::gpu::Gpu;
use crate::audio::AudioSubsystem;
use crate::cdrom::CdromDrive;
use crate::dma::DmaController;
use crate::interrupt::{InterruptController, IrqSource};

const CFIO_BASE: u32 = 0x0220_0000;
const CFIO_SIZE: u32 = 0x40;

// ── DVD Expansion Slot ──────────────────────────────────────────────

pub struct DvdExpansion {
    pub present: bool,
    pub regs: [u32; 16],
    pub data: Option<Vec<u8>>,
}

impl DvdExpansion {
    fn new() -> Self {
        Self { present: false, regs: [0u32; 16], data: None }
    }

    pub fn insert(&mut self, data: Vec<u8>) {
        let len = data.len();
        self.present = true;
        self.data = Some(data);
        self.regs[0] = 0x01;
        log::info!("DVD: disc inserted ({} bytes)", len);
    }

    pub fn eject(&mut self) {
        self.present = false;
        self.data = None;
        self.regs[0] = 0x00;
        log::info!("DVD: ejected");
    }

    fn read_reg(&self, addr: u32) -> u32 {
        let idx = ((addr & 0xFFFF) >> 2) as usize;
        if idx < 16 { self.regs[idx] } else { 0 }
    }

    fn write_reg(&mut self, addr: u32, val: u32) {
        let idx = ((addr & 0xFFFF) >> 2) as usize;
        if idx < 16 { self.regs[idx] = val; }
    }
}

// ColdFire I/O — registradores de 32 bits (lwz/stw nativos do PPC).
// Mapeamento por slot de 4 bytes:
//   0x00: UART_DATA    0x04: UART_STATUS
//   0x10: SPI_DATA     0x14: (reserved)
//   0x20: GPIO/JOYPAD  0x24: (reserved)
//   0x30: RTC          0x34: (reserved)

struct ColdFireIo {
    regs: [AtomicU32; 16],
}

impl ColdFireIo {
    fn new() -> Self {
        let mut regs: [AtomicU32; 16] = Default::default();
        regs[1] = AtomicU32::new(0x000000C0); // UART status: TX ready
        Self { regs }
    }

    fn read32(&self, offset: u32) -> u32 {
        let idx = (offset >> 2) as usize;
        if idx < 16 { self.regs[idx].load(Ordering::Relaxed) } else { 0 }
    }

    fn write32(&mut self, offset: u32, val: u32) {
        let idx = (offset >> 2) as usize;
        if idx >= 16 { return; }
        match idx {
            0 => {
                let byte = val as u8;
                if byte >= 0x20 && byte < 0x7f {
                    log::info!("CF UART: '{}'", byte as char);
                }
                self.regs[0].store(val, Ordering::Relaxed);
                self.regs[1].store(0x000000C0, Ordering::Relaxed);
            }
            1 => self.regs[1].store(val, Ordering::Relaxed),
            4 => self.regs[4].store(val, Ordering::Relaxed),
            8 => self.regs[8].store(val, Ordering::Relaxed),
            12 => self.regs[12].store(val, Ordering::Relaxed),
            _ => log::warn!("CF I/O: write32 to unknown idx {}", idx),
        }
    }
}

pub trait BusInterface {
    fn read_byte(&self, addr: u32) -> Option<u8>;
    fn read_half(&self, addr: u32) -> Option<u16>;
    fn read_word(&self, addr: u32) -> Option<u32>;
    fn write_byte(&mut self, addr: u32, val: u8) -> Option<()>;
    fn write_half(&mut self, addr: u32, val: u16) -> Option<()>;
    fn write_word(&mut self, addr: u32, val: u32) -> Option<()>;

    /// Ciclos de wait-state que a MIU insere para este acesso.
    fn access_cycles(&self, addr: u32, is_coldfire: bool) -> u32;

    /// Interrupt interface
    fn ppc_irq_pending(&self) -> bool;
    fn cf_irq_pending(&self) -> Option<(u8, u8)>;
}

pub struct Bus {
    pub mem: MemoryMap,
    pub gpu: Gpu,
    pub audio: AudioSubsystem,
    pub cdrom: CdromDrive,
    // Mailbox entre PPC e ColdFire
    pub mailbox_cmd: u32,
    pub mailbox_resp: u32,
    pub mailbox_status: u32,
    pub mailbox_arg: u32,
    // MIU registers
    pub miu_cfg: u32,
    pub miu_stat: u32,
    pub miu_arb: u32,
    pub miu_timing: u32,
    // ColdFire I/O peripherals
    cfio: ColdFireIo,
    pub intc: InterruptController,
    pub dma: DmaController,
    // DVD expansion slot (opcional, mapeado em 0x0800_0000)
    pub dvd: DvdExpansion,
}

impl Bus {
    pub fn new(bios: Vec<u8>) -> Self {
        Self {
            mem: MemoryMap::new(bios),
            gpu: Gpu::new(),
            audio: AudioSubsystem::new(),
            cdrom: CdromDrive::new(),
            mailbox_cmd: 0,
            mailbox_resp: 0,
            mailbox_status: 0,
            mailbox_arg: 0,
            miu_cfg: 0,
            miu_stat: 0,
            miu_arb: 0,
            miu_timing: 0,
            cfio: ColdFireIo::new(),
            intc: InterruptController::new(),
            dma: DmaController::new(),
            dvd: DvdExpansion::new(),
        }
    }

    pub fn tick(&mut self, ppc_cycles: u32, cf_cycles: u32) {
        self.gpu.tick(ppc_cycles);
        self.audio.tick(cf_cycles);
        self.cdrom.tick(cf_cycles);
        // Interrupts gerados pelos periféricos
        if self.gpu.regs[0x20] & 1 != 0 {
            self.intc.assert_irq(IrqSource::GpuVBlank);
            self.gpu.regs[0x20] &= !1; // clear after asserting
        }
    }

    /// O PPC pode pôr um comando na mailbox.
    pub fn ppc_mailbox_send(&mut self, cmd: u32, arg: u32) {
        self.mailbox_cmd = cmd;
        self.mailbox_arg = arg;
        self.mailbox_status = 1; // pending
        log::debug!("Mailbox: PPC → CF cmd=0x{:02X} arg=0x{:08X}", cmd, arg);
    }

    /// O ColdFire lê a mailbox e responde.
    pub fn cf_mailbox_poll(&mut self) -> Option<(u32, u32)> {
        if self.mailbox_status == 1 {
            self.mailbox_status = 2; // being read
            Some((self.mailbox_cmd, self.mailbox_arg))
        } else {
            None
        }
    }

    pub fn cf_mailbox_respond(&mut self, resp: u32) {
        self.mailbox_resp = resp;
        self.mailbox_status = 0; // done
        log::debug!("Mailbox: CF → PPC resp=0x{:08X}", resp);
    }

    pub fn set_joypad(&mut self, state: u16) {
        self.cfio.write32(0x20, state as u32);
    }
}

impl BusInterface for Bus {
    fn read_byte(&self, addr: u32) -> Option<u8> {
        match self.mem.region(addr) {
            MemRegion::SystemRam | MemRegion::ChipRam | MemRegion::ColdFireLocal
            | MemRegion::BootRom => self.mem.read_byte(addr),
            MemRegion::GpuRegs => self.gpu.read_reg(addr).map(|v| v as u8),
            MemRegion::AudioDsp => Some(self.audio.read_byte(addr)),
            MemRegion::CdromRegs => Some(self.cdrom.read_byte(addr)),
            MemRegion::DvdExpansion | MemRegion::Reserved => None,
            MemRegion::MiuRegs => self.read_miu_reg(addr).map(|v| v as u8),
            MemRegion::Mailbox => self.read_mailbox(addr).map(|v| v as u8),
            MemRegion::Vram => self.mem.read_byte(addr),
            MemRegion::ColdFireIo => {
                let v = self.cfio.read32(addr & 0x3F);
                let shift = (addr & 3) << 3;
                Some((v >> shift) as u8)
            }
            MemRegion::DmaRegs => Some((self.dma.read_reg(addr) & 0xFF) as u8),
            _ => None,
        }
    }

    fn read_half(&self, addr: u32) -> Option<u16> {
        if addr & 1 != 0 { return None; }
        match self.mem.region(addr) {
            MemRegion::SystemRam | MemRegion::ChipRam | MemRegion::ColdFireLocal
            | MemRegion::BootRom => self.mem.read_half(addr),
            MemRegion::GpuRegs => self.gpu.read_reg(addr).map(|v| v as u16),
            MemRegion::AudioDsp => Some(self.audio.read_half(addr)),
            MemRegion::CdromRegs => Some(self.cdrom.read_half(addr)),
            MemRegion::MiuRegs => self.read_miu_reg(addr).map(|v| v as u16),
            MemRegion::Mailbox => self.read_mailbox(addr).map(|v| v as u16),
            MemRegion::Vram => self.mem.read_half(addr),
            MemRegion::ColdFireIo => {
                let v = self.cfio.read32(addr & 0x3F);
                let shift = (addr & 2) << 3;
                Some((v >> shift) as u16)
            }
            MemRegion::DmaRegs => Some((self.dma.read_reg(addr) & 0xFFFF) as u16),
            _ => None,
        }
    }

    fn read_word(&self, addr: u32) -> Option<u32> {
        if addr & 3 != 0 { return None; }
        match self.mem.region(addr) {
            MemRegion::SystemRam | MemRegion::ChipRam | MemRegion::ColdFireLocal
            | MemRegion::BootRom => self.mem.read_word(addr),
            MemRegion::GpuRegs => self.gpu.read_reg(addr),
            MemRegion::AudioDsp => self.audio.read_word(addr),
            MemRegion::CdromRegs => self.cdrom.read_word(addr),
            MemRegion::MiuRegs => self.read_miu_reg(addr),
            MemRegion::Mailbox => self.read_mailbox(addr),
            MemRegion::Vram => self.mem.read_word(addr),
            MemRegion::ColdFireIo => {
                Some(self.cfio.read32(addr & 0x3F))
            }
            MemRegion::DmaRegs => Some(self.dma.read_reg(addr)),
            _ => None,
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) -> Option<()> {
        match self.mem.region(addr) {
            MemRegion::SystemRam | MemRegion::ChipRam => self.mem.write_byte(addr, val),
            MemRegion::GpuRegs => { self.gpu.write_reg(addr, val as u32); Some(()) }
            MemRegion::AudioDsp => { self.audio.write_byte(addr, val); Some(()) }
            MemRegion::CdromRegs => { self.cdrom.write_byte(addr, val); Some(()) }
            MemRegion::MiuRegs => { self.write_miu_reg(addr, val as u32); Some(()) }
            MemRegion::Mailbox => { self.write_mailbox(addr, val as u32); Some(()) }
            MemRegion::Vram => self.mem.write_byte(addr, val),
            MemRegion::ColdFireIo => {
                let off = addr & 0x3F;
                let shift = (off & 3) << 3;
                let mask = !(0xFFu32 << shift);
                let prev = self.cfio.read32(off & !3);
                self.cfio.write32(off & !3, (prev & mask) | ((val as u32) << shift));
                Some(())
            }
            _ => None,
        }
    }

    fn write_half(&mut self, addr: u32, val: u16) -> Option<()> {
        if addr & 1 != 0 { return None; }
        match self.mem.region(addr) {
            MemRegion::SystemRam | MemRegion::ChipRam => self.mem.write_half(addr, val),
            MemRegion::GpuRegs => { self.gpu.write_reg(addr, val as u32); Some(()) }
            MemRegion::AudioDsp => { self.audio.write_half(addr, val); Some(()) }
            MemRegion::CdromRegs => { self.cdrom.write_half(addr, val); Some(()) }
            MemRegion::MiuRegs => { self.write_miu_reg(addr, val as u32); Some(()) }
            MemRegion::Mailbox => { self.write_mailbox(addr, val as u32); Some(()) }
            MemRegion::Vram => self.mem.write_half(addr, val),
            MemRegion::ColdFireIo => {
                let off = addr & 0x3F;
                let shift = (off & 2) << 3;
                let mask = !(0xFFFFu32 << shift);
                let prev = self.cfio.read32(off & !2);
                self.cfio.write32(off & !2, (prev & mask) | ((val as u32) << shift));
                Some(())
            }
            MemRegion::DmaRegs => { self.dma.write_reg(addr, val as u32); Some(()) }
            _ => None,
        }
    }

    fn write_word(&mut self, addr: u32, val: u32) -> Option<()> {
        if addr & 3 != 0 { return None; }
        match self.mem.region(addr) {
            MemRegion::SystemRam | MemRegion::ChipRam => self.mem.write_word(addr, val),
            MemRegion::ColdFireLocal => {
                // ColdFire local RAM — writable only by ColdFire
                if cfg!(feature = "allow_cf_local_write") {
                    self.mem.write_word(addr, val)
                } else {
                    log::warn!("Bus: PPC tried to write to ColdFire local RAM at 0x{:08X}", addr);
                    None
                }
            }
            MemRegion::GpuRegs => { self.gpu.write_reg(addr, val); Some(()) }
            MemRegion::AudioDsp => { self.audio.write_word(addr, val); Some(()) }
            MemRegion::CdromRegs => { self.cdrom.write_word(addr, val); Some(()) }
            MemRegion::MiuRegs => { self.write_miu_reg(addr, val); Some(()) }
            MemRegion::Mailbox => { self.write_mailbox(addr, val); Some(()) }
            MemRegion::Vram => self.mem.write_word(addr, val),
            MemRegion::ColdFireIo => {
                self.cfio.write32(addr & 0x3F, val);
                Some(())
            }
            MemRegion::DmaRegs => { self.dma.write_reg(addr, val); Some(()) }
            _ => None,
        }
    }

    fn ppc_irq_pending(&self) -> bool {
        self.intc.ppc_irq_pending()
    }

    fn cf_irq_pending(&self) -> Option<(u8, u8)> {
        self.intc.cf_irq_pending()
    }

    fn access_cycles(&self, addr: u32, is_coldfire: bool) -> u32 {
        match self.mem.region(addr) {
            MemRegion::ChipRam => {
                if is_coldfire { 0 } else { 1 }
            }
            MemRegion::SystemRam => {
                if is_coldfire { 2 } else { 0 }
            }
            MemRegion::Vram => 3, // VRAM é lenta, sempre paga 3 wait-states
            MemRegion::ColdFireIo => 1,
            _ => 0,
        }
    }
}

// ── MIU & Mailbox register access ──────────────────────────────────────

impl Bus {
    fn read_miu_reg(&self, addr: u32) -> Option<u32> {
        let offset = addr & 0x0F;
        Some(match offset {
            0x00 => self.miu_cfg,
            0x04 => self.miu_stat,
            0x08 => self.miu_arb,
            0x0C => self.miu_timing,
            _ => return None,
        })
    }

    fn write_miu_reg(&mut self, addr: u32, val: u32) {
        let offset = addr & 0x0F;
        match offset {
            0x00 => self.miu_cfg = val,
            0x08 => self.miu_arb = val,
            0x0C => self.miu_timing = val,
            _ => log::warn!("MIU: write to reserved offset 0x{:02X}", offset),
        }
    }

    fn read_mailbox(&self, addr: u32) -> Option<u32> {
        // Mailbox mapeado nos primeiros 16 bytes da Chip RAM (0x0100_0000)
        let offset = addr & 0x0F;
        Some(match offset {
            0x00 => self.mailbox_cmd,
            0x04 => self.mailbox_resp,
            0x08 => self.mailbox_status,
            0x0C => self.mailbox_arg,
            _ => return None,
        })
    }

    fn write_mailbox(&mut self, addr: u32, val: u32) {
        let offset = addr & 0x0F;
        match offset {
            0x00 => self.mailbox_cmd = val,
            0x08 => self.mailbox_status = val,
            0x0C => self.mailbox_arg = val,
            _ => log::warn!("Mailbox: write to reserved offset 0x{:02X}", offset),
        }
    }
}
