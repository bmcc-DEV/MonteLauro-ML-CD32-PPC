//! Bus Interface e arbitragem PPC ↔ ColdFire ↔ GPU.
//!
//! A MIU gere o acesso à RAM unificada de 24MB e aos periféricos.
//! O barramento ColdFire roda a 140MHz e o PPC a 266MHz.

use crate::memory::{MemRegion, MemoryMap};
use crate::gpu::Gpu;
use crate::audio::AudioSubsystem;
use crate::cdrom::CdromDrive;
use crate::dma::DmaController;
use crate::interrupt::{InterruptController, IrqSource};

// Mailbox commands (docs/aros/abi.md)
const CF_CMD_EXEC: u32 = 0x01;
const CF_CMD_IO_READ: u32 = 0x02;
const CF_CMD_IO_WRITE: u32 = 0x03;
const CF_CMD_JOYPAD: u32 = 0x04;
const CF_CMD_CDROM_STATUS: u32 = 0x05;
const CF_CMD_DMA_AUDIO: u32 = 0x06;
const CF_CMD_UART_WRITE: u32 = 0x07;
const CF_CMD_HALT: u32 = 0xFF;

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
}

// ColdFire I/O — registradores de 32 bits (lwz/stw nativos do PPC).
//   0x00: UART_DATA    0x04: UART_STATUS
//   0x10: SPI_DATA
//   0x20: GPIO/JOYPAD  (active-low no hardware)
//   0x30: RTC

struct ColdFireIo {
    regs: [u32; 16],
}

impl ColdFireIo {
    fn new() -> Self {
        let mut regs = [0u32; 16];
        regs[1] = 0x000000C0; // UART status: TX ready
        regs[8] = 0x0000_FFFF; // GPIO default: all released = all high (active-low)
        Self { regs }
    }

    fn read32(&self, offset: u32) -> u32 {
        let idx = (offset >> 2) as usize;
        if idx < 16 { self.regs[idx] } else { 0 }
    }

