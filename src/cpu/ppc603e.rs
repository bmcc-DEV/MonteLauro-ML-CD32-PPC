//! Interpretador PPC603e — subset mínimo para bootstrap e execução de código nativo CD³².
//!
//! Implementa o suficiente do PowerPC ISA (MPC603e) para rodar o microkernel e
//! o kernel AmigaOS PPC. Instruções não implementadas geram `unimplemented!()`.

use crate::bus::BusInterface;
use crate::disasm;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PpcError {
    #[error("ilhóp (illegal instruction) at PC=0x{0:08X}")]
    IllegalInstruction(u32),
    #[error("page fault at address 0x{0:08X}")]
    PageFault(u32),
    #[error("alignment fault at address 0x{0:08X}")]
    AlignmentFault(u32),
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PpcRegs {
    pub gpr: [u32; 32],
    pub pc: u32,
    pub lr: u32,
    pub ctr: u32,
    pub cr: u32,
    pub xer: u32,
    pub msr: u32,
    pub srr0: u32,
    pub srr1: u32,
    // MMU
    pub sr: [u32; 16],       // 16 segment registers
    pub sdr1: u32,            // page table base & size
    pub ibat: [u32; 8],      // IBAT0U, IBAT0L, IBAT1U, IBAT1L, IBAT2U, IBAT2L, IBAT3U, IBAT3L
    pub dbat: [u32; 8],      // DBAT0U-DBAT3L
    pub tlb_miss: bool,       // set when last translation caused TLB miss
}

#[derive(Default)]
pub struct Ppu {
    pub regs: PpcRegs,
    pub halt: bool,
    pub trace: bool,
}

impl Ppu {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.reset();
        s
    }

    pub fn reset(&mut self) {
        self.regs = PpcRegs::default();
        self.regs.pc = 0x0000_0100;
        self.regs.msr = 0x0000_0040;
        self.halt = false;
        self.trace = false;
    }

    pub fn step(&mut self, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        if self.halt {
            return Ok(0);
        }

        // Check for external interrupt (MSR[EE] bit 15)
        if (self.regs.msr >> 15) & 1 != 0 && bus.ppc_irq_pending() {
            self.take_extint(bus)?;
            return Ok(1);
        }

        let pc = self.regs.pc;
        // Translate instruction fetch address
        let phys_pc = self.translate(false, pc, bus)?;
        let insn = bus.read_word(phys_pc).ok_or(PpcError::PageFault(pc))?;
        self.regs.pc = pc.wrapping_add(4);

        let result = self.dispatch(insn, bus);
        if self.trace {
            let status = match &result {
                Ok(_) => "OK",
                Err(_) => "ERR",
            };
            let mnem = disasm::disasm_ppc(insn);
            log::debug!(
                "PPC TRACE 0x{:08X}: 0x{:08X}  {:<26} R3={:08X} R4={:08X} R5={:08X} LR={:08X} CR={:08X} [{}]",
                pc, insn, mnem,
                self.regs.gpr[3], self.regs.gpr[4], self.regs.gpr[5],
                self.regs.lr, self.regs.cr, status,
            );
        }
        result
    }

    // ── MMU Translation ───────────────────────────────────────────────

    fn translate(&self, is_data: bool, addr: u32, bus: &dyn BusInterface) -> Result<u32, PpcError> {
        let enable = if is_data {
            (self.regs.msr >> 9) & 1  // MSR[DR]
        } else {
            (self.regs.msr >> 8) & 1  // MSR[IR]
        };
        if enable == 0 {
            return Ok(addr); // identity
        }

        // 1. BAT lookup
        let bats = if is_data { &self.regs.dbat } else { &self.regs.ibat };
        for i in 0..4 {
            let u = bats[i * 2];
            let l = bats[i * 2 + 1];
            let vs = (u >> 31) & 1;
            let vp = (u >> 30) & 1;
            let msr_pr = (self.regs.msr >> 19) & 1;
            let valid = if msr_pr == 0 { vs != 0 } else { vp != 0 };
            if !valid { continue; }

            let bepi = u & 0xFFFC_0000;       // upper 15 bits of effective page
            let bl = (u >> 21) & 0xF;          // block length (4 bits, 0-15)
            let block_bits = bl + 17;           // 128KB (2^17) to 256MB (2^28)
            let mask = if block_bits >= 32 { 0 } else { !((1u64 << block_bits) - 1) as u32 };

            if (addr ^ bepi) & mask == 0 {
                let brpn = l & 0xFFFC_0000;
                let phys = (addr & !mask) | (brpn & mask);
                return Ok(phys);
            }
        }

        // 2. Page table walk
        let sr_idx = (addr >> 28) & 0xF;
        let sr_val = self.regs.sr[sr_idx as usize];
        let vsid = sr_val & 0x00FF_FFFF;

        let page_index = (addr >> 12) & 0xFFFF;
        let offset = addr & 0xFFF;
        let api = (page_index >> 10) as u8;   // high 6 bits of page index = EA[10-15] via addr bits 10-15

        let sdr1 = self.regs.sdr1;
        if sdr1 == 0 {
            return Err(PpcError::PageFault(addr));
        }
        let htabs = sdr1 & 0xFFFF0000;
        let htabmask = (sdr1 << 16) >> 16;
        let hmask = (htabmask | 0xFFFF) as u16 as u32;

        let hash1 = vsid ^ (page_index as u32);
        let hash2 = !hash1;

        for (hash, hbit) in [(hash1, 0u32), (hash2, 1u32)] {
            let pteg_addr = htabs | ((hash & hmask as u32) << 6);
            for pte_idx in 0..8u32 {
                let pte_addr = pteg_addr + pte_idx * 8;
                let pte0 = match bus.read_word(pte_addr) {
                    Some(v) => v,
                    None => return Err(PpcError::PageFault(addr)),
                };
                if pte0 & 1 == 0 { continue; }
                let pte_vsid = (pte0 >> 7) & 0x00FF_FFFF;
                let pte_h = (pte0 >> 6) & 1;
                let pte_api = pte0 & 0x3F;
                if pte_vsid != vsid || pte_h != hbit || pte_api != api as u32 {
                    continue;
                }
                let pte1 = match bus.read_word(pte_addr + 4) {
                    Some(v) => v,
                    None => return Err(PpcError::PageFault(addr)),
                };
                let rpn = pte1 & 0xFFFFF000;
                return Ok(rpn | offset);
            }
        }

        Err(PpcError::PageFault(addr))
    }

    fn data_translate(&self, addr: u32, bus: &dyn BusInterface) -> Result<u32, PpcError> {
        self.translate(true, addr, bus)
    }

    fn take_extint(&mut self, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        self.regs.srr0 = self.regs.pc;
        self.regs.srr1 = self.regs.msr;
        self.regs.msr &= !(1 << 15); // MSR[EE] = 0
        self.regs.pc = 0x0000_0500;  // PPC external interrupt vector
        if self.trace {
            log::debug!("PPC IRQ vector=0x0500 SRR0=0x{:08X} SRR1=0x{:08X}", self.regs.srr0, self.regs.srr1);
        }
        Ok(1)
    }

    fn dispatch(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let opcd = (insn >> 26) & 0x3F;

        match opcd {
            0b000011 => self.op_addi(insn, bus),   // ADDI
            0b000100 => self.op_addic(insn, bus),  // ADDIC
            0b001111 => self.op_addis(insn, bus),  // ADDIS
            0b011100 => self.op_pform(insn, bus),  // Instruções primárias (OPCD=0x1C)
            0b010011 => {
                // Opcd 19: branches (B-form) + CR ops + RFI (XL-form)
                let xop = (insn >> 1) & 0x3FF;
                if xop == 50 {
                    // RFI (Return From Interrupt)
                    self.regs.msr = self.regs.srr1;
                    self.regs.pc = self.regs.srr0;
                    Ok(1)
                } else {
                    self.op_bform(insn, bus)
                }
            }
            0b000111 => self.op_mulli(insn, bus),  // MULLI
            0b001011 => self.op_cmpi(insn, bus),   // CMPI (opcd 11)
            0b001010 => self.op_cmpli(insn, bus),  // CMPLI (opcd 10)
            0b011000 => self.op_ori(insn, bus),    // ORI
            0b011001 => self.op_oris(insn, bus),   // ORIS
            0b011100 => self.op_andi(insn, bus),   // ANDI.
            0b010101 => self.op_rlwinm(insn, bus), // RLWINM (opcd 21)
            0b010100 => self.op_rlwimi(insn, bus), // RLWIMI (opcd 20)
            0b100000 => self.op_lwz(insn, bus),    // LWZ
            0b100001 => self.op_lwzu(insn, bus),   // LWZU
            0b011111 => self.op_xform(insn, bus),  // X-form (OPCD=0x1F)
            0b100100 => self.op_stw(insn, bus),    // STW
            0b100101 => self.op_stwu(insn, bus),   // STWU
            0b010000 => {
                // BC (B-form: BO, BI, BD, AA, LK)
                let bo = ((insn >> 21) & 0x1F) as u8;
                let bi = ((insn >> 16) & 0x1F) as u8;
                 let bd_sext = ((((insn >> 2) & 0x3FFF) as i16) << 2) >> 2; // sign-extend 14-bit BD
                // PPC: IBM bit 30 = Rust bit 1 (AA), IBM bit 31 = Rust bit 0 (LK)
                let aa = (insn >> 1) & 1;
                let lk = insn & 1;
                if lk == 1 { self.regs.lr = self.regs.pc; }
                let target = if aa == 1 {
                    (bd_sext << 2) as u32
                } else {
                    (self.regs.pc as i32).wrapping_add((bd_sext << 2) as i32) as u32
                };
                // Branch logic
                if (bo & 0x14) == 0x14 {
                    self.regs.pc = target;
                    return Ok(1);
                }
                if bo & 0x04 == 0 {
                    self.regs.ctr = self.regs.ctr.wrapping_sub(1);
                    let ctr_zero = self.regs.ctr == 0;
                    if ctr_zero && (bo & 0x02) != 0 { return Ok(1); }
                    if !ctr_zero && (bo & 0x02) == 0 { return Ok(1); }
                }
                let cr_bit = (self.regs.cr >> (31 - bi)) & 1;
                let cond_true = (bo >> 3) & 1;
                if (cr_bit as u8) == cond_true {
                    self.regs.pc = target;
                }
                Ok(1)
            }
            0b010010 => {
                // opcd 18 = B (branch). B-form: AA bit (bit 30) determines absolute/relative
                // For our i_b function, we use opcd 18 with AA=1 (absolute)
                // Reuse op_bform but need to handle differently
                let lk = insn & 1;
                let aa = (insn >> 1) & 1;
                let li = ((insn >> 2) & 0x3FF_FFFF) as u32;
                if lk == 1 { self.regs.lr = self.regs.pc; }
                let target = if aa == 1 {
                    li << 2
                } else {
                    self.regs.pc.wrapping_add(((li as i32) << 2) as u32).wrapping_sub(4)
                };
                self.regs.pc = target;
                Ok(1)
            }
            _ => {
                log::warn!("PPC: instruction 0x{:08X} at PC=0x{:08X} not implemented (opcd={})", insn, self.regs.pc.wrapping_sub(4), opcd);
                Err(PpcError::IllegalInstruction(self.regs.pc.wrapping_sub(4)))
            }
        }
    }

    // ── D-form instructions ────────────────────────────────────────────

    fn dform_op(&self, insn: u32) -> (u32, u32, i16) {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let si = insn as i16;
        (rd, ra, si)
    }

    fn op_addis(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rd, ra, si) = self.dform_op(insn);
        let a = if ra == 0 { 0 } else { self.regs.gpr[ra as usize] };
        self.regs.gpr[rd as usize] = a.wrapping_add((((si as i32) as i64) << 16) as u32);
        Ok(1)
    }

    fn op_addi(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rd, ra, si) = self.dform_op(insn);
        let a = if ra == 0 { 0 } else { self.regs.gpr[ra as usize] };
        self.regs.gpr[rd as usize] = a.wrapping_add(si as u32);
        Ok(1)
    }

    fn op_addic(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rd, ra, si) = self.dform_op(insn);
        let a = if ra == 0 { 0 } else { self.regs.gpr[ra as usize] };
        let (res, carry) = a.overflowing_add(si as u32);
        self.regs.gpr[rd as usize] = res;
        if carry { self.regs.xer |= 0x20000000; } else { self.regs.xer &= !0x20000000; }
        Ok(1)
    }

    fn op_cmpi(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let si = insn as i16;
        let a = self.regs.gpr[ra as usize] as i32;
        let b = si as i32;
        let crf = (rd >> 2) & 0x07;
        let cr_shift = 28 - crf * 4;
        let cr_bits = if a < b {
            0b1000
        } else if a > b {
            0b0100
        } else {
            0b0010
        } | 0;
        self.regs.cr = (self.regs.cr & !(0xF << cr_shift)) | (cr_bits << cr_shift);
        Ok(1)
    }

    fn op_cmpli(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let ui = insn as u16;
        let a = self.regs.gpr[ra as usize];
        let b = ui as u32;
        let crf = (rd >> 2) & 0x07;
        let cr_shift = 28 - crf * 4;
        let cr_bits = if a < b {
            0b1000
        } else if a > b {
            0b0100
        } else {
            0b0010
        };
        self.regs.cr = (self.regs.cr & !(0xF << cr_shift)) | (cr_bits << cr_shift);
        Ok(1)
    }

    fn op_mulli(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rd, ra, si) = self.dform_op(insn);
        let a = if ra == 0 { 0 } else { self.regs.gpr[ra as usize] };
        self.regs.gpr[rd as usize] = (a as i32).wrapping_mul(si as i32) as u32;
        Ok(1)
    }

    fn op_ori(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let ui = insn as u16;
        self.regs.gpr[ra as usize] = self.regs.gpr[rd as usize] | ui as u32;
        Ok(1)
    }

    fn op_oris(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let ui = insn as u16;
        self.regs.gpr[ra as usize] = self.regs.gpr[rd as usize] | ((ui as u32) << 16);
        Ok(1)
    }

    fn op_andi(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let ui = insn as u16;
        let val = self.regs.gpr[rd as usize] & (ui as u32);
        self.regs.gpr[ra as usize] = val;
        // ANDI. atualiza CR0
        let cr_bits = if val == 0 { 0b0010 } else if (val as i32) < 0 { 0b1000 } else { 0b0100 };
        self.regs.cr = (self.regs.cr & 0x0FFF_FFFF) | (cr_bits << 28);
        Ok(1)
    }

    fn op_rlwinm(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let sh = (insn >> 11) & 0x1F;
        let mb = (insn >> 6) & 0x1F;
        let me = (insn >> 1) & 0x1F;
        let r = self.regs.gpr[rs as usize].rotate_left(sh);
        let mask = make_mask(mb, me);
        self.regs.gpr[ra as usize] = r & mask;
        if insn & 1 != 0 {
            let val = self.regs.gpr[ra as usize];
            let cr_bits = if val == 0 { 0b0010 } else if (val as i32) < 0 { 0b1000 } else { 0b0100 };
            self.regs.cr = (self.regs.cr & 0x0FFF_FFFF) | (cr_bits << 28);
        }
        Ok(1)
    }

    fn op_rlwimi(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let sh = (insn >> 11) & 0x1F;
        let mb = (insn >> 6) & 0x1F;
        let me = (insn >> 1) & 0x1F;
        let r = self.regs.gpr[rs as usize].rotate_left(sh);
        let mask = make_mask(mb, me);
        self.regs.gpr[ra as usize] = (self.regs.gpr[ra as usize] & !mask) | (r & mask);
        Ok(1)
    }

    // ── Load / Store ───────────────────────────────────────────────────

    fn op_lwz(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rd, ra, si) = self.dform_op(insn);
        let ea = (if ra == 0 { 0 } else { self.regs.gpr[ra as usize] }).wrapping_add(si as u32);
        let addr = self.data_translate(ea, bus)?;
        let val = bus.read_word(addr).ok_or(PpcError::PageFault(ea))?;
        self.regs.gpr[rd as usize] = val;
        Ok(1)
    }

    fn op_lwzu(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rd, ra, si) = self.dform_op(insn);
        let ea = self.regs.gpr[ra as usize].wrapping_add(si as u32);
        let addr = self.data_translate(ea, bus)?;
        let val = bus.read_word(addr).ok_or(PpcError::PageFault(ea))?;
        self.regs.gpr[rd as usize] = val;
        self.regs.gpr[ra as usize] = ea;
        Ok(1)
    }

    fn op_stw(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rs, ra, si) = self.dform_op(insn);
        let ea = (if ra == 0 { 0 } else { self.regs.gpr[ra as usize] }).wrapping_add(si as u32);
        let addr = self.data_translate(ea, bus)?;
        let val = self.regs.gpr[rs as usize];
        bus.write_word(addr, val).ok_or(PpcError::PageFault(ea))?;
        Ok(1)
    }

    fn op_stwu(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let (rs, ra, si) = self.dform_op(insn);
        let ea = self.regs.gpr[ra as usize].wrapping_add(si as u32);
        let addr = self.data_translate(ea, bus)?;
        let val = self.regs.gpr[rs as usize];
        bus.write_word(addr, val).ok_or(PpcError::PageFault(ea))?;
        self.regs.gpr[ra as usize] = ea;
        Ok(1)
    }

    fn op_lwzx(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let ea = (if ra == 0 { 0 } else { self.regs.gpr[ra as usize] }).wrapping_add(self.regs.gpr[rb as usize]);
        let addr = self.data_translate(ea, bus)?;
        let val = bus.read_word(addr).ok_or(PpcError::PageFault(ea))?;
        self.regs.gpr[rd as usize] = val;
        Ok(1)
    }

    // ── Branch ─────────────────────────────────────────────────────────

    fn op_bform(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        // PPC: IBM bit 31 = Rust bit 0 (LK), IBM bit 30 = Rust bit 1 (AA)
        let lk = insn & 1;
        let aa = (insn >> 1) & 1;
        let bo = ((insn >> 21) & 0x1F) as u8;
        let bi = ((insn >> 16) & 0x1F) as u8;
        let bd = ((insn as i16) & !3) as u32;

        if lk == 1 {
            self.regs.lr = self.regs.pc; // set link (já avançou pc)
        }

        let target = if aa == 1 {
            bd // absolute
        } else {
            self.regs.pc.wrapping_add(bd.wrapping_sub(4)) // relative (pc já avançou)
        };

        // BO bits: branch always?
        if (bo & 0x14) == 0x14 {
            // branch always
            self.regs.pc = target;
            return Ok(1);
        }

        // CTR-based
        if bo & 0x04 == 0 {
            self.regs.ctr = self.regs.ctr.wrapping_sub(1);
            if self.regs.ctr == 0 && (bo & 0x02) != 0 { return Ok(1); }
            if self.regs.ctr != 0 && (bo & 0x02) == 0 { return Ok(1); }
        }

        // CR bit condition
        let cr_bit = (self.regs.cr >> (31 - bi)) & 1;
        let cond_true = (bo >> 3) & 1;
        if (cr_bit as u32) == cond_true as u32 {
            self.regs.pc = target;
        }

        Ok(1)
    }

    // ── P-form (OPCD=0x1C) ────────────────────────────────────────────

    fn op_pform(&mut self, insn: u32, _bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let subop = (insn >> 1) & 0x1F;
        match subop {
            0b00000 => self.op_mcrf(insn),
            _ => Err(PpcError::IllegalInstruction(self.regs.pc.wrapping_sub(4)))
        }
    }

    fn op_mcrf(&mut self, insn: u32) -> Result<u32, PpcError> {
        let crf_s = (insn >> 18) & 0x07;
        let crf_d = (insn >> 23) & 0x07;
        let shift_s = 28 - crf_s * 4;
        let shift_d = 28 - crf_d * 4;
        let field = (self.regs.cr >> shift_s) & 0xF;
        self.regs.cr = (self.regs.cr & !(0xF << shift_d)) | (field << shift_d);
        Ok(1)
    }

    // ── X-form (OPCD=0x1F) ────────────────────────────────────────────

    fn op_xform(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let xop = (insn >> 1) & 0x3FF;
        match xop {
            0b0000001000 => self.op_subfc(insn),  // SUBFC
            0b0000100011 => self.op_or(insn),     // OR
            0b0000100111 => self.op_nor(insn),    // NOR
            0b0001000100 => self.op_and(insn),    // AND
            0b0001000111 => self.op_andc(insn),   // ANDC
            0b0001011000 => self.op_neg(insn),    // NEG
            0b0001100000 => self.op_cntlzw(insn), // CNTLZW
            0b0001110100 => self.op_add(insn),    // ADD
            0b0010000000 => self.op_subf(insn),   // SUBF
            0b0010011011 => self.op_subfe(insn),  // SUBFE
            0b0010101010 => self.op_addme(insn),  // ADDME
            0b0010101100 => self.op_mulhw(insn),  // MULHW
            0b0010101110 => self.op_mulhwu(insn), // MULHWU
            0b0011111011 => self.op_mullw(insn),  // MULLW
            0b0111011011 => self.op_divw(insn),   // DIVW
            0b0111110111 => self.op_divwu(insn),  // DIVWU
            0b0000010010 => self.op_mfsr(insn),   // MFSR
            0b0000110110 => self.op_mtsr(insn),   // MTSR
            0b0010000010 => self.op_cmp(insn),    // CMP
            0b0010000011 => self.op_cmpl(insn),   // CMPL
            0b0010010111 => self.op_mtmsr(insn),  // MTMSR
            0b0100000110 => self.op_mfmsr(insn),  // MFMSR
            0b0010010000 => self.op_mtcrf(insn),  // MTCRF
            0b0000010011 => self.op_mfcr(insn),   // MFCR
            0b0010011001 => self.op_lwzx(insn, bus), // LWZX indexed
            0b0010001001 => self.op_stwx(insn, bus), // STWX
            0b0010010010 => self.op_mtspr(insn),    // MTSPR
            0b0101010011 => self.op_mfspr(insn),    // MFSPR
            _ => {
                log::warn!("PPC: X-form instruction xop={} not implemented", xop);
                Err(PpcError::IllegalInstruction(self.regs.pc.wrapping_sub(4)))
            }
        }
    }

    fn op_subfc(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let a = self.regs.gpr[ra as usize];
        let b = self.regs.gpr[rb as usize];
        let (res, carry) = b.overflowing_add(!a.wrapping_add(1));
        self.regs.gpr[rd as usize] = res;
        if carry { self.regs.xer |= 0x20000000; } else { self.regs.xer &= !0x20000000; }
        Ok(1)
    }

    fn op_or(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[ra as usize] = self.regs.gpr[rs as usize] | self.regs.gpr[rb as usize];
        if insn & 1 != 0 {
            let val = self.regs.gpr[ra as usize];
            let cr_bits = if val == 0 { 0b0010 } else if (val as i32) < 0 { 0b1000 } else { 0b0100 };
            self.regs.cr = (self.regs.cr & 0x0FFF_FFFF) | (cr_bits << 28);
        }
        Ok(1)
    }

    fn op_nor(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[ra as usize] = !(self.regs.gpr[rs as usize] | self.regs.gpr[rb as usize]);
        if insn & 1 != 0 {
            let val = self.regs.gpr[ra as usize];
            let cr_bits = if val == 0 { 0b0010 } else if (val as i32) < 0 { 0b1000 } else { 0b0100 };
            self.regs.cr = (self.regs.cr & 0x0FFF_FFFF) | (cr_bits << 28);
        }
        Ok(1)
    }

    fn op_and(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[ra as usize] = self.regs.gpr[rs as usize] & self.regs.gpr[rb as usize];
        if insn & 1 != 0 {
            let val = self.regs.gpr[ra as usize];
            let cr_bits = if val == 0 { 0b0010 } else if (val as i32) < 0 { 0b1000 } else { 0b0100 };
            self.regs.cr = (self.regs.cr & 0x0FFF_FFFF) | (cr_bits << 28);
        }
        Ok(1)
    }

    fn op_andc(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[ra as usize] = self.regs.gpr[rs as usize] & !self.regs.gpr[rb as usize];
        Ok(1)
    }

    fn op_neg(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        self.regs.gpr[rd as usize] = (!self.regs.gpr[ra as usize]).wrapping_add(1);
        Ok(1)
    }

    fn op_cntlzw(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        self.regs.gpr[ra as usize] = self.regs.gpr[rs as usize].leading_zeros();
        Ok(1)
    }

    fn op_add(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[rd as usize] = self.regs.gpr[ra as usize].wrapping_add(self.regs.gpr[rb as usize]);
        Ok(1)
    }

    fn op_subf(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[rd as usize] = (!self.regs.gpr[ra as usize]).wrapping_add(self.regs.gpr[rb as usize]).wrapping_add(1);
        Ok(1)
    }

    fn op_subfe(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let carry = (self.regs.xer >> 29) & 1;
        let a = self.regs.gpr[ra as usize];
        let b = self.regs.gpr[rb as usize];
        let (res, c1) = (!a).overflowing_add(carry);
        let (res, c2) = b.overflowing_add(res);
        let (res, _) = res.overflowing_add(1);
        self.regs.gpr[rd as usize] = res;
        if c1 || c2 { self.regs.xer |= 0x20000000; } else { self.regs.xer &= !0x20000000; }
        Ok(1)
    }

    fn op_addme(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let carry = (self.regs.xer >> 29) & 1;
        let a = self.regs.gpr[ra as usize] as i32;
        let res = a.wrapping_add(a >> 31).wrapping_add(carry as i32);
        self.regs.gpr[rd as usize] = res as u32;
        Ok(1)
    }

    fn op_mulhw(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let a = self.regs.gpr[ra as usize] as i32 as i64;
        let b = self.regs.gpr[rb as usize] as i32 as i64;
        self.regs.gpr[rd as usize] = ((a * b) >> 32) as u32;
        Ok(1)
    }

    fn op_mulhwu(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let a = self.regs.gpr[ra as usize] as u64;
        let b = self.regs.gpr[rb as usize] as u64;
        self.regs.gpr[rd as usize] = ((a * b) >> 32) as u32;
        Ok(1)
    }

    fn op_mullw(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        self.regs.gpr[rd as usize] = self.regs.gpr[ra as usize].wrapping_mul(self.regs.gpr[rb as usize]);
        Ok(1)
    }

    fn op_divw(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let a = self.regs.gpr[ra as usize] as i32;
        let b = self.regs.gpr[rb as usize] as i32;
        self.regs.gpr[rd as usize] = a.wrapping_div(b) as u32;
        Ok(1)
    }

    fn op_divwu(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let a = self.regs.gpr[ra as usize];
        let b = self.regs.gpr[rb as usize];
        self.regs.gpr[rd as usize] = a.wrapping_div(b);
        Ok(1)
    }

    fn op_stwx(&mut self, insn: u32, bus: &mut dyn BusInterface) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let ea = (if ra == 0 { 0 } else { self.regs.gpr[ra as usize] }).wrapping_add(self.regs.gpr[rb as usize]);
        let addr = self.data_translate(ea, bus)?;
        bus.write_word(addr, self.regs.gpr[rs as usize]).ok_or(PpcError::PageFault(ea))?;
        Ok(1)
    }

    fn op_cmp(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let crf = (rd >> 2) & 0x07;
        let cr_shift = 28 - crf * 4;
        let a = self.regs.gpr[ra as usize] as i32;
        let b = self.regs.gpr[rb as usize] as i32;
        let cr_bits = if a < b { 0b1000 } else if a > b { 0b0100 } else { 0b0010 };
        self.regs.cr = (self.regs.cr & !(0xF << cr_shift)) | (cr_bits << cr_shift);
        Ok(1)
    }

    fn op_cmpl(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let ra = (insn >> 16) & 0x1F;
        let rb = (insn >> 11) & 0x1F;
        let crf = (rd >> 2) & 0x07;
        let cr_shift = 28 - crf * 4;
        let a = self.regs.gpr[ra as usize];
        let b = self.regs.gpr[rb as usize];
        let cr_bits = if a < b { 0b1000 } else if a > b { 0b0100 } else { 0b0010 };
        self.regs.cr = (self.regs.cr & !(0xF << cr_shift)) | (cr_bits << cr_shift);
        Ok(1)
    }

    fn op_mtmsr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        self.regs.msr = self.regs.gpr[rs as usize];
        Ok(1)
    }

    fn op_mfmsr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        self.regs.gpr[rd as usize] = self.regs.msr;
        Ok(1)
    }

    fn op_mtcrf(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let mask = ((insn >> 12) & 0xFF) as u8;
        let val = self.regs.gpr[rs as usize];
        if mask == 0xFF {
            self.regs.cr = val;
        } else {
            for i in 0..8 {
                if (mask >> (7 - i)) & 1 != 0 {
                    let shift = 28 - i * 4;
                    self.regs.cr = (self.regs.cr & !(0xF << shift)) | ((val >> shift) & 0xF) << shift;
                }
            }
        }
        Ok(1)
    }

    fn op_mfcr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        self.regs.gpr[rd as usize] = self.regs.cr;
        Ok(1)
    }

    // ── SPR ────────────────────────────────────────────────────────────

    fn op_mtspr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let sprn = ((insn >> 11) & 0x1F) << 5 | ((insn >> 16) & 0x1F);
        let val = self.regs.gpr[rd as usize];
        match sprn {
            8 => self.regs.lr = val,
            9 => self.regs.ctr = val,
            25 => self.regs.sdr1 = val,
            26 => self.regs.srr0 = val,
            27 => self.regs.srr1 = val,
            // IBAT upper/lower: SPR 528-535
            528 => self.regs.ibat[0] = val,
            529 => self.regs.ibat[1] = val,
            530 => self.regs.ibat[2] = val,
            531 => self.regs.ibat[3] = val,
            532 => self.regs.ibat[4] = val,
            533 => self.regs.ibat[5] = val,
            534 => self.regs.ibat[6] = val,
            535 => self.regs.ibat[7] = val,
            // DBAT upper/lower: SPR 536-543
            536 => self.regs.dbat[0] = val,
            537 => self.regs.dbat[1] = val,
            538 => self.regs.dbat[2] = val,
            539 => self.regs.dbat[3] = val,
            540 => self.regs.dbat[4] = val,
            541 => self.regs.dbat[5] = val,
            542 => self.regs.dbat[6] = val,
            543 => self.regs.dbat[7] = val,
            _ => log::warn!("PPC: write to unknown SPR {}", sprn),
        }
        Ok(1)
    }

    fn op_mfspr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let sprn = ((insn >> 11) & 0x1F) << 5 | ((insn >> 16) & 0x1F);
        self.regs.gpr[rd as usize] = match sprn {
            8 => self.regs.lr,
            9 => self.regs.ctr,
            25 => self.regs.sdr1,
            26 => self.regs.srr0,
            27 => self.regs.srr1,
            528 => self.regs.ibat[0],
            529 => self.regs.ibat[1],
            530 => self.regs.ibat[2],
            531 => self.regs.ibat[3],
            532 => self.regs.ibat[4],
            533 => self.regs.ibat[5],
            534 => self.regs.ibat[6],
            535 => self.regs.ibat[7],
            536 => self.regs.dbat[0],
            537 => self.regs.dbat[1],
            538 => self.regs.dbat[2],
            539 => self.regs.dbat[3],
            540 => self.regs.dbat[4],
            541 => self.regs.dbat[5],
            542 => self.regs.dbat[6],
            543 => self.regs.dbat[7],
            _ => { log::warn!("PPC: read from unknown SPR {}", sprn); 0 }
        };
        Ok(1)
    }

    fn op_mfsr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rd = (insn >> 21) & 0x1F;
        let sr = (insn >> 16) & 0xF;
        self.regs.gpr[rd as usize] = self.regs.sr[sr as usize];
        Ok(1)
    }

    fn op_mtsr(&mut self, insn: u32) -> Result<u32, PpcError> {
        let rs = (insn >> 21) & 0x1F;
        let sr = (insn >> 16) & 0xF;
        self.regs.sr[sr as usize] = self.regs.gpr[rs as usize];
        if self.trace {
            log::debug!("PPC MMU: SR[{}] = 0x{:08X}", sr, self.regs.sr[sr as usize]);
        }
        Ok(1)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn make_mask(mb: u32, me: u32) -> u32 {
    if mb <= me {
        let m = (1u64 << (me - mb + 1)) - 1;
        (m << (31 - me)) as u32
    } else {
        let m = (1u64 << (32 - mb + me + 1)) - 1;
        !((m << (31 - me)) as u32)
    }
}
