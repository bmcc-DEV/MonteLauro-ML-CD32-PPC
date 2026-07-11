//! GPU Tile-Based Deferred Renderer — "Lisa II"
//!
//! Pipeline: recebe display lists (command buffers) da Chip RAM,
//! processa comandos de desenho em lote, e gera VBlank ao final.
//!
//! Formato do command buffer (cada entrada = 8 bytes big-endian):
//!   u16 opcode; u16 flags; u32 data
//!
//! Comandos:
//!   0x0001 CLEAR  — data = 0x00RRGGBB
//!   0x0002 RECT   — flags = (x:5|y:5|w:5|h:5) ; data = color
//!   0x0003 TRI    — data = pointer p/ {x:16,y:16,color:32} * 3
//!   0x0004 LINE   — flags = (x0:8|y0:8) ; data = (x1:16|y1:16)
//!   0xFFFF END    — fim da lista

const FB_WIDTH: u32 = 640;
const FB_HEIGHT: u32 = 480;

fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([buf[off], buf[off + 1]])
}
fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

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
            fb_stride: FB_WIDTH * 4,
        }
    }

    /// Processa o command buffer, escrevendo os pixels em `vram`.
    pub fn exec_dl(cmd_buf: &[u8], vram: &mut [u8]) {
        let mut off = 0usize;
        loop {
            if off + 8 > cmd_buf.len() { break; }
            let op = read_u16(cmd_buf, off);
            let flags = read_u16(cmd_buf, off + 2);
            let data = read_u32(cmd_buf, off + 4);
            off += 8;

            match op {
                0x0001 => {
                    let color = data | 0xFF000000;
                    let bytes = color.to_be_bytes();
                    for chunk in vram.chunks_mut(4) {
                        if chunk.len() == 4 {
                            chunk.copy_from_slice(&bytes);
                        }
                    }
                }
                0x0002 => {
                    // RECT: flags = (x:6|y:6|w:6|h:6) * 10px cada
                    let x = ((flags >> 10) & 0x3F) as i32 * 10;
                    let y = ((flags >> 5) & 0x1F) as i32 * 10;
                    let w = ((flags >> 10) & 0x3F) as i32 * 10;
                    let h = (flags & 0x1F) as i32 * 10;
                    let color = (data | 0xFF000000).to_be_bytes();
                    for row in y..y + h {
                        for col in x..x + w {
                            if row >= 0 && row < FB_HEIGHT as i32 && col >= 0 && col < FB_WIDTH as i32 {
                                let px = (row as u32 * FB_WIDTH + col as u32) as usize * 4;
                                if px + 4 <= vram.len() {
                                    vram[px..px + 4].copy_from_slice(&color);
                                }
                            }
                        }
                    }
                }
                0x0003 => {
                    let ptr = data as usize;
                    let mut verts = [(0i32, 0i32, 0u32); 3];
                    for i in 0..3 {
                        let voff = ptr + i * 4; // cada vertice = 1 word (x:16|y:16)
                        if voff + 4 > cmd_buf.len() { break; }
                        let vx = read_u16(cmd_buf, voff) as i16 as i32;
                        let vy = read_u16(cmd_buf, voff + 2) as i16 as i32;
                        verts[i as usize] = (vx, vy, 0);
                    }
                    // Cor compartilhada no word apos os 3 vertices
                    let vc = if ptr + 12 < cmd_buf.len() {
                        read_u32(cmd_buf, ptr + 12) | 0xFF000000
                    } else { 0xFFFFFFFF };
                    Self::raster_tri(verts[0], verts[1], verts[2], vc, vram);
                }
                0x0004 => {
                    // LINE: flags = (x0:8|y0:8) ; data = (x1:16|y1:16|color:32)
                    let x0 = (flags >> 8) as i8 as i32;
                    let y0 = (flags as u8) as i8 as i32;
                    let x1 = ((data >> 16) as u16) as i16 as i32;
                    let y1 = (data as u16) as i16 as i32;
                    let color = 0xFFFFFFFFu32.to_be_bytes();
                    let dx = if x1 > x0 { x1 - x0 } else { x0 - x1 };
                    let dy = if y1 > y0 { y1 - y0 } else { y0 - y1 };
                    let sx = if x0 < x1 { 1 } else { -1 };
                    let sy = if y0 < y1 { 1 } else { -1 };
                    let mut err = dx - dy;
                    let (mut cx, mut cy) = (x0, y0);
                    loop {
                        if cx >= 0 && cx < FB_WIDTH as i32 && cy >= 0 && cy < FB_HEIGHT as i32 {
                            let px = (cy as u32 * FB_WIDTH + cx as u32) as usize * 4;
                            if px + 4 <= vram.len() { vram[px..px + 4].copy_from_slice(&color); }
                        }
                        if cx == x1 && cy == y1 { break; }
                        let e2 = err * 2;
                        if e2 > -dy { err -= dy; cx += sx; }
                        if e2 < dx { err += dx; cy += sy; }
                    }
                }
                0xFFFF => break,
                _ => {} // NOP
            }
        }
    }

    fn raster_tri(a: (i32, i32, u32), b: (i32, i32, u32), c: (i32, i32, u32), color: u32, vram: &mut [u8]) {
        let (x0, y0, _) = a; let (x1, y1, _) = b; let (x2, y2, _) = c;
        let col_bytes = (color | 0xFF000000).to_be_bytes();
        let mut minx = x0.min(x1).min(x2).max(0);
        let mut maxx = x0.max(x1).max(x2).min(FB_WIDTH as i32 - 1);
        let mut miny = y0.min(y1).min(y2).max(0);
        let mut maxy = y0.max(y1).max(y2).min(FB_HEIGHT as i32 - 1);
        let sa = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        for y in miny..=maxy {
            for x in minx..=maxx {
                let w0 = (x1 - x0) * (y - y0) - (y1 - y0) * (x - x0);
                let w1 = (x2 - x1) * (y - y1) - (y2 - y1) * (x - x1);
                let w2 = (x0 - x2) * (y - y2) - (y0 - y2) * (x - x2);
                let hit = if sa >= 0 { w0 >= 0 && w1 >= 0 && w2 >= 0 } else { w0 <= 0 && w1 <= 0 && w2 <= 0 };
                if hit {
                    let px = (y as u32 * FB_WIDTH + x as u32) as usize * 4;
                    vram[px..px + 4].copy_from_slice(&col_bytes);
                }
            }
        }
    }

    pub fn tick(&mut self, _cycles: u32) {
        match self.state {
            GpuState::Idle => {
                if self.regs[0x00] & 1 != 0 {
                    self.regs[0x00] &= !1;
                    self.state = GpuState::Presenting;
                }
            }
            GpuState::Presenting => {
                self.state = GpuState::VerticalBlank;
            }
            GpuState::VerticalBlank => {
                self.frame_count = self.frame_count.wrapping_add(1);
                self.regs[0x10] = self.frame_count as u32;
                self.regs[0x08] = 2;
                self.regs[0x20] |= 1;
                self.state = GpuState::Idle;
                self.regs[0x08] = 0;
            }
        }
    }

    pub fn read_reg(&self, addr: u32) -> Option<u32> {
        let idx = ((addr & 0xFFFF) >> 2) as usize;
        if idx < 256 { Some(self.regs[idx]) } else { None }
    }

    pub fn write_reg(&mut self, addr: u32, val: u32) {
        let idx = ((addr & 0xFFFF) >> 2) as usize;
        if idx < 256 {
            self.regs[idx] = val;
            if idx == 1 {
                let vram_base = 0x0401_0000u32;
                if val >= vram_base {
                    self.fb_addr = val.wrapping_sub(vram_base);
                }
            }
        }
    }

    pub fn fb_width() -> u32 { FB_WIDTH }
    pub fn fb_height() -> u32 { FB_HEIGHT }

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