    fn write32(&mut self, offset: u32, val: u32) {
        let idx = (offset >> 2) as usize;
        if idx >= 16 { return; }
        match idx {
            0 => {
                let byte = val as u8;
                if (0x20..0x7f).contains(&byte) {
                    log::info!("CF UART: '{}'", byte as char);
                }
                self.regs[0] = val;
                self.regs[1] = 0x000000C0;
            }
            _ => { self.regs[idx] = val; }
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

    fn ppc_irq_pending(&self) -> bool;
    fn cf_irq_pending(&self) -> Option<(u8, u8)>;
}

pub struct Bus {
    pub mem: MemoryMap,
    pub gpu: Gpu,
    pub audio: AudioSubsystem,
    pub cdrom: CdromDrive,
    pub mailbox_cmd: u32,
    pub mailbox_resp: u32,
    pub mailbox_status: u32,
    pub mailbox_arg: u32,
    pub miu_cfg: u32,
    pub miu_stat: u32,
    pub miu_arb: u32,
    pub miu_timing: u32,
    cfio: ColdFireIo,
    pub intc: InterruptController,
    pub dma: DmaController,
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
        self.service_mailbox();

        // Salva estado anterior da GPU
        let was_idle = self.gpu.state == crate::gpu::tbdr::GpuState::Idle;
        self.gpu.tick(ppc_cycles);

        // Se acabou de entrar em Presenting (kick foi processado),
        // executa o command buffer da Chip RAM
        if was_idle && self.gpu.state == crate::gpu::tbdr::GpuState::Presenting {
            let list_addr = self.gpu.regs[1]; // GPU_LIST_ADDR
            // Copia comando buffer da sysram antes de pegar vram mutavel
            let cmd_buf = if list_addr < self.mem.unified_ram().len() as u32 {
                let end = (list_addr as usize + 8192).min(self.mem.unified_ram().len());
                self.mem.unified_ram()[list_addr as usize..end].to_vec()
            } else {
                Vec::new()
            };
            let vram = self.mem.vram_mut();
            if !cmd_buf.is_empty() {
                crate::gpu::tbdr::Gpu::exec_dl(&cmd_buf, vram);
            }
            self.gpu.tick(ppc_cycles);
        }

        self.audio.tick(cf_cycles);
        self.cdrom.tick(cf_cycles);

        if self.gpu.regs[0x20] & 1 != 0 {
            self.intc.assert_irq(IrqSource::GpuVBlank);
            self.gpu.regs[0x20] &= !1;
        }
    }

    /// Processa um comando pendente na mailbox (status==1).
    pub fn service_mailbox(&mut self) {
        if self.mailbox_status != 1 {
            return;
        }
        let cmd = self.mailbox_cmd;
        let arg = self.mailbox_arg;
        let resp = match cmd {
            CF_CMD_IO_READ => {
                let off = arg & 0x3F;
                self.cfio.read32(off)
            }
            CF_CMD_IO_WRITE => {
                let off = arg & 0xFF;
                let val = (arg >> 16) & 0xFFFF;
                self.cfio.write32(off, val);
                0
            }
            CF_CMD_JOYPAD => self.cfio.read32(0x20),
            CF_CMD_CDROM_STATUS => {
                if self.cdrom.disc_inserted {
                    1
                } else {
                    0
                }
            }
            CF_CMD_UART_WRITE => {
                let byte = (arg & 0xFF) as u8;
                self.cfio.write32(0x00, byte as u32);
                0
            }
            CF_CMD_EXEC | CF_CMD_DMA_AUDIO => 0,
            CF_CMD_HALT => 0,
            _ => {
                log::debug!("Mailbox: unknown cmd 0x{:02X}", cmd);
                0
            }
        };
        self.mailbox_resp = resp;
        self.mailbox_status = 0;
        log::debug!(
            "Mailbox: CF handled cmd=0x{:02X} arg=0x{:08X} resp=0x{:08X}",
            cmd, arg, resp
        );
    }

    pub fn ppc_mailbox_send(&mut self, cmd: u32, arg: u32) {
        self.mailbox_cmd = cmd;
        self.mailbox_arg = arg;
        self.mailbox_status = 1;
        self.service_mailbox();
    }

    pub fn cf_mailbox_poll(&mut self) -> Option<(u32, u32)> {
        if self.mailbox_status == 1 {
            Some((self.mailbox_cmd, self.mailbox_arg))
        } else {
            None
        }
    }

    pub fn cf_mailbox_respond(&mut self, resp: u32) {
        self.mailbox_resp = resp;
        self.mailbox_status = 0;
    }

    /// `state`: bit set = botão pressionado (API host/SDL).
    /// Hardware GPIO é active-low.
    pub fn set_joypad(&mut self, state: u16) {
        self.cfio.write32(0x20, (!state) as u32);
    }

    pub fn joypad_raw_gpio(&self) -> u16 {
        self.cfio.read32(0x20) as u16
    }

    /// Framebuffer RGBA a partir da VRAM unificada (mesmo buffer do guest).
    pub fn framebuffer_rgba(&self) -> &[u8] {
        let fb = self.gpu.fb_addr as usize;
        let size = self.gpu.fb_byte_size();
        let vram = self.mem.vram();
        let end = (fb + size).min(vram.len());
        if fb >= vram.len() {
            &vram[..0]
        } else {
            &vram[fb..end]
        }
    }
}

impl BusInterface for Bus {
    fn read_byte(&self, addr: u32) -> Option<u8> {
        match self.mem.region(addr) {
            MemRegion::UnifiedRam
            | MemRegion::SystemRam
            | MemRegion::ChipRam
            | MemRegion::ColdFireLocal
            | MemRegion::BootRom
            | MemRegion::Vram => self.mem.read_byte(addr),
            MemRegion::GpuRegs => self.gpu.read_reg(addr).map(|v| v as u8),
            MemRegion::AudioDsp => Some(self.audio.read_byte(addr)),
            MemRegion::CdromRegs => Some(self.cdrom.read_byte(addr)),
            MemRegion::DvdExpansion | MemRegion::Reserved => None,
            MemRegion::MiuRegs => self.read_miu_reg(addr).map(|v| v as u8),
            MemRegion::Mailbox => self.read_mailbox(addr).map(|v| (v & 0xFF) as u8),
            MemRegion::ColdFireIo => {
                let v = self.cfio.read32(addr & 0x3F);
                let shift = (3 - (addr & 3)) << 3;
                Some((v >> shift) as u8)
            }
            MemRegion::DmaRegs => Some((self.dma.read_reg(addr) & 0xFF) as u8),
        }
    }

    fn read_half(&self, addr: u32) -> Option<u16> {
        match self.mem.region(addr) {
            MemRegion::UnifiedRam
            | MemRegion::SystemRam
            | MemRegion::ChipRam
            | MemRegion::ColdFireLocal
            | MemRegion::BootRom
            | MemRegion::Vram => self.mem.read_half(addr),
            MemRegion::GpuRegs => self.gpu.read_reg(addr).map(|v| v as u16),
            MemRegion::AudioDsp => Some(self.audio.read_half(addr)),
            MemRegion::CdromRegs => Some(self.cdrom.read_half(addr)),
            MemRegion::MiuRegs => self.read_miu_reg(addr).map(|v| v as u16),
            MemRegion::Mailbox => self.read_mailbox(addr).map(|v| (v & 0xFFFF) as u16),
            MemRegion::ColdFireIo => {
                let v = self.cfio.read32(addr & 0x3F);
                let shift = (2 - (addr & 2)) << 3;
                Some((v >> shift) as u16)
            }
            MemRegion::DmaRegs => Some((self.dma.read_reg(addr) & 0xFFFF) as u16),
            _ => None,
        }
    }

