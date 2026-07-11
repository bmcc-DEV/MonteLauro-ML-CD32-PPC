//! GPU Tile-Based Deferred Renderer — "Lisa II" (modo software-FB).
//!
//! Pipeline real TBDR ainda é futuro. O path de jogos homebrew usa
//! framebuffer software na VRAM (putpixel/rect no guest). Neste modo:
//!   - GPU_CTRL kick = "present" / avanço de VBlank
//!   - frame_count incrementa
//!   - IRQ de VBlank é gerada
//!   - o conteúdo da VRAM (em MemoryMap) NÃO é sobrescrito
//!
//! A VRAM vive em `MemoryMap` — buffer único partilhado com o bus e o SDL.

const FB_WIDTH: u32 = 640;
const FB_HEIGHT: u32 = 480;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuState {
    Idle,
    Presenting,
    VerticalBlank,
}

pub struct Gpu {
    pub state: GpuState,
    pub regs: [u32; 256],
    pub frame_count: u64,
    pub display_enabled: bool,
    /// Offset do framebuffer ativo dentro da VRAM (bytes).
    pub fb_addr: u32,
    pub fb_stride: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Self {
            state: GpuState::Idle,
            regs: [0u32; 256],
            frame_count: 0,
            display_enabled: true,
            fb_addr: 0,
            fb_stride: FB_WIDTH * 4, // RGBA32
        }
    }

    /// Avança o pipeline. Em modo software-FB, kick só gera VBlank.
    pub fn tick(&mut self, _cycles: u32) {
        match self.state {
            GpuState::Idle => {
                if self.regs[0x00] & 1 != 0 {
                    self.regs[0x00] &= !1; // clear kick bit
                    self.state = GpuState::Presenting;
                    log::debug!("GPU: present frame {}", self.frame_count);
                }
            }
            GpuState::Presenting => {
                // Present imediato — não toca no conteúdo da VRAM.
                self.state = GpuState::VerticalBlank;
            }
            GpuState::VerticalBlank => {
                self.frame_count = self.frame_count.wrapping_add(1);
                self.regs[0x10] = self.frame_count as u32;
                self.regs[0x08] = 2; // VBLANK status
                self.regs[0x20] |= 1; // IRQ pending
                self.state = GpuState::Idle;
                self.regs[0x08] = 0; // idle after VBlank edge
            }
        }
    }

    pub fn read_reg(&self, addr: u32) -> Option<u32> {
        let idx = ((addr & 0xFFFF) >> 2) as usize;
        if idx < 256 {
            Some(self.regs[idx])
        } else {
            None
        }
    }

    pub fn write_reg(&mut self, addr: u32, val: u32) {
        let idx = ((addr & 0xFFFF) >> 2) as usize;
        if idx < 256 {
            self.regs[idx] = val;
            // GPU_LIST_ADDR (reg 1) pode ser interpretado como fb base relativo
            if idx == 1 {
                // Lista/FB: se apontar para VRAM base, fb_addr = 0
                let vram_base = 0x0401_0000u32;
                if val >= vram_base {
                    self.fb_addr = val.wrapping_sub(vram_base);
                }
            }
        }
    }

    pub fn fb_width() -> u32 {
        FB_WIDTH
    }

    pub fn fb_height() -> u32 {
        FB_HEIGHT
    }

    /// Tamanho do framebuffer RGBA em bytes.
    pub fn fb_byte_size(&self) -> usize {
        (FB_HEIGHT as usize) * (self.fb_stride as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kick_increments_frame_without_vram() {
        let mut g = Gpu::new();
        g.regs[0] = 1; // kick
        g.tick(1);
        assert_eq!(g.state, GpuState::Presenting);
        g.tick(1);
        assert_eq!(g.state, GpuState::VerticalBlank);
        g.tick(1);
        assert_eq!(g.frame_count, 1);
        assert_eq!(g.regs[0x10], 1);
        assert!(g.regs[0x20] & 1 != 0);
        assert_eq!(g.state, GpuState::Idle);
    }
}
