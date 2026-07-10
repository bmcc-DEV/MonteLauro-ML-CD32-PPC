//! DMA Controller do CD³².
//!
//! 4 canais com prioridade fixa: CDROM > GPU > Audio > ColdFire.
//! Cada canal transfere em bursts de 16 words (64 bytes) por vez.
//! Gerenciado pela MIU — acessa o barramento como master.

use crate::interrupt::IrqSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaChannelId {
    Cdrom = 0,
    Gpu = 1,
    Audio = 2,
    ColdFire = 3,
}

pub enum DmaTransfer {
    Copy { channel: DmaChannelId, src: u32, dst: u32, size: u32 },
    Done(DmaChannelId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DmaState {
    Idle,
    Bursting,
    Done,
}

#[derive(Debug, Clone)]
pub struct DmaChannel {
    pub src_addr: u32,
    pub dst_addr: u32,
    pub transfer_size: u32,     // bytes restantes
    pub burst_size: u32,        // bytes por burst (default 64)
    pub ctrl: u32,              // control register
    pub status: u32,            // status register
    state: DmaState,
    bytes_this_burst: u32,
}

impl DmaChannel {
    fn new() -> Self {
        Self {
            src_addr: 0,
            dst_addr: 0,
            transfer_size: 0,
            burst_size: 64,
            ctrl: 0,
            status: 0,
            state: DmaState::Idle,
            bytes_this_burst: 0,
        }
    }

    fn start(&mut self) {
        if self.transfer_size > 0 && self.src_addr != 0 && self.dst_addr != 0 {
            self.state = DmaState::Bursting;
            self.bytes_this_burst = 0;
            self.status |= 0x01; // busy
            self.status &= !0x02; // clear done
            self.status &= !0x04; // clear error
        }
    }
}

pub struct DmaController {
    pub channels: [DmaChannel; 4],
    pub regs: [u32; 16], // DMA controller registers
}

impl DmaController {
    pub fn new() -> Self {
        Self {
            channels: [
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
                DmaChannel::new(),
            ],
            regs: [0u32; 16],
        }
    }

    /// Processa um tick do DMA. Operação em duas fases:
    /// 1. DMA decide quais transferências fazer.
    /// 2. O tick retorna uma lista de transferências pendentes.
    pub fn tick(&mut self) -> Vec<DmaTransfer> {
        let mut transfers = Vec::new();
        let order = [DmaChannelId::Cdrom, DmaChannelId::Gpu, DmaChannelId::Audio, DmaChannelId::ColdFire];

        for &ch_id in &order {
            let idx = ch_id as usize;
            if self.channels[idx].state != DmaState::Bursting {
                continue;
            }

            let ch = &mut self.channels[idx];
            let remaining_in_burst = ch.burst_size - ch.bytes_this_burst;
            let chunk = remaining_in_burst.min(ch.transfer_size).min(4);

            if chunk == 0 {
                if ch.transfer_size == 0 {
                    ch.state = DmaState::Done;
                    ch.status = (ch.status & !0x01) | 0x02;
                    transfers.push(DmaTransfer::Done(ch_id));
                    continue;
                }
                ch.bytes_this_burst = 0;
                continue;
            }

            transfers.push(DmaTransfer::Copy {
                channel: ch_id,
                src: ch.src_addr,
                dst: ch.dst_addr,
                size: chunk,
            });

            ch.src_addr = ch.src_addr.wrapping_add(4);
            ch.dst_addr = ch.dst_addr.wrapping_add(4);
            ch.transfer_size = ch.transfer_size.saturating_sub(4);
            ch.bytes_this_burst += 4;
        }

        transfers
    }

    pub fn mark_error(&mut self, ch_id: DmaChannelId) {
        let idx = ch_id as usize;
        self.channels[idx].state = DmaState::Done;
        self.channels[idx].status = (self.channels[idx].status & !0x01) | 0x04;
    }

    pub fn write_reg(&mut self, addr: u32, val: u32) {
        let offset = (addr & 0xFF) as usize;

        // Each channel has 4 regs: src(0), dst(4), size(8), ctrl(C)
        if offset < 0x40 {
            let ch_idx = offset / 0x10;
            let reg_off = offset % 0x10;
            if ch_idx < 4 {
                match reg_off {
                    0x00 => self.channels[ch_idx].src_addr = val,
                    0x04 => self.channels[ch_idx].dst_addr = val,
                    0x08 => self.channels[ch_idx].transfer_size = val,
                    0x0C => {
                        self.channels[ch_idx].ctrl = val;
                        if val & 1 != 0 {
                            self.channels[ch_idx].start();
                        }
                        if val & 2 != 0 {
                            self.channels[ch_idx].state = DmaState::Idle;
                            self.channels[ch_idx].status = 0;
                        }
                    }
                    _ => {}
                }
            }
        } else if offset < 0x50 {
            let reg_idx = (offset - 0x40) / 4;
            if reg_idx < 4 {
                self.regs[reg_idx] = val;
            }
        }
    }

    pub fn read_reg(&self, addr: u32) -> u32 {
        let offset = (addr & 0xFF) as usize;
        if offset < 0x40 {
            let ch_idx = offset / 0x10;
            let reg_off = offset % 0x10;
            if ch_idx < 4 {
                match reg_off {
                    0x00 => self.channels[ch_idx].src_addr,
                    0x04 => self.channels[ch_idx].dst_addr,
                    0x08 => self.channels[ch_idx].transfer_size,
                    0x0C => self.channels[ch_idx].status,
                    _ => 0,
                }
            } else {
                0
            }
        } else if offset < 0x50 {
            let reg_idx = (offset - 0x40) / 4;
            if reg_idx < 4 { self.regs[reg_idx] } else { 0 }
        } else {
            0
        }
    }
}
