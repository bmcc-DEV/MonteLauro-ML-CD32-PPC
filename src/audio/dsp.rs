//! Áudio do CD³²: DSP + ColdFire co-processamento.
//!
//! O CD³² não tem chip de áudio dedicado (polêmica conhecida na comunidade).
//! O áudio é processado pelo ColdFire em conjunto com um DSP interno que
//! faz mixagem de canais, reamostragem e efeitos.
//!
//! Canais: 8 (estéreo, 16-bit, 44.1kHz)
//! FIFO: 2KB por canal, preenchido via DMA pela Chip RAM.

const NUM_CHANNELS: usize = 8;
const FIFO_SIZE: usize = 2048;
const SAMPLE_RATE: u32 = 44100;

#[derive(Debug, Clone)]
pub struct AudioChannel {
    fifo: Vec<i16>,
    rd_ptr: usize,
    wr_ptr: usize,
    enabled: bool,
    volume: u16,       // 0..1024
    pan: u8,           // 0..255 (0=esquerda, 255=direita)
    sample_pos: u32,    // posição atual na waveform (para streaming)
    loop_enabled: bool,
}

impl AudioChannel {
    fn new() -> Self {
        Self {
            fifo: vec![0i16; FIFO_SIZE],
            rd_ptr: 0,
            wr_ptr: 0,
            enabled: false,
            volume: 1024,
            pan: 128,
            sample_pos: 0,
            loop_enabled: false,
        }
    }

    fn push_sample(&mut self, s: i16) {
        self.fifo[self.wr_ptr] = s;
        self.wr_ptr = (self.wr_ptr + 1) % FIFO_SIZE;
    }

    fn read_sample(&mut self) -> i16 {
        let s = self.fifo[self.rd_ptr];
        if self.rd_ptr != self.wr_ptr {
            self.rd_ptr = (self.rd_ptr + 1) % FIFO_SIZE;
        }
        s
    }
}

pub struct AudioSubsystem {
    channels: Vec<AudioChannel>,
    master_volume: u16,
    output_l: i16,
    output_r: i16,
    dsp_regs: [u32; 64],  // DSP control registers
    irq_pending: bool,
}

impl AudioSubsystem {
    pub fn new() -> Self {
        Self {
            channels: (0..NUM_CHANNELS).map(|_| AudioChannel::new()).collect(),
            master_volume: 1024,
            output_l: 0,
            output_r: 0,
            dsp_regs: [0u8; 64].map(|_| 0u32),
            irq_pending: false,
        }
    }

    pub fn tick(&mut self, _cycles: u32) {
        // A cada 44.1k ciclos do ColdFire, gera uma amostra
        // (simplificado: ColdFire a 140MHz → ~3175 ciclos por sample)
        let _sample_clock = 140_000_000 / SAMPLE_RATE;

        // Mixa todos os canais ativos
        let mut mix_l: i32 = 0;
        let mut mix_r: i32 = 0;

        for ch in &mut self.channels {
            if !ch.enabled {
                continue;
            }
            let s = ch.read_sample() as i32;
            let vol = ch.volume as i32;
            let s_vol = s * vol / 1024;
            let pan_l = (255 - ch.pan as i32) * s_vol / 255;
            let pan_r = (ch.pan as i32) * s_vol / 255;
            mix_l += pan_l;
            mix_r += pan_r;
        }

        // Master volume + clipping
        mix_l = mix_l * self.master_volume as i32 / 1024;
        mix_r = mix_r * self.master_volume as i32 / 1024;
        self.output_l = mix_l.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.output_r = mix_r.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }

    pub fn push_sample(&mut self, channel: usize, sample: i16) {
        if channel < NUM_CHANNELS {
            self.channels[channel].push_sample(sample);
        }
    }

    pub fn set_channel_enabled(&mut self, channel: usize, enabled: bool) {
        if channel < NUM_CHANNELS {
            self.channels[channel].enabled = enabled;
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        let idx = ((addr & 0xFF) >> 2) as usize;
        if idx < 64 { (self.dsp_regs[idx] & 0xFF) as u8 } else { 0 }
    }

    pub fn read_half(&self, addr: u32) -> u16 {
        let idx = ((addr & 0xFF) >> 2) as usize;
        if idx < 64 { (self.dsp_regs[idx] & 0xFFFF) as u16 } else { 0 }
    }

    pub fn read_word(&self, addr: u32) -> Option<u32> {
        let idx = ((addr & 0xFF) >> 2) as usize;
        if idx < 64 { Some(self.dsp_regs[idx]) } else { None }
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) {
        if addr & 3 != 0 {
            // registros DSP são word-aligned
            return;
        }
        let idx = ((addr & 0xFF) >> 2) as usize;
        if idx < 64 {
            self.dsp_regs[idx] = (self.dsp_regs[idx] & !0xFF) | val as u32;
        }
    }

    pub fn write_half(&mut self, addr: u32, val: u16) {
        if addr & 1 != 0 { return; }
        let idx = ((addr & 0xFF) >> 2) as usize;
        if idx < 64 {
            self.dsp_regs[idx] = (self.dsp_regs[idx] & !0xFFFF) | val as u32;
        }
    }

    pub fn write_word(&mut self, addr: u32, val: u32) {
        if addr & 3 != 0 { return; }
        let idx = ((addr & 0xFF) >> 2) as usize;
        if idx < 64 {
            self.dsp_regs[idx] = val;
            // Reg 0 = control: bits 0-7 enable channels
            if idx == 0 {
                for ch in 0..NUM_CHANNELS.min(8) {
                    self.channels[ch].enabled = (val >> ch) & 1 != 0;
                }
            }
        }
    }

    pub fn output_stereo(&self) -> (i16, i16) {
        (self.output_l, self.output_r)
    }
}