    fn read_word(&self, addr: u32) -> Option<u32> {
        match self.mem.region(addr) {
            MemRegion::UnifiedRam
            | MemRegion::SystemRam
            | MemRegion::ChipRam
            | MemRegion::ColdFireLocal
            | MemRegion::BootRom
            | MemRegion::Vram => self.mem.read_word(addr),
            MemRegion::GpuRegs => self.gpu.read_reg(addr),
            MemRegion::AudioDsp => self.audio.read_word(addr),
            MemRegion::CdromRegs => self.cdrom.read_word(addr),
            MemRegion::MiuRegs => self.read_miu_reg(addr),
            MemRegion::Mailbox => self.read_mailbox(addr),
            MemRegion::ColdFireIo => Some(self.cfio.read32(addr & 0x3F)),
            MemRegion::DmaRegs => Some(self.dma.read_reg(addr)),
            _ => None,
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) -> Option<()> {
        match self.mem.region(addr) {
            MemRegion::UnifiedRam | MemRegion::SystemRam | MemRegion::ChipRam | MemRegion::Vram => {
                self.mem.write_byte(addr, val)
            }
            MemRegion::GpuRegs => {
                self.gpu.write_reg(addr, val as u32);
                Some(())
            }
            MemRegion::AudioDsp => {
                self.audio.write_byte(addr, val);
                Some(())
            }
            MemRegion::CdromRegs => {
                self.cdrom.write_byte(addr, val);
                Some(())
            }
            MemRegion::MiuRegs => {
                self.write_miu_reg(addr, val as u32);
                Some(())
            }
            MemRegion::Mailbox => {
                self.write_mailbox_byte(addr, val);
                Some(())
            }
            MemRegion::ColdFireIo => {
                let off = addr & 0x3F;
                let shift = (3 - (off & 3)) << 3;
                let mask = !(0xFFu32 << shift);
                let prev = self.cfio.read32(off & !3);
                self.cfio
                    .write32(off & !3, (prev & mask) | ((val as u32) << shift));
                Some(())
            }
            _ => None,
        }
    }

    fn write_half(&mut self, addr: u32, val: u16) -> Option<()> {
        match self.mem.region(addr) {
            MemRegion::UnifiedRam | MemRegion::SystemRam | MemRegion::ChipRam | MemRegion::Vram => {
                self.mem.write_half(addr, val)
            }
            MemRegion::GpuRegs => {
                self.gpu.write_reg(addr, val as u32);
                Some(())
            }
            MemRegion::AudioDsp => {
                self.audio.write_half(addr, val);
                Some(())
            }
            MemRegion::CdromRegs => {
                self.cdrom.write_half(addr, val);
                Some(())
            }
            MemRegion::MiuRegs => {
                self.write_miu_reg(addr, val as u32);
                Some(())
            }
            MemRegion::Mailbox => {
                self.write_mailbox(addr, val as u32);
                Some(())
            }
            MemRegion::ColdFireIo => {
                let off = addr & 0x3F;
                let shift = (2 - (off & 2)) << 3;
                let mask = !(0xFFFFu32 << shift);
                let prev = self.cfio.read32(off & !2);
                self.cfio
                    .write32(off & !2, (prev & mask) | ((val as u32) << shift));
                Some(())
            }
            MemRegion::DmaRegs => {
                self.dma.write_reg(addr, val as u32);
                Some(())
            }
            _ => None,
        }
    }

    fn write_word(&mut self, addr: u32, val: u32) -> Option<()> {
        match self.mem.region(addr) {
            MemRegion::UnifiedRam | MemRegion::SystemRam | MemRegion::ChipRam | MemRegion::Vram => {
                self.mem.write_word(addr, val)
            }
            MemRegion::ColdFireLocal => {
                if cfg!(feature = "allow_cf_local_write") {
                    self.mem.write_word(addr, val)
                } else {
                    log::warn!(
                        "Bus: PPC tried to write to ColdFire local RAM at 0x{:08X}",
                        addr
                    );
                    None
                }
            }
            MemRegion::GpuRegs => {
                self.gpu.write_reg(addr, val);
                Some(())
            }
            MemRegion::AudioDsp => {
                self.audio.write_word(addr, val);
                Some(())
            }
            MemRegion::CdromRegs => {
                self.cdrom.write_word(addr, val);
                Some(())
            }
            MemRegion::MiuRegs => {
                self.write_miu_reg(addr, val);
                Some(())
            }
            MemRegion::Mailbox => {
                self.write_mailbox(addr, val);
                Some(())
            }
            MemRegion::ColdFireIo => {
                self.cfio.write32(addr & 0x3F, val);
                Some(())
            }
            MemRegion::DmaRegs => {
                self.dma.write_reg(addr, val);
                Some(())
            }
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
            MemRegion::UnifiedRam | MemRegion::SystemRam | MemRegion::ChipRam => {
                // RAM unificada: 0 wait p/ PPC, 1 p/ ColdFire
                if is_coldfire {
                    1
                } else {
                    0
                }
            }
            MemRegion::Vram => 3,
            MemRegion::ColdFireIo | MemRegion::Mailbox => 1,
            _ => 0,
        }
    }
}

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
        let offset = addr & 0x0F;
        Some(match offset {
            0x00 => self.mailbox_cmd,
            0x04 => self.mailbox_resp,
            0x08 => self.mailbox_status,
            0x0C => self.mailbox_arg,
            _ => return None,
        })
    }

