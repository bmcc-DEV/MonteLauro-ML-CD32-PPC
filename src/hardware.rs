//! Orquestração do hardware completo do CD³².
//!
//! Gerencia o ciclo de boot: ColdFire inicia, carrega microkernel PPC,
//! libera reset, e ambos executam concorrentemente.

use crate::bus::Bus;
use crate::cpu::ppc603e::Ppu;
use crate::cpu::coldfire::ColdFire;
use crate::dma::DmaChannelId;
use crate::interrupt::IrqSource;

// Timing: PPC 266MHz, ColdFire 140MHz
// A cada quantum de PPC, ColdFire roda quantum * 140/266
const QUANTUM: u64 = 16;

pub struct Cd32Hardware {
    pub ppc: Ppu,
    pub coldfire: ColdFire,
    pub bus: Bus,
    pub total_cycles: u64,
    pub ppc_cycles: u64,
    pub cf_cycles: u64,
    pub boot_complete: bool,
    pub serial_out: String,
    cycle_acc: i64,
    pub ppc_hold: bool,      // PPC held in reset during ColdFire bootstrap
}

impl Cd32Hardware {
    pub fn new(bios: Vec<u8>) -> Self {
        Self {
            ppc: Ppu::new(),
            coldfire: ColdFire::new(),
            bus: Bus::new(bios),
            total_cycles: 0,
            ppc_cycles: 0,
            cf_cycles: 0,
            boot_complete: false,
            serial_out: String::new(),
            cycle_acc: 0,
            ppc_hold: true,
        }
    }

    pub fn set_trace(&mut self, ppc_trace: bool, cf_trace: bool) {
        self.ppc.trace = ppc_trace;
        self.coldfire.trace = cf_trace;
    }

    pub fn reset(&mut self) {
        self.ppc.reset();
        self.coldfire.reset();
        // O ColdFire começa executando do vetor de reset (0xFF00_0000)
        self.boot_complete = false;
        self.serial_out.clear();
        log::info!("CD³²: system reset");
    }

    /// Executa N ciclos combinados com interleave PPC/CF.
    ///
    /// Usa um acumulador de debt para manter o ratio 266:140 entre PPC e ColdFire.
    /// Periféricos (GPU, DMA, CDROM) são atualizados a cada QUANTUM.
    pub fn run_cycles(&mut self, cycles: u64) {
        let target = self.total_cycles + cycles;
        let cf_per_ppc = 140; // CF cycles por 266 PPC cycles
        let ppc_per_cf = 266;

        while self.total_cycles < target {
            // Decide qual CPU roda baseado no debt acumulado
            if self.cycle_acc >= 0 {
                // CF está atrasado → roda ColdFire
                if self.coldfire.halt && self.bus.intc.cf_irq_pending().is_some() {
                    self.coldfire.halt = false;
                }
                if !self.coldfire.halt {
                    if let Err(e) = self.coldfire.step(&mut self.bus) {
                        log::error!("ColdFire error: {} — halting", e);
                        self.coldfire.halt = true;
                    }
                }
                self.cf_cycles += 1;
                self.cycle_acc -= ppc_per_cf;
                // Se ColdFire parou (STOP/halt), libera PPC
                if self.coldfire.halt && self.ppc_hold {
                    self.ppc_hold = false;
                    log::info!("CD³²: ColdFire handed off, PPC released");
                }
            } else if !self.ppc_hold {
                // PPC está atrasado e não está em hold → roda PPC
                if self.ppc.halt && self.bus.intc.ppc_irq_pending() {
                    self.ppc.halt = false;
                }
                if !self.ppc.halt {
                    if let Err(e) = self.ppc.step(&mut self.bus) {
                        log::error!("PPC error: {} — halting", e);
                        self.ppc.halt = true;
                    }
                }
                self.ppc_cycles += 1;
                self.cycle_acc += cf_per_ppc;
            } else {
                // PPC em hold, mas o acumulador quer rodar PPC
                // Em vez disso, debita mais e tenta de novo
                self.cycle_acc += cf_per_ppc;
            }

            self.total_cycles += 1;

            // Periféricos a cada QUANTUM ciclos
            if self.total_cycles % QUANTUM == 0 {
                let ppc_in_quantum = (QUANTUM as f64 * cf_per_ppc as f64 / (cf_per_ppc + ppc_per_cf) as f64) as u32;
                let cf_in_quantum = QUANTUM as u32 - ppc_in_quantum;
                self.bus.tick(ppc_in_quantum, cf_in_quantum);

                // DMA
                for xfer in self.bus.dma.tick() {
                    use crate::dma::DmaTransfer;
                    match xfer {
                        DmaTransfer::Copy { channel: _, src, dst, size: _ } => {
                            if let Some(val) = self.bus.mem.read_word(src) {
                                self.bus.mem.write_word(dst, val);
                            } else {
                                log::warn!("DMA: bus error at src=0x{:08X}", src);
                            }
                        }
                        DmaTransfer::Done(ch_id) => {
                            let src = match ch_id {
                                DmaChannelId::Cdrom => IrqSource::CdromData,
                                _ => IrqSource::DmaDone,
                            };
                            self.bus.intc.assert_irq(src);
                        }
                    }
                }

                if self.coldfire.halt && !self.boot_complete && !self.ppc_hold {
                    self.boot_complete = true;
                }
                self.poll_serial();
            }
        }
    }

    /// Simula um boot completo (~3.2 segundos reais).
    pub fn boot(&mut self) {
        log::info!("CD³²: power-on boot sequence starting...");

        // Fase 0: ColdFire bootstrap (~500μs = ~70k CF cycles)
        self.run_cycles(70_000);

        // Fase 1: PPC boot (~5ms = ~1.33M PPC cycles)
        self.run_cycles(1_330_000);

        // Fase 2: Kickstart Desktop (~3.2s total)
        self.run_cycles(850_000_000);

        log::info!("CD³²: boot complete ({} total cycles)", self.total_cycles);
    }

    fn poll_serial(&mut self) {
        // Stub: lê da UART do ColdFire (0x0220_0000)
        // Emuladores reais usariam um callback pro terminal
        let _uart_status = self.bus.mem.read_byte(0x0220_0000);
        let _uart_data = self.bus.mem.read_byte(0x0220_0004);
        // Se houver caractere disponível, anexa ao buffer serial_out
    }
}
