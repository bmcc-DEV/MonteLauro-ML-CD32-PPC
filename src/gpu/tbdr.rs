//! GPU Tile-Based Deferred Renderer — "Lisa II".
//!
//! A GPU do CD³² usa o modelo tile-based deferred rendering (TBDR), similar
//! ao PowerVR Series2 (Dreamcast) mas com paralelismo interno diferente.
//!
//! Pipeline:
//!   1. Tile Accelerator recebe vértices e agrupa por tile (32x32 pixels)
//!   2. Scene Buffer: lista de primitivas por tile (em Chip RAM)
//!   3. Deferred pass: calcula pixels visíveis (depth test por tile)
//!   4. Rasterização: preenche tiles visíveis pra framebuffer (em VRAM)
//!
//! Esta implementação simula o pipeline em software, rasterizando tiles
//! por demanda.

const VRAM_SIZE: usize = 8 * 1024 * 1024; // 8MB
const TILE_SIZE: u32 = 32;
const FB_WIDTH: u32 = 640;
const FB_HEIGHT: u32 = 480;
const TILES_X: u32 = FB_WIDTH / TILE_SIZE;   // 20
const TILES_Y: u32 = FB_HEIGHT / TILE_SIZE;  // 15

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuState {
    Idle,
    ReceivingTileList,
    ProcessingTile(u32, u32), // (tile_x, tile_y)
    Flushing,
    VerticalBlank,
}

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: u32, // ARGB
}

#[derive(Debug, Clone)]
pub struct Primitive {
    pub vertices: Vec<Vertex>,
    pub kind: PrimKind,
    pub tile_mask: u64, // bitmask of tiles this touches
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimKind {
    Triangle,
    Sprite,
    Rectangle,
}

pub struct Gpu {
    pub vram: Vec<u8>,
    pub state: GpuState,
    pub regs: [u32; 256],
    pub frame_count: u64,
    pub tile_buffers: Vec<Vec<Primitive>>, // indexado por tile_y * TILES_X + tile_x
    // Display
    pub display_enabled: bool,
    pub fb_addr: u32,   // endereço do framebuffer ativo em VRAM
    pub fb_stride: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Self {
            vram: vec![0u8; VRAM_SIZE],
            state: GpuState::Idle,
            regs: [0u8; 256].map(|_| 0u32),
            frame_count: 0,
            tile_buffers: vec![Vec::new(); (TILES_X * TILES_Y) as usize],
            display_enabled: false,
            fb_addr: 0,
            fb_stride: FB_WIDTH * 4, // RGBA32
        }
    }

    pub fn tick(&mut self, _cycles: u32) {
        match self.state {
            GpuState::Idle => {
                // Se o PPC deu kick no render, começa
                if self.regs[0x00] & 1 != 0 {
                    self.state = GpuState::ReceivingTileList;
                    self.regs[0x00] &= !1; // clear kick bit
                    log::info!("GPU: starting render frame {}", self.frame_count);
                }
            }
            GpuState::ReceivingTileList => {
                // Stub: lê a primitive list da Chip RAM (endereço em regs[0x04])
                let _list_addr = self.regs[0x04];
                // Normalmente iteraria e distribuiria por tiles
                self.state = GpuState::Flushing;
            }
            GpuState::Flushing => {
                // Resolve tiles, rasteriza
                self.rasterize_frame();
                self.state = GpuState::VerticalBlank;
            }
            GpuState::VerticalBlank => {
                // VSync period
                self.frame_count += 1;
                self.regs[0x10] = self.frame_count as u32;
                self.state = GpuState::Idle;
                // Gera interrupção de VBlank (regs[0x20] bit 0)
                self.regs[0x20] |= 1;
            }
            GpuState::ProcessingTile(tx, ty) => {
                let _ = (tx, ty);
                self.state = GpuState::Flushing;
            }
        }
    }

    /// Rasteriza todos os tiles acumulados no framebuffer.
    /// Implementação simbólica: preenche com gradiente de teste.
    fn rasterize_frame(&mut self) {
        let fb = self.fb_addr as usize;
        let stride = self.fb_stride as usize;

        for ty in 0..TILES_Y {
            for tx in 0..TILES_X {
                let tile_base_y = ty * TILE_SIZE;
                let tile_base_x = tx * TILE_SIZE;

                for py in 0..TILE_SIZE {
                    for px in 0..TILE_SIZE {
                        let x = tile_base_x + px;
                        let y = tile_base_y + py;
                        if x >= FB_WIDTH || y >= FB_HEIGHT {
                            continue;
                        }
                        let offset = fb + (y as usize) * stride + (x as usize) * 4;
                        if offset + 3 < self.vram.len() {
                            // Padrão de teste: gradiente RGBA
                            let r = (x * 255 / FB_WIDTH) as u8;
                            let g = (y * 255 / FB_HEIGHT) as u8;
                            let b = ((x + y) * 128 / (FB_WIDTH + FB_HEIGHT)) as u8;
                            self.vram[offset + 0] = r;
                            self.vram[offset + 1] = g;
                            self.vram[offset + 2] = b;
                            self.vram[offset + 3] = 0xFF;
                        }
                    }
                }
            }
        }

        log::debug!("GPU: rasterized frame {} ({}x{})", self.frame_count, FB_WIDTH, FB_HEIGHT);
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
        }
    }

    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn load_vram(&mut self, data: &[u8]) {
        let len = self.vram.len().min(data.len());
        self.vram[..len].copy_from_slice(&data[..len]);
    }

    /// Retorna o framebuffer atual como slice de RGBA.
    pub fn framebuffer_rgba(&self) -> &[u8] {
        let fb = self.fb_addr as usize;
        let size = (FB_HEIGHT as usize) * (self.fb_stride as usize);
        &self.vram[fb..fb + size.min(self.vram.len() - fb)]
    }
}