    fn write_mailbox_byte(&mut self, addr: u32, val: u8) {
        // Guest normalmente usa word; byte write actualiza low byte.
        let offset = addr & 0x0C;
        let prev = self.read_mailbox(offset).unwrap_or(0);
        let shift = (3 - (addr & 3)) << 3;
        let merged = (prev & !(0xFFu32 << shift)) | ((val as u32) << shift);
        self.write_mailbox(offset, merged);
    }

    fn write_mailbox(&mut self, addr: u32, val: u32) {
        let offset = addr & 0x0F;
        match offset {
            0x00 => self.mailbox_cmd = val,
            0x04 => self.mailbox_resp = val, // guest normalmente não escreve
            0x08 => {
                self.mailbox_status = val;
                if val == 1 {
                    // Comando pendente — companion responde de imediato
                    self.service_mailbox();
                }
            }
            0x0C => self.mailbox_arg = val,
            _ => log::warn!("Mailbox: write to reserved offset 0x{:02X}", offset),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::BusInterface;
    use crate::memory::VRAM_BASE;

    #[test]
    fn mailbox_overlay_not_unified_ram() {
        let mut bus = Bus::new(vec![]);
        bus.write_word(0x0100_0000, 0x02).unwrap(); // cmd
        bus.write_word(0x0100_000C, 0x20).unwrap(); // arg = GPIO
        bus.write_word(0x0100_0008, 1).unwrap(); // status pending → service
        assert_eq!(bus.mailbox_status, 0);
        // default GPIO all high (released)
        assert_eq!(bus.mailbox_resp & 0xFFFF, 0xFFFF);
    }

    #[test]
    fn joypad_active_low_via_mailbox() {
        let mut bus = Bus::new(vec![]);
        bus.set_joypad(1 << 4); // A pressed (host API)
        bus.ppc_mailbox_send(CF_CMD_IO_READ, 0x20);
        let raw = bus.mailbox_resp as u16;
        assert_eq!(raw & (1 << 4), 0); // active-low: pressed bit clear
        // Guest: state = ~raw → pressed bit set
        let guest = !raw;
        assert_ne!(guest & (1 << 4), 0);
    }

    #[test]
    fn vram_guest_and_framebuffer_same_buffer() {
        let mut bus = Bus::new(vec![]);
        // Write red pixel at (0,0) as BE word ARGB-ish
        bus.write_word(VRAM_BASE, 0xFFFF_0000).unwrap();
        let fb = bus.framebuffer_rgba();
        assert_eq!(fb[0], 0xFF);
        assert_eq!(fb[1], 0xFF);
        assert_eq!(fb[2], 0x00);
        assert_eq!(fb[3], 0x00);
    }

    #[test]
    fn gpu_kick_does_not_wipe_vram() {
        let mut bus = Bus::new(vec![]);
        bus.write_word(VRAM_BASE, 0xAABB_CCDD).unwrap();
        bus.gpu.regs[0] = 1;
        for _ in 0..4 {
            bus.tick(16, 8);
        }
        assert!(bus.gpu.frame_count >= 1);
        assert_eq!(bus.mem.read_word(VRAM_BASE), Some(0xAABB_CCDD));
    }

    #[test]
    fn unified_24mb_accessible() {
        let mut bus = Bus::new(vec![]);
        bus.write_word(0x017F_FFFC, 0xCAFEBABE).unwrap();
        assert_eq!(bus.read_word(0x017F_FFFC), Some(0xCAFEBABE));
    }
}
