//! Controlador de Interrupções do CD³².
//!
//! Gerencia 8 níveis de IRQ (0=nenhum, 1-7=prioridade) e roteia
//! para o PPC (externas) ou ColdFire (priorizadas com máscara SR).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqSource {
    GpuVBlank,
    CdromData,
    CdromDone,
    Timer0,
    Timer1,
    DmaDone,
    UartRx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuTarget {
    Ppc,
    ColdFire,
}

#[derive(Debug, Clone, Copy)]
struct IrqLine {
    asserted: bool,
}

pub struct InterruptController {
    lines: [IrqLine; 8],
    routing: [(IrqSource, CpuTarget, u8, u8); 7], // (source, target, level, vector)
    pending_ppc: bool,
    pending_cf_level: u8,
    pending_cf_vector: u8,
}

impl InterruptController {
    pub fn new() -> Self {
        Self {
            lines: [IrqLine { asserted: false }; 8],
            routing: [
                (IrqSource::GpuVBlank,   CpuTarget::Ppc,      1, 0x64),
                (IrqSource::CdromData,   CpuTarget::Ppc,      2, 0x68),
                (IrqSource::CdromDone,   CpuTarget::Ppc,      2, 0x68),
                (IrqSource::Timer0,      CpuTarget::ColdFire, 3, 0x6C),
                (IrqSource::Timer1,      CpuTarget::Ppc,      3, 0x6C),
                (IrqSource::DmaDone,     CpuTarget::Ppc,      4, 0x70),
                (IrqSource::UartRx,      CpuTarget::ColdFire, 4, 0x70),
            ],
            pending_ppc: false,
            pending_cf_level: 0,
            pending_cf_vector: 0,
        }
    }

    pub fn assert_irq(&mut self, source: IrqSource) {
        for (src, _, level, _vector) in &self.routing {
            if *src == source {
                let idx = *level as usize;
                self.lines[idx] = IrqLine { asserted: true };
            }
        }
        self.update_pending();
    }

    pub fn deassert_irq(&mut self, source: IrqSource) {
        for (src, _, level, _) in &self.routing {
            if *src == source {
                let idx = *level as usize;
                self.lines[idx].asserted = false;
            }
        }
        self.update_pending();
    }

    fn update_pending(&mut self) {
        self.pending_ppc = false;
        self.pending_cf_level = 0;
        self.pending_cf_vector = 0;

        for level in (1..8).rev() {
            if !self.lines[level].asserted {
                continue;
            }
            for (_, target, lvl, vec) in &self.routing {
                if *lvl as usize != level { continue; }
                match target {
                    CpuTarget::Ppc => self.pending_ppc = true,
                    CpuTarget::ColdFire => {
                        if *lvl > self.pending_cf_level {
                            self.pending_cf_level = *lvl;
                            self.pending_cf_vector = *vec;
                        }
                    }
                }
            }
        }
    }

    pub fn ppc_irq_pending(&self) -> bool {
        self.pending_ppc
    }

    pub fn cf_irq_pending(&self) -> Option<(u8, u8)> {
        if self.pending_cf_level > 0 {
            Some((self.pending_cf_level, self.pending_cf_vector))
        } else {
            None
        }
    }
}
