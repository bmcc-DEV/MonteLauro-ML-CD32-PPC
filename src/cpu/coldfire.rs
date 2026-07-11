//! ColdFire EMAC (V4e) — core mínimo com subset 68k-ColdFire compatível.
//!
//! O ColdFire é essencialmente um 68k sem instruções bit-field, BCD, ou
//! endereçamento memory-indirect. Implementamos o suficiente pro boot ROM
//! e pro kickstart compatibility layer rodarem.

use crate::bus::BusInterface;
use crate::disasm;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CfError {
    #[error("ColdFire: illegal instruction 0x{0:04X} at PC=0x{1:08X}")]
    IllegalInstruction(u16, u32),
    #[error("ColdFire: access fault at 0x{0:08X}")]
    AccessFault(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum CfAddressing {
    DataRegisterDirect { val: u32, idx: usize },
    AddressRegisterDirect { val: u32, idx: usize },
    AddressRegisterIndirect(u32),
    AddressRegisterPostinc(u32),
    AddressRegisterPredec(u32),
    AddressRegisterDisplacement(u32, i16),
    AbsShort(u32),
    AbsLong(u32),
    PCDisplacement(u32, i16),
    Immediate(u32),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CfRegs {
    pub d: [u32; 8],
    pub a: [u32; 8],
    pub pc: u32,
    pub sr: u16,
}

impl CfRegs {
    pub fn usp(&self) -> u32 { self.a[7] }
    pub fn set_usp(&mut self, val: u32) { self.a[7] = val; }
    pub fn ssp(&self) -> u32 { self.a[6] }
    pub fn set_ssp(&mut self, val: u32) { self.a[6] = val; }
}

#[derive(Default)]
pub struct ColdFire {
    pub regs: CfRegs,
    pub halt: bool,
    pub trace: bool,
}

impl ColdFire {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.reset();
        s
    }

    pub fn reset(&mut self) {
        self.regs = CfRegs::default();
        self.regs.pc = 0xFF00_0000;
        self.regs.sr = 0x2700;
        self.halt = false;
        self.trace = false;
    }

    pub fn step(&mut self, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        if self.halt {
            return Ok(0);
        }
        // Check interrupts
        if let Some((level, vector)) = bus.cf_irq_pending() {
            let mask = (self.regs.sr >> 8) & 0x07;
            if level > mask as u8 {
                self.take_interrupt(bus, level, vector)?;
                return Ok(1);
            }
        }

        let pc = self.regs.pc;
        let op = bus.read_half(pc).ok_or(CfError::AccessFault(pc))?;
        self.regs.pc = pc.wrapping_add(2);
        let result = self.dispatch(op, bus);
        if self.trace {
            let status = match &result {
                Ok(_) => "OK",
                Err(_) => "ERR",
            };
            let mnem = disasm::disasm_cf(op);
            log::debug!(
                "CF TRACE  0x{:08X}: 0x{:04X}  {:<18} D0={:08X} D1={:08X} A0={:08X} A7={:08X} SR={:04X} [{}]",
                pc, op, mnem,
                self.regs.d[0], self.regs.d[1],
                self.regs.a[0], self.regs.a[7],
                self.regs.sr, status,
            );
        }
        result
    }

    fn dispatch(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        match op >> 12 {
            0x0 => {
                // 0x0xxx: Bit manipulation, immediate, OR
                match op >> 8 {
                    0x00 => self.op_ill(op),
                    0x01 => self.op_btst_static(op, bus),
                    0x02 => self.op_bchg_static(op, bus),
                    0x03 => self.op_bclr_static(op, bus),
                    0x04 => self.op_bset_static(op, bus),
                    0x06 => self.op_bclr_dynamic(op, bus),
                    _ => self.op_ill(op),
                }
            }
            0x1 => self.op_move_byte(op, bus),   // MOVE.b
            0x2 => self.op_move_long(op, bus),   // MOVE.l
            0x3 => self.op_move_word(op, bus),   // MOVE.w
             0x4 => {
                // LEA: bits 8-6 = 111 (0x01C0 mask)
                if (op & 0x01C0) == 0x01C0 {
                    self.op_lea(op, bus)
                } else {
                match (op >> 8) & 0xFF {
                    0x40 => self.op_lea(op, bus),
                    0x41 => self.op_chk(op, bus),
                    0x42 => {
                        // CLR: bits 8-7 = size (00=byte, 01=word, 10=long)
                        // bit 6 = EA mode bit (0=Dn, 1=An/memory)
                        let sz = match (op >> 7) & 0x03 { 0 => 1, 1 => 2, _ => 4 };
                        self.clr_instr(op, sz, bus)
                    }
                    0x43 => self.op_move_from_sr(op, bus),
                    0x44 => self.op_move_to_ccr(op),
                    0x46 => self.op_move_to_sr(op, bus),
                    0x48 => self.op_swap(op),
                    0x49 => self.op_extw(op),
                    0x4A => self.op_tst_word(op),
                    0x4B => self.op_tst_long(op),
                    0x4C => self.op_div(op, bus), // DIVU / DIVS
                    0x4E => {
                match op & 0xFF {
                    0x71 => Ok(1),  // NOP
                    0x72 => self.op_stop(op),
                    0x73 => self.op_rte(bus),
                    0x75 => self.op_rts(bus),
                    0x80..=0xBF => self.op_jsr(bus),
                    _ => self.op_jump(op, bus),
                }
            }
                    0x4F => self.op_link_unlk(op, bus),
                     0x50 => self.op_stop(op),
                    0x54 => self.op_addq_word(op, bus),
                    0x55 => self.op_subq_word(op, bus),
                    0x56 => self.op_addq_long(op, bus),
                    0x57 => self.op_subq_long(op, bus),
                    0x58 => self.op_addq_byte(op, bus),
                    0x59 => self.op_subq_byte(op, bus),
                    0x5A => self.op_addq_byte(op, bus),
                    0x5C => self.op_subq_byte(op, bus),
                    0x60 => self.op_negx(op),
                     0x62 => self.op_clr_byte(op, bus),
                    0x64 => self.op_clr_word(op, bus),
                    0x66 => self.op_clr_long(op, bus),
                    0x68 => self.op_neg_word(op, bus),
                    0x6A => self.op_neg_long(op, bus),
                    0x6C => self.op_not_byte(op, bus),
                    0x6E => self.op_not_word(op, bus),
                    0x70 => self.op_not_long(op, bus),
                    0x72 => self.op_ill(op),
                    0x74 => self.op_nbcd(op),
                    0x76 => self.op_ill(op),
                    0x78 => self.op_ill(op),
                    0x7A => self.op_tas(op),
                    0x7C => self.op_ill(op),
                    0x7E => self.op_ill(op),
                    0x80 => self.op_move_usp(op),
                    0x82 => self.op_ill(op),
                    0x84 => self.op_ill(op),
                    0x86 => self.op_ill(op),
                    0x88 => self.op_movem(op, bus),
                    0x8A => self.op_ill(op),
                    0x8C => self.op_ill(op),
                    0x8E => self.op_ill(op),
                    0x90 => self.op_ill(op),
                    0x92 => self.op_ill(op),
                    0x94 => self.op_ill(op),
                    0x96 => self.op_ill(op),
                    0x98 => self.op_ill(op),
                    0x9A => self.op_ill(op),
                    0x9C => self.op_ill(op),
                    0x9E => self.op_ill(op),
                    0xA0..=0xBF => self.op_ill(op),
                     0xC0 => self.op_and_byte(op, bus),
                    0xC2 => self.op_and_long(op, bus),
                    0xC4 => self.op_and_word(op, bus),
                    0xC6 => self.op_mul_unsigned(op, bus),
                    0xC8 => self.op_eor_byte(op, bus),
                    0xCA => self.op_eor_long(op, bus),
                    0xCC => self.op_eor_word(op, bus),
                    0xCE => self.op_mul_signed(op, bus),
                    0xD0 => self.op_add_byte(op, bus),
                    0xD2 => self.op_add_long(op, bus),
                    0xD4 => self.op_add_word(op, bus),
                    0xD6 => self.op_addx_long(op),
                    0xD8 => self.op_sub_byte(op, bus),
                    0xDA => self.op_sub_long(op, bus),
                    0xDC => self.op_sub_word(op, bus),
                    0xDE => self.op_subx_long(op),
                    _ => self.op_ill(op),
                }
                } // end else (LEA detection)
            }
            0x5 => {
                // Scc (bits 8-6=111), ADDQ (bit 8=0), SUBQ (bit 8=1)
                if (op & 0x01C0) == 0x01C0 {
                    self.op_scc(op, bus)
                } else {
                    let sz = match (op >> 7) & 0x03 { 0 => 1, 1 => 2, _ => 4 };
                    if (op & 0x0100) == 0 {
                        self.addq_instr(op, sz, bus)
                    } else {
                        self.subq_instr(op, sz, bus)
                    }
                }
            }
             0x6 => {
                // Bcc (0x6xxx), BSR (0x61xx)
                if (op >> 8) & 0x0F == 0x01 {
                    self.op_bsr(bus)
                } else {
                    self.op_bcc(op, bus)
                }
            }
            0x7 => {
                match op >> 8 {
                    0x70 => self.op_moveq(op),
                    0x72 => self.op_moveq(op),
                    0x74 => self.op_moveq(op),
                    0x76 => self.op_moveq(op),
                    0x78 => self.op_moveq(op),
                    0x7A => self.op_moveq(op),
                    0x7C => self.op_moveq(op),
                    0x7E => self.op_moveq(op),
                    _ => self.op_ill(op),
                }
            }
            0x8 => {
                // 8xxx: DIV, OR, SUB
                match op >> 8 {
                     0x80 => self.op_or_byte(op, bus),
                    0x82 => self.op_or_long(op, bus),
                    0x84 => self.op_or_word(op, bus),
                    0x86 => self.op_div(op, bus),
                    0x88 => self.op_sub_byte(op, bus),
                    0x8A => self.op_sub_long(op, bus),
                    0x8C => self.op_sub_word(op, bus),
                    0x8E => self.op_subx_long(op),
                    _ => self.op_ill(op),
                }
            }
            0x9 => self.op_sub(op, bus),
            0xA => self.op_cmp_mem(op, bus),
            0xB => self.op_cmp(op, bus),        // CMPI, CMPM, CMP, EOR
            0xC => {
                match op >> 8 {
                     0xC0 => self.op_and_byte(op, bus),
                    0xC2 => self.op_and_long(op, bus),
                    0xC4 => self.op_and_word(op, bus),
                    0xC6 => self.op_mul_unsigned(op, bus),
                    0xC8 => self.op_eor_byte(op, bus),
                    0xCA => self.op_eor_long(op, bus),
                    0xCC => self.op_eor_word(op, bus),
                    0xCE => self.op_mul_signed(op, bus),
                    _ => self.op_ill(op),
                }
            }
            0xD => self.op_add(op, bus),
            0xE => {
                match op >> 12 {
                    0xE => {
                        // Shift/rotate
                        match op >> 8 {
                            0xE0..=0xE7 => self.op_asr(op),   // ASR
                            0xE8..=0xEF => self.op_lsr(op),    // LSR
                            0xF0..=0xF7 => self.op_roxr(op),   // ROXR
                            0xF8..=0xFF => self.op_ror(op),    // ROR
                            _ => self.op_ill(op),
                        }
                    }
                    _ => self.op_ill(op),
                }
            }
            0xF => self.op_ill(op),            // ColdFire coprocessor / line-F
            _ => self.op_ill(op),
        }
    }

    // ── Vastas operaçōes de MOVE ──────────────────────────────────────

    fn decode_ea(&mut self, mode: u16, reg: u16, size: u32, bus: &mut dyn BusInterface) -> Result<CfAddressing, CfError> {
        let pc = self.regs.pc;
        let idx = reg as usize;
        match mode {
            0 => Ok(CfAddressing::DataRegisterDirect { val: self.regs.d[idx], idx }),
            1 => Ok(CfAddressing::AddressRegisterDirect { val: self.regs.a[idx], idx }),
            2 => Ok(CfAddressing::AddressRegisterIndirect(self.regs.a[reg as usize])),
            3 => {
                let addr = self.regs.a[reg as usize];
                self.regs.a[reg as usize] = addr.wrapping_add(match size { 1 => 1, 2 => 2, _ => 4 });
                Ok(CfAddressing::AddressRegisterPostinc(addr))
            }
            4 => {
                let addr = self.regs.a[reg as usize].wrapping_sub(match size { 1 => 1, 2 => 2, _ => 4 });
                self.regs.a[reg as usize] = addr;
                Ok(CfAddressing::AddressRegisterPredec(addr))
            }
            5 => {
                let disp = bus.read_word(pc).ok_or(CfError::AccessFault(pc))? as i16;
                self.regs.pc = pc.wrapping_add(2);
                Ok(CfAddressing::AddressRegisterDisplacement(self.regs.a[reg as usize], disp))
            }
            6 => {
                let ext = bus.read_word(pc).ok_or(CfError::AccessFault(pc))?;
                self.regs.pc = pc.wrapping_add(2);
                let base = self.regs.a[reg as usize];
                let disp = (ext as i16) as i32;
                let idx_reg = ((ext >> 12) & 0x7) as usize;
                let idx_scale = if (ext >> 11) & 1 != 0 { if (ext >> 10) & 1 != 0 { 8 } else { 4 } } else { if (ext >> 10) & 1 != 0 { 2 } else { 1 } };
                let idx_val = if (ext >> 15) & 1 != 0 {
                    self.regs.d[idx_reg] as i32
                } else {
                    self.regs.a[idx_reg] as i32
                } * idx_scale;
                let addr = (base as i32).wrapping_add(disp).wrapping_add(idx_val) as u32;
                Ok(CfAddressing::AddressRegisterDisplacement(addr, 0))
            }
            7 => match reg {
                0 => {
                    let addr = bus.read_word(pc).ok_or(CfError::AccessFault(pc))? as i16 as u32;
                    self.regs.pc = pc.wrapping_add(2);
                    Ok(CfAddressing::AbsShort(addr))
                }
                1 => {
                    let hi = bus.read_half(pc).ok_or(CfError::AccessFault(pc))? as u32;
                    let lo = bus.read_half(pc.wrapping_add(2)).ok_or(CfError::AccessFault(pc.wrapping_add(2)))? as u32;
                    self.regs.pc = pc.wrapping_add(4);
                    Ok(CfAddressing::AbsLong((hi << 16) | lo))
                }
                2 => {
                    let disp = bus.read_word(pc).ok_or(CfError::AccessFault(pc))? as i16;
                    self.regs.pc = pc.wrapping_add(2);
                    Ok(CfAddressing::PCDisplacement(pc, disp))
                }
                3 => {
                    let ext = bus.read_word(pc).ok_or(CfError::AccessFault(pc))?;
                    self.regs.pc = pc.wrapping_add(2);
                    let base = pc;
                    let disp = (ext as i16) as i32;
                    let idx_reg = ((ext >> 12) & 0x7) as usize;
                    let idx_scale = if (ext >> 11) & 1 != 0 { if (ext >> 10) & 1 != 0 { 8 } else { 4 } } else { if (ext >> 10) & 1 != 0 { 2 } else { 1 } };
                    let idx_val = if (ext >> 15) & 1 != 0 {
                        self.regs.d[idx_reg] as i32
                    } else {
                        self.regs.a[idx_reg] as i32
                    } * idx_scale;
                    let addr = (base as i32).wrapping_add(disp).wrapping_add(idx_val) as u32;
                    Ok(CfAddressing::PCDisplacement(addr, 0))
                }
                4 => {
                    let hi = bus.read_half(pc).ok_or(CfError::AccessFault(pc))? as u32;
                    let lo = bus.read_half(pc.wrapping_add(2)).ok_or(CfError::AccessFault(pc.wrapping_add(2)))? as u32;
                    self.regs.pc = pc.wrapping_add(4);
                    Ok(CfAddressing::Immediate((hi << 16) | lo))
                }
                _ => Err(CfError::IllegalInstruction(0, pc)),
            },
            _ => Err(CfError::IllegalInstruction(0, pc)),
        }
    }

    fn ea_read(
        &self,
        ea: &CfAddressing,
        size: u32,
        bus: &mut dyn BusInterface,
    ) -> Result<u32, CfError> {
        match *ea {
            CfAddressing::DataRegisterDirect { val, .. } => Ok(match size {
                1 => val as u8 as u32,
                2 => val as u16 as u32,
                _ => val,
            }),
            CfAddressing::AddressRegisterDirect { val, .. } => Ok(val),
            CfAddressing::AddressRegisterIndirect(addr)
            | CfAddressing::AddressRegisterPostinc(addr)
            | CfAddressing::AddressRegisterPredec(addr)
            | CfAddressing::AbsShort(addr)
            | CfAddressing::AbsLong(addr)
            | CfAddressing::AddressRegisterDisplacement(addr, _)
            | CfAddressing::PCDisplacement(addr, _) => {
                match size {
                    1 => bus.read_byte(addr).ok_or(CfError::AccessFault(addr)).map(|v| v as u32),
                    2 => bus.read_half(addr).ok_or(CfError::AccessFault(addr)).map(|v| v as u32),
                    _ => bus.read_word(addr).ok_or(CfError::AccessFault(addr)),
                }
            }
            CfAddressing::Immediate(val) => Ok(val),
        }
    }

    fn ea_write(
        &mut self,
        ea: &CfAddressing,
        val: u32,
        size: u32,
        bus: &mut dyn BusInterface,
    ) -> Result<(), CfError> {
        match *ea {
            CfAddressing::DataRegisterDirect { idx, .. } => {
                match size {
                    1 => self.regs.d[idx] = (self.regs.d[idx] & !0xFF) | (val & 0xFF),
                    2 => self.regs.d[idx] = (self.regs.d[idx] & !0xFFFF) | (val & 0xFFFF),
                    _ => self.regs.d[idx] = val,
                }
                Ok(())
            }
            CfAddressing::AddressRegisterDirect { idx, .. } => {
                self.regs.a[idx] = val;
                Ok(())
            }
            CfAddressing::AddressRegisterIndirect(addr)
            | CfAddressing::AddressRegisterPostinc(addr)
            | CfAddressing::AddressRegisterPredec(addr)
            | CfAddressing::AbsShort(addr)
            | CfAddressing::AbsLong(addr)
            | CfAddressing::AddressRegisterDisplacement(addr, _)
            | CfAddressing::PCDisplacement(addr, _) => {
                match size {
                    1 => bus.write_byte(addr, val as u8).ok_or(CfError::AccessFault(addr)),
                    2 => bus.write_half(addr, val as u16).ok_or(CfError::AccessFault(addr)),
                    _ => bus.write_word(addr, val).ok_or(CfError::AccessFault(addr)),
                }
            }
            CfAddressing::Immediate(_) => {
                log::warn!("ColdFire: write to immediate — ignoring");
                Ok(())
            }
        }
    }

    fn ea_addr(&self, ea: &CfAddressing) -> Option<u32> {
        match *ea {
            CfAddressing::AddressRegisterIndirect(addr)
            | CfAddressing::AddressRegisterPostinc(addr)
            | CfAddressing::AddressRegisterPredec(addr)
            | CfAddressing::AbsShort(addr)
            | CfAddressing::AbsLong(addr)
            | CfAddressing::AddressRegisterDisplacement(addr, _)
            | CfAddressing::PCDisplacement(addr, _) => Some(addr),
            _ => None,
        }
    }

    // ── MOVE ──────────────────────────────────────────────────────────

    fn move_instr(&mut self, op: u16, size: u32, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        // MOVE <ea>, <ea>: bits 11-6 = dest EA, bits 5-0 = source EA
        // EA encoding: (mode << 3) | register
        let dest_ea = ((op >> 6) & 0x3F) as u16;
        let dst_mode = (dest_ea >> 3) & 0x07;
        let dst_reg = dest_ea & 0x07;
        let src_ea = (op & 0x3F) as u16;
        let src_mode = (src_ea >> 3) & 0x07;
        let src_reg = src_ea & 0x07;

        let src_ea = self.decode_ea(src_mode, src_reg, size, bus)?;
        let val = self.ea_read(&src_ea, size, bus)?;

        let dst_ea = self.decode_ea(dst_mode, dst_reg, size, bus)?;
        self.ea_write(&dst_ea, val, size, bus)?;

        // MOVE atualiza flags (N, Z, V, C)
        self.update_flags(val, size, false, false);
        Ok(1)
    }

    fn op_move_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.move_instr(op, 1, bus)
    }

    fn op_move_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.move_instr(op, 2, bus)
    }

    fn op_move_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.move_instr(op, 4, bus)
    }

    fn op_moveq(&mut self, op: u16) -> Result<u32, CfError> {
        let reg = ((op >> 9) & 0x07) as usize;
        let data = (op as i8) as u32;
        self.regs.d[reg] = data;
        self.update_flags(data, 4, false, false);
        Ok(1)
    }

    // ── Branch ────────────────────────────────────────────────────────

    fn get_cc_cond(&self, cond: u8) -> bool {
        let sr = self.regs.sr;
        let c = (sr >> 0) & 1;
        let v = (sr >> 1) & 1;
        let z = (sr >> 2) & 1;
        let n = (sr >> 3) & 1;
        match cond {
            0x0 => true,                     // T
            0x1 => false,                    // F
            0x2 => c == 0 && z == 0,         // HI
            0x3 => c == 1 || z == 1,         // LS
            0x4 => c == 0,                   // CC / HS
            0x5 => c == 1,                   // CS / LO
            0x6 => z == 0,                   // NE
            0x7 => z == 1,                   // EQ
            0x8 => v == 0,                   // VC
            0x9 => v == 1,                   // VS
            0xA => n == 0,                   // PL
            0xB => n == 1,                   // MI
            0xC => (n ^ v) == 0,             // GE
            0xD => (n ^ v) == 1,             // LT
            0xE => (z == 0) && ((n ^ v) == 0), // GT
            0xF => (z == 1) || ((n ^ v) == 1), // LE
            _ => false,
        }
    }

    fn op_bcc(&mut self, _op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let pc = self.regs.pc;
        let opword = bus.read_half(pc.wrapping_sub(2)).unwrap_or(0);
        let disp8 = opword as i8 as i32;
        let disp16 = bus.read_half(pc).unwrap_or(0) as i16 as i32;
        let cond = ((opword >> 8) & 0x0F) as u8;

        if self.get_cc_cond(cond) {
            let disp = if disp8 == 0 { self.regs.pc = pc.wrapping_add(2); disp16 } else { disp8 };
            self.regs.pc = self.regs.pc.wrapping_add(disp as u32);
        } else if disp8 == 0 {
            self.regs.pc = pc.wrapping_add(2); // skip the 16-bit displacement
        }
        Ok(1)
    }

    // ── Arithmetic ────────────────────────────────────────────────────

    fn update_flags(&mut self, val: u32, size: u32, carry: bool, overflow: bool) {
        let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
        let v = val & mask;
        let sr = &mut self.regs.sr;
        *sr &= 0xFFF0;
        if v == 0 { *sr |= 0x0004; }             // Z
        if (v as i32) < 0 { *sr |= 0x0008; }      // N
        if carry { *sr |= 0x0001; }               // C
        if overflow { *sr |= 0x0002; }            // V
    }

    // ── Interrupt handling ────────────────────────────────────────────

    fn take_interrupt(
        &mut self,
        bus: &mut dyn BusInterface,
        level: u8,
        vector: u8,
    ) -> Result<u32, CfError> {
        // ColdFire/68k stack frame: SR (word) then PC (long) pushed
        let sr = self.regs.sr;
        let pc = self.regs.pc;
        self.regs.a[7] = self.regs.a[7].wrapping_sub(2);
        bus.write_half(self.regs.a[7], sr).ok_or(CfError::AccessFault(self.regs.a[7]))?;
        self.regs.a[7] = self.regs.a[7].wrapping_sub(4);
        bus.write_word(self.regs.a[7], pc).ok_or(CfError::AccessFault(self.regs.a[7]))?;

        // Set interrupt mask to current level
        self.regs.sr = (self.regs.sr & 0xF8FF) | ((level as u16) << 8);

        // Vector to handler
        let vector_addr = (vector as u32) * 4;
        let new_pc = bus.read_word(vector_addr).ok_or(CfError::AccessFault(vector_addr))?;
        self.regs.pc = new_pc;

        if self.trace {
            log::debug!("CF IRQ level={} vector=0x{:02X} handler=0x{:08X}", level, vector, new_pc);
        }
        Ok(1)
    }

    // ── Instruções implementadas ──────────────────────────────────────

    fn op_ill(&self, op: u16) -> Result<u32, CfError> {
        Err(CfError::IllegalInstruction(op, self.regs.pc.wrapping_sub(2)))
    }

    // ── Bit ops ───────────────────────────────────────────────────────

    fn op_btst_static(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let bit = (op >> 9) & 0x07;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 1, bus)?;
        let val = self.ea_read(&ea, 1, bus)?;
        let z = (val >> bit) & 1;
        if z == 0 { self.regs.sr |= 0x0004; } else { self.regs.sr &= !0x0004; }
        Ok(1)
    }

    fn op_bchg_static(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let bit = (op >> 9) & 0x07;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 1, bus)?;
        let val = self.ea_read(&ea, 1, bus)?;
        let z = (val >> bit) & 1;
        if z == 0 { self.regs.sr |= 0x0004; } else { self.regs.sr &= !0x0004; }
        self.ea_write(&ea, val ^ (1 << bit), 1, bus)?;
        Ok(1)
    }

    fn op_bclr_static(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let bit = (op >> 9) & 0x07;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 1, bus)?;
        let val = self.ea_read(&ea, 1, bus)?;
        let z = (val >> bit) & 1;
        if z == 0 { self.regs.sr |= 0x0004; } else { self.regs.sr &= !0x0004; }
        self.ea_write(&ea, val & !(1 << bit), 1, bus)?;
        Ok(1)
    }

    fn op_bset_static(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let bit = (op >> 9) & 0x07;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 1, bus)?;
        let val = self.ea_read(&ea, 1, bus)?;
        let z = (val >> bit) & 1;
        if z == 0 { self.regs.sr |= 0x0004; } else { self.regs.sr &= !0x0004; }
        self.ea_write(&ea, val | (1 << bit), 1, bus)?;
        Ok(1)
    }

    fn op_bclr_dynamic(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let reg = (op >> 9) & 0x07;
        let bit = self.regs.d[reg as usize] & 0x1F;
        let mode = (op >> 3) & 0x07;
        let reg2 = op & 0x07;
        let ea = self.decode_ea(mode, reg2, 1, bus)?;
        let val = self.ea_read(&ea, 1, bus)?;
        let z = (val >> bit) & 1;
        if z == 0 { self.regs.sr |= 0x0004; } else { self.regs.sr &= !0x0004; }
        self.ea_write(&ea, val & !(1 << bit), 1, bus)?;
        Ok(1)
    }

    // ── LEA (Load Effective Address) ──────────────────────────────────

    fn op_lea(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let areg = ((op >> 9) & 0x07) as usize;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 4, bus)?;
        let addr = match &ea {
            CfAddressing::AddressRegisterIndirect(a)
            | CfAddressing::AddressRegisterDisplacement(a, _)
            | CfAddressing::AbsShort(a)
            | CfAddressing::AbsLong(a)
            | CfAddressing::PCDisplacement(a, _) => *a,
            _ => return Err(CfError::IllegalInstruction(op, self.regs.pc.wrapping_sub(4))),
        };
        // decode_ea já avançou PC pros extension words, então o addr está certo
        // Mas precisamos ajustar: se mode=7 reg=2 (PC-rel), o addr já é absoluto
        self.regs.a[areg] = addr;
        Ok(1)
    }

    // ── CHK ───────────────────────────────────────────────────────────

    fn op_chk(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let dn = (op >> 9) & 0x07;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 4, bus)?;
        let bound = self.ea_read(&ea, 4, bus)? as i32;
        let val = self.regs.d[dn as usize] as i32;
        if val < 0 || val > bound {
            // CHK exception — emulamos como trap
            return Err(CfError::IllegalInstruction(op, self.regs.pc.wrapping_sub(2)));
        }
        Ok(1)
    }

    // ── Status Register ───────────────────────────────────────────────

    fn op_move_from_sr(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 2, bus)?;
        self.ea_write(&ea, self.regs.sr as u32, 2, bus)?;
        Ok(1)
    }

    fn op_move_to_ccr(&mut self, op: u16) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        if mode == 0 {
            // MOVE Dn, CCR
            let dn = op & 0x07;
            self.regs.sr = (self.regs.sr & 0xFF00) | (self.regs.d[dn as usize] as u16 & 0x00FF);
        } else {
            // MOVE <ea>, CCR — simplified, assume data register
            let dn = op & 0x07;
            self.regs.sr = (self.regs.sr & 0xFF00) | (self.regs.d[dn as usize] as u16 & 0x00FF);
        }
        Ok(1)
    }

    fn op_move_to_sr(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let ea = self.decode_ea(mode, reg, 2, bus)?;
        let val = self.ea_read(&ea, 2, bus)?;
        self.regs.sr = val as u16;
        Ok(1)
    }

    // ── Data manipulation ─────────────────────────────────────────────

    fn op_swap(&mut self, op: u16) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let v = self.regs.d[dn];
        self.regs.d[dn] = (v << 16) | (v >> 16);
        self.update_flags(self.regs.d[dn], 4, false, false);
        Ok(1)
    }

    fn op_extw(&mut self, op: u16) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        self.regs.d[dn] = self.regs.d[dn] as i8 as i32 as u32;
        self.update_flags(self.regs.d[dn], 4, false, false);
        Ok(1)
    }

    // ── TST ───────────────────────────────────────────────────────────

    fn op_tst_word(&mut self, op: u16) -> Result<u32, CfError> {
        // Encoding for TST.W <ea> — simplified: only data register
        let dn = ((op >> 9) & 0x07) as usize;
        let val = self.regs.d[dn] as u16 as u32;
        self.update_flags(val, 2, false, false);
        Ok(1)
    }

    fn op_tst_long(&mut self, op: u16) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let val = self.regs.d[dn];
        self.update_flags(val, 4, false, false);
        Ok(1)
    }

    // ── CLR ───────────────────────────────────────────────────────────

    fn clr_instr(&mut self, op: u16, size: u32, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        if mode == 0 {
            let dn = reg as usize;
            match size {
                1 => self.regs.d[dn] &= !0xFF,
                2 => self.regs.d[dn] &= !0xFFFF,
                _ => self.regs.d[dn] = 0,
            }
        } else {
            let ea = self.decode_ea(mode, reg, size, bus)?;
            self.ea_write(&ea, 0, size, bus)?;
        }
        self.update_flags(0, size, false, false);
        Ok(1)
    }

    fn op_clr_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.clr_instr(op, 1, bus)
    }

    fn op_clr_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.clr_instr(op, 2, bus)
    }

    fn op_clr_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.clr_instr(op, 4, bus)
    }

    // ── NEG ───────────────────────────────────────────────────────────

    fn neg_instr(&mut self, op: u16, size: u32, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        if mode == 0 {
            let dn = reg as usize;
            let val = self.regs.d[dn];
            let res = (!val).wrapping_add(1);
            let (_, borrow) = val.overflowing_add(res);
            self.regs.d[dn] = match size { 1 => res & 0xFF, 2 => res & 0xFFFF, _ => res };
            self.update_flags(self.regs.d[dn], size, borrow, false);
        } else {
            let ea = self.decode_ea(mode, reg, size, bus)?;
            let val = self.ea_read(&ea, size, bus)?;
            let res = (!val).wrapping_add(1);
            let (_, borrow) = val.overflowing_add(res);
            self.ea_write(&ea, match size { 1 => res & 0xFF, 2 => res & 0xFFFF, _ => res }, size, bus)?;
            self.update_flags(match size { 1 => res & 0xFF, 2 => res & 0xFFFF, _ => res }, size, borrow, false);
        }
        Ok(1)
    }

    fn op_neg_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.neg_instr(op, 2, bus)
    }
    fn op_neg_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.neg_instr(op, 4, bus)
    }

    // ── NOT ───────────────────────────────────────────────────────────

    fn not_instr(&mut self, op: u16, size: u32, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        if mode == 0 {
            let dn = reg as usize;
            self.regs.d[dn] = match size { 1 => !self.regs.d[dn] & 0xFF, 2 => !self.regs.d[dn] & 0xFFFF, _ => !self.regs.d[dn] };
            self.update_flags(self.regs.d[dn], size, false, false);
        } else {
            let ea = self.decode_ea(mode, reg, size, bus)?;
            let val = self.ea_read(&ea, size, bus)?;
            let res = match size { 1 => !val & 0xFF, 2 => !val & 0xFFFF, _ => !val };
            self.ea_write(&ea, res, size, bus)?;
            self.update_flags(res, size, false, false);
        }
        Ok(1)
    }

    fn op_not_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.not_instr(op, 1, bus) }
    fn op_not_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.not_instr(op, 2, bus) }
    fn op_not_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.not_instr(op, 4, bus) }

    // ── Jump / Subroutine ─────────────────────────────────────────────

    fn op_jump(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        match mode {
            2 => {
                // JMP (An)
                self.regs.pc = self.regs.a[reg as usize];
            }
            5 => {
                // JMP (d,An)
                let disp = bus.read_word(self.regs.pc).unwrap_or(0) as i16;
                self.regs.pc = self.regs.pc.wrapping_add(2);
                self.regs.pc = (self.regs.a[reg as usize] as i32).wrapping_add(disp as i32) as u32;
            }
            7 if reg == 1 => {
                // JMP Abs.L
                let addr = bus.read_word(self.regs.pc).unwrap_or(0) as u32;
                self.regs.pc = self.regs.pc.wrapping_add(2);
                // Absolute long: next word is high 16 bits, then low 16 bits
                let hi = addr;
                let lo = bus.read_word(self.regs.pc).unwrap_or(0) as u32;
                self.regs.pc = (hi << 16) | lo;
            }
            7 if reg == 0 => {
                // JMP Abs.W
                let addr = bus.read_word(self.regs.pc).unwrap_or(0) as i16 as u32;
                self.regs.pc = self.regs.pc.wrapping_add(2);
                self.regs.pc = addr;
            }
            _ => {
                // JMP (An, Xi) etc — simplified
                return self.op_ill(op);
            }
        }
        Ok(1)
    }

    fn op_jsr(&mut self, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let op = bus.read_half(self.regs.pc.wrapping_sub(2)).unwrap_or(0);
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        // Push return address
        self.regs.a[7] = self.regs.a[7].wrapping_sub(4);
        bus.write_word(self.regs.a[7], self.regs.pc).ok_or(CfError::AccessFault(self.regs.a[7]))?;
        match mode {
            2 => self.regs.pc = self.regs.a[reg as usize],
            5 => {
                let disp = bus.read_word(self.regs.pc).unwrap_or(0) as i16;
                self.regs.pc = self.regs.pc.wrapping_add(2);
                self.regs.pc = (self.regs.a[reg as usize] as i32).wrapping_add(disp as i32) as u32;
            }
            7 if reg == 1 => {
                let hi = bus.read_word(self.regs.pc).unwrap_or(0) as u32;
                let lo = bus.read_word(self.regs.pc.wrapping_add(2)).unwrap_or(0) as u32;
                self.regs.pc = (hi << 16) | lo;
            }
            _ => return self.op_ill(op),
        }
        Ok(1)
    }

    fn op_bsr(&mut self, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let op = bus.read_half(self.regs.pc.wrapping_sub(2)).unwrap_or(0);
        let disp = if op & 0xFF != 0 {
            (op as i8) as i32
        } else {
            let ext = bus.read_word(self.regs.pc).unwrap_or(0) as i16 as i32;
            self.regs.pc = self.regs.pc.wrapping_add(2);
            ext
        };
        self.regs.a[7] = self.regs.a[7].wrapping_sub(4);
        bus.write_word(self.regs.a[7], self.regs.pc).ok_or(CfError::AccessFault(self.regs.a[7]))?;
        self.regs.pc = (self.regs.pc as i32).wrapping_add(disp) as u32;
        Ok(1)
    }

    fn op_rts(&mut self, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let addr = bus.read_word(self.regs.a[7]).ok_or(CfError::AccessFault(self.regs.a[7]))?;
        self.regs.a[7] = self.regs.a[7].wrapping_add(4);
        self.regs.pc = addr;
        Ok(1)
    }

    // ── LINK / UNLK ───────────────────────────────────────────────────

    fn op_link_unlk(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        if op & 0x08 != 0 {
            // UNLK An
            let an = ((op >> 9) & 0x07) as usize;
            self.regs.a[7] = self.regs.a[an];
            let addr = bus.read_word(self.regs.a[7]).ok_or(CfError::AccessFault(self.regs.a[7]))?;
            self.regs.a[7] = self.regs.a[7].wrapping_add(4);
            self.regs.a[an] = addr;
        } else {
            // LINK An, #disp
            let an = ((op >> 9) & 0x07) as usize;
            let disp = bus.read_word(self.regs.pc).unwrap_or(0) as i16 as i32;
            self.regs.pc = self.regs.pc.wrapping_add(2);
            self.regs.a[7] = self.regs.a[7].wrapping_sub(4);
            bus.write_word(self.regs.a[7], self.regs.a[an]).ok_or(CfError::AccessFault(self.regs.a[7]))?;
            self.regs.a[an] = self.regs.a[7];
            self.regs.a[7] = (self.regs.a[7] as i32).wrapping_add(disp) as u32;
        }
        Ok(1)
    }

    // ── RTE (Return from Exception) ──────────────────────────────────

    fn op_rte(&mut self, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let pc = bus.read_word(self.regs.a[7]).ok_or(CfError::AccessFault(self.regs.a[7]))?;
        self.regs.a[7] = self.regs.a[7].wrapping_add(4);
        let sr = bus.read_half(self.regs.a[7]).ok_or(CfError::AccessFault(self.regs.a[7]))?;
        self.regs.a[7] = self.regs.a[7].wrapping_add(2);
        self.regs.pc = pc;
        self.regs.sr = sr;
        Ok(1)
    }

    // ── STOP ──────────────────────────────────────────────────────────

    fn op_stop(&mut self, _op: u16) -> Result<u32, CfError> {
        // STOP é 2 words: opcode + imm16. Já avançamos PC em 2 no step(),
        // avance mais 2 para pular o operando.
        self.regs.pc = self.regs.pc.wrapping_add(2);
        self.halt = true;
        Ok(1)
    }

    // ── ADDQ / SUBQ ───────────────────────────────────────────────────

    fn addq_instr(&mut self, op: u16, size: u32, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let data = ((op >> 9) & 0x07) as u32;
        let data = if data == 0 { 8 } else { data };
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        if mode == 1 {
            // ADDQ to address register
            self.regs.a[reg as usize] = self.regs.a[reg as usize].wrapping_add(data);
            return Ok(1);
        }
        if mode == 0 {
            let dn = reg as usize;
            let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
            let val = self.regs.d[dn] & mask;
            let res = val.wrapping_add(data) & mask;
            self.regs.d[dn] = (self.regs.d[dn] & !mask) | res;
            self.update_flags(res, size, res < val, false);
        } else {
            let ea = self.decode_ea(mode, reg, size, bus)?;
            let val = self.ea_read(&ea, size, bus)?;
            let res = val.wrapping_add(data);
            self.ea_write(&ea, res & match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF }, size, bus)?;
            self.update_flags(res, size, res < val, false);
        }
        Ok(1)
    }

    fn subq_instr(&mut self, op: u16, size: u32, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let data = ((op >> 9) & 0x07) as u32;
        let data = if data == 0 { 8 } else { data };
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        if mode == 1 {
            self.regs.a[reg as usize] = self.regs.a[reg as usize].wrapping_sub(data);
            return Ok(1);
        }
        if mode == 0 {
            let dn = reg as usize;
            let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
            let val = self.regs.d[dn] & mask;
            let res = val.wrapping_sub(data) & mask;
            self.regs.d[dn] = (self.regs.d[dn] & !mask) | res;
            self.update_flags(res, size, val < data, false);
        } else {
            let ea = self.decode_ea(mode, reg, size, bus)?;
            let val = self.ea_read(&ea, size, bus)?;
            let res = val.wrapping_sub(data);
            self.ea_write(&ea, res & match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF }, size, bus)?;
            self.update_flags(res, size, val < data, false);
        }
        Ok(1)
    }

    fn op_addq_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.addq_instr(op, 1, bus) }
    fn op_addq_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.addq_instr(op, 2, bus) }
    fn op_addq_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.addq_instr(op, 4, bus) }
    fn op_subq_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.subq_instr(op, 1, bus) }
    fn op_subq_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.subq_instr(op, 2, bus) }
    fn op_subq_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.subq_instr(op, 4, bus) }

    // ── NEGX ──────────────────────────────────────────────────────────

    fn op_negx(&mut self, op: u16) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let x = ((self.regs.sr >> 4) & 1) as u32;
        let val = self.regs.d[dn];
        let res = (!val).wrapping_add(x);
        let borrow = val != 0 || x != 0;
        self.regs.d[dn] = res;
        self.update_flags(res, 4, borrow, false);
        if borrow { self.regs.sr |= 0x0010; } else { self.regs.sr &= !0x0010; }
        Ok(1)
    }

    // ── TAS ───────────────────────────────────────────────────────────

    fn op_tas(&mut self, op: u16) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let val = self.regs.d[dn] as u8;
        self.update_flags(val as u32, 1, false, false);
        self.regs.d[dn] |= 0x80; // set bit 7
        Ok(1)
    }

    // ── MOVE USP ──────────────────────────────────────────────────────

    fn op_move_usp(&mut self, op: u16) -> Result<u32, CfError> {
        let an = ((op >> 9) & 0x07) as usize;
        if op & 0x08 != 0 {
            // MOVE USP, An
            self.regs.a[an] = self.regs.a[7];
        } else {
            // MOVE An, USP
            self.regs.a[7] = self.regs.a[an];
        }
        Ok(1)
    }

    // ── MOVEM ─────────────────────────────────────────────────────────

    fn op_movem(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let dir = (op >> 10) & 1; // 0 = register to memory, 1 = memory to register
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let size = if op & 0x40 != 0 { 4 } else { 2 };

        let mask = bus.read_word(self.regs.pc).unwrap_or(0);
        self.regs.pc = self.regs.pc.wrapping_add(2);

        let ea = self.decode_ea(mode, reg, size, bus)?;
        let base = self.ea_addr(&ea).unwrap_or(0);

        if dir == 1 {
            // Memory → registers
            let mut addr = base;
            for i in 0..16 {
                if (mask >> (15 - i)) & 1 != 0 {
                    let val = if size == 4 {
                        bus.read_word(addr).ok_or(CfError::AccessFault(addr))?
                    } else {
                        bus.read_word(addr).ok_or(CfError::AccessFault(addr))? as u16 as u32
                    };
                    if i < 8 {
                        self.regs.d[i] = val;
                    } else {
                        self.regs.a[i - 8] = val;
                    }
                    addr = addr.wrapping_add(size);
                }
            }
            match mode {
                3 => { self.regs.a[reg as usize] = addr; } // (An)+
                _ => {}
            }
        } else {
            // Registers → memory
            let mut addr = if mode == 4 { base } else { base };
            let start = if mode == 4 { 15 } else { 0 };
            let end = if mode == 4 { 0 } else { 15 };
            let step: i32 = if mode == 4 { -1 } else { 1 };

            let mut i = start;
            for _ in 0..16 {
                if (mask >> (15 - i)) & 1 != 0 {
                    let val = if i < 8 { self.regs.d[i] } else { self.regs.a[i - 8] };
                    if size == 4 {
                        bus.write_word(addr, val).ok_or(CfError::AccessFault(addr))?;
                    } else {
                        bus.write_half(addr, val as u16).ok_or(CfError::AccessFault(addr))?;
                    }
                    addr = if step > 0 { addr.wrapping_add(size) } else { addr.wrapping_sub(size) };
                }
                i = (i as i32 + step) as usize;
                if (step > 0 && i > end) || (step < 0 && (i as i32) < end as i32) {
                    break;
                }
            }
        }
        Ok(1)
    }

    // ── MUL / DIV ─────────────────────────────────────────────────────

    fn op_div(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        let signed = (op >> 8) & 1 != 0; // DIVS vs DIVU
        if mode == 0 {
            let divisor = self.regs.d[reg as usize] as u16 as u32;
            let dividend = self.regs.d[dn];
            if divisor == 0 { return Err(CfError::IllegalInstruction(op, self.regs.pc.wrapping_sub(4))); }
            if signed {
                let quot = (dividend as i32).wrapping_div(divisor as i16 as i32);
                let rem = (dividend as i32).wrapping_rem(divisor as i16 as i32);
                self.regs.d[dn] = ((rem as u16 as u32) << 16) | (quot as u16 as u32);
            } else {
                let quot = dividend.wrapping_div(divisor);
                let rem = dividend.wrapping_rem(divisor);
                self.regs.d[dn] = ((rem & 0xFFFF) << 16) | (quot & 0xFFFF);
            }
        } else {
            let ea = self.decode_ea(mode, reg, 2, bus)?;
            let divisor = self.ea_read(&ea, 2, bus)?;
            let dividend = self.regs.d[dn];
            if divisor == 0 { return Err(CfError::IllegalInstruction(op, self.regs.pc.wrapping_sub(4))); }
            if signed {
                let quot = (dividend as i32).wrapping_div(divisor as i16 as i32);
                let rem = (dividend as i32).wrapping_rem(divisor as i16 as i32);
                self.regs.d[dn] = ((rem as u16 as u32) << 16) | (quot as u16 as u32);
            } else {
                let quot = dividend.wrapping_div(divisor & 0xFFFF);
                let rem = dividend.wrapping_rem(divisor & 0xFFFF);
                self.regs.d[dn] = ((rem & 0xFFFF) << 16) | (quot & 0xFFFF);
            }
        }
        Ok(1)
    }

    fn mul_instr(&mut self, op: u16, signed: bool, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let mode = (op >> 3) & 0x07;
        let reg = op & 0x07;
        if mode == 0 {
            let src = self.regs.d[reg as usize];
            let dst = self.regs.d[dn];
            let res = if signed {
                ((dst as i32 as i64) * (src as i16 as i32 as i64)) as u32
            } else {
                dst.wrapping_mul(src as u16 as u32)
            };
            self.regs.d[dn] = res;
        } else {
            let ea = self.decode_ea(mode, reg, 2, bus)?;
            let src = self.ea_read(&ea, 2, bus)?;
            let dst = self.regs.d[dn];
            let res = if signed {
                ((dst as i32 as i64) * (src as i16 as i32 as i64)) as u32
            } else {
                dst.wrapping_mul(src & 0xFFFF)
            };
            self.regs.d[dn] = res;
        }
        Ok(1)
    }

    fn op_mul_unsigned(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.mul_instr(op, false, bus)
    }
    fn op_mul_signed(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.mul_instr(op, true, bus)
    }

    // ── SCC ───────────────────────────────────────────────────────────

    fn op_scc(&mut self, op: u16, _bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let dn = ((op >> 9) & 0x07) as usize;
        let cond = ((op >> 8) & 0x0F) as u8;
        if self.get_cc_cond(cond) {
            self.regs.d[dn] = (self.regs.d[dn] & !0xFF) | 0xFF;
        } else {
            self.regs.d[dn] &= !0xFF;
        }
        Ok(1)
    }

    // ── Arithmetic (ADD / SUB / ADDA / SUBA) ──────────────────────────

    fn alu_arith(
        &mut self,
        op: u16,
        size: u32,
        is_sub: bool,
        bus: &mut dyn BusInterface,
    ) -> Result<u32, CfError> {
        let opmode = (op >> 6) & 0x03;
        let reg = ((op >> 9) & 0x07) as usize;
        let mode = (op >> 3) & 0x07;
        let r = op & 0x07;

        match opmode {
            0 | 2 => {
                // <ea> + Dn → Dn  (opmode 0: byte  2: word)
                let ea = self.decode_ea(mode, r, size, bus)?;
                let src = self.ea_read(&ea, size, bus)?;
                let dst = self.regs.d[reg];
                let res = if is_sub { dst.wrapping_sub(src) } else { dst.wrapping_add(src) };
                let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
                let masked_res = res & mask;
                self.regs.d[reg] = (dst & !mask) | masked_res;
                let carry = if is_sub { (dst & mask) < (src & mask) } else { res < dst };
                self.update_flags(masked_res, size, carry, false);
            }
            1 | 3 => {
                // Dn + <ea> → Dn (opmode 1: byte  3: word)
                // Actually for 0xD range: opmode 3 is ADDA (address)
                if mode == 1 || (mode == 7 && r <= 1) {
                    // ADDA/SUBA
                    let ea = self.decode_ea(mode, r, 4, bus)?;
                    let src = self.ea_read(&ea, 4, bus)?;
                    if is_sub {
                        self.regs.a[reg] = self.regs.a[reg].wrapping_sub(src);
                    } else {
                        self.regs.a[reg] = self.regs.a[reg].wrapping_add(src);
                    }
                    return Ok(1);
                }
                // Dn + <ea> → <ea>  (for memory destination)
                let dst_size = if opmode == 1 { 1 } else if opmode == 3 { 2 } else { 4 };
                let ea = self.decode_ea(mode, r, dst_size, bus)?;
                let val = self.ea_read(&ea, dst_size, bus)?;
                let src = self.regs.d[reg];
                let res = if is_sub { val.wrapping_sub(src) } else { val.wrapping_add(src) };
                let mask = match dst_size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
                self.ea_write(&ea, res & mask, dst_size, bus)?;
                let carry = if is_sub { (val & mask) < (src & mask) } else { (res & mask) < (val & mask) };
                self.update_flags(res & mask, dst_size, carry, false);
            }
            _ => {}
        }
        Ok(1)
    }

    fn alu_binary(
        &mut self,
        op: u16,
        size: u32,
        is_and: bool,
        is_or: bool,
        is_eor: bool,
        bus: &mut dyn BusInterface,
    ) -> Result<u32, CfError> {
        let reg = ((op >> 9) & 0x07) as usize;
        let r = op & 0x07;

        if is_eor {
            // EOR Dn, <ea>
            let mode = (op >> 3) & 0x07;
            let ea = self.decode_ea(mode, r, size, bus)?;
            let val = self.ea_read(&ea, size, bus)?;
            let res = val ^ self.regs.d[reg];
            let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
            self.ea_write(&ea, res & mask, size, bus)?;
            self.update_flags(res & mask, size, false, false);
            return Ok(1);
        }

        if is_and {
            // AND <ea>, Dn  (modes 0x0C range)
            let mode = (op >> 3) & 0x07;
            let r = op & 0x07;
            let ea = self.decode_ea(mode, r, size, bus)?;
            let src = self.ea_read(&ea, size, bus)?;
            let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
            self.regs.d[reg] = (self.regs.d[reg] & !mask) | (self.regs.d[reg] & src & mask);
            let res = self.regs.d[reg] & mask;
            self.update_flags(res, size, false, false);
            return Ok(1);
        }

        if is_or {
            // OR <ea>, Dn  (modes 0x08 range)
            let mode = (op >> 3) & 0x07;
            let r = op & 0x07;
            let ea = self.decode_ea(mode, r, size, bus)?;
            let src = self.ea_read(&ea, size, bus)?;
            let mask = match size { 1 => 0xFF, 2 => 0xFFFF, _ => 0xFFFF_FFFF };
            self.regs.d[reg] = (self.regs.d[reg] & !mask) | ((self.regs.d[reg] | src) & mask);
            let res = self.regs.d[reg] & mask;
            self.update_flags(res, size, false, false);
            return Ok(1);
        }

        Ok(1)
    }

    // ── CMP ───────────────────────────────────────────────────────────

    fn op_cmp(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let reg = ((op >> 9) & 0x07) as usize;
        let mode = (op >> 3) & 0x07;
        let r = op & 0x07;

        if mode == 1 {
            // CMPA.L <ea>, An
            let ea = self.decode_ea(mode, r, 4, bus)?;
            let src = self.ea_read(&ea, 4, bus)?;
            let dst = self.regs.a[reg];
            let res = dst.wrapping_sub(src);
            let overflow = (dst as i32) < 0 && (src as i32) >= 0 && (res as i32) >= 0
                || (dst as i32) >= 0 && (src as i32) < 0 && (res as i32) < 0;
            self.update_flags(res, 4, dst < src, overflow);
            return Ok(1);
        }
        if mode == 0 || (mode >= 2 && mode <= 7) {
            let ea = self.decode_ea(mode, r, 4, bus)?;
            let src = self.ea_read(&ea, 4, bus)?;
            let dst = self.regs.d[reg];
            let res = dst.wrapping_sub(src);
            let overflow = (dst as i32) < 0 && (src as i32) >= 0 && (res as i32) >= 0
                || (dst as i32) >= 0 && (src as i32) < 0 && (res as i32) < 0;
            self.update_flags(res, 4, dst < src, overflow);
        }
        Ok(1)
    }

    fn op_cmp_mem(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        // CMPM (An)+, (Am)+ — compare memory with auto-increment
        let ay = ((op >> 9) & 0x07) as usize;
        let ax = op & 0x07;
        let val_y = bus.read_word(self.regs.a[ay]).ok_or(CfError::AccessFault(self.regs.a[ay]))?;
        let val_x = bus.read_word(self.regs.a[ax as usize]).ok_or(CfError::AccessFault(self.regs.a[ax as usize]))?;
        self.regs.a[ay] = self.regs.a[ay].wrapping_add(4);
        self.regs.a[ax as usize] = self.regs.a[ax as usize].wrapping_add(4);
        let res = val_x.wrapping_sub(val_y);
        self.update_flags(res, 4, val_x < val_y, false);
        Ok(1)
    }

    // ── ADD (0xD) / SUB (0x9) dispatch ────────────────────────────────

    fn op_add(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let size = match (op >> 6) & 0x03 {
            0 => 1, // byte
            1 => 2, // word
            _ => 4, // long
        };
        self.alu_arith(op, size, false, bus)
    }

    fn op_sub(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        let size = match (op >> 6) & 0x03 {
            0 => 1,
            1 => 2,
            _ => 4,
        };
        self.alu_arith(op, size, true, bus)
    }

    // ── AND / OR / EOR ───────────────────────────────────────────────

    fn op_and_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 1, true, false, false, bus) }
    fn op_and_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 2, true, false, false, bus) }
    fn op_and_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 4, true, false, false, bus) }
    fn op_or_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 1, false, true, false, bus) }
    fn op_or_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 2, false, true, false, bus) }
    fn op_or_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 4, false, true, false, bus) }
    fn op_eor_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 1, false, false, true, bus) }
    fn op_eor_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 2, false, false, true, bus) }
    fn op_eor_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> { self.alu_binary(op, 4, false, false, true, bus) }

    // ── ADDX / SUBX ──────────────────────────────────────────────────

    fn op_addx_long(&mut self, _op: u16) -> Result<u32, CfError> {
        // ADDX.L Dn, Dm (simplified)
        Ok(1)
    }
    fn op_subx_long(&mut self, _op: u16) -> Result<u32, CfError> {
        Ok(1)
    }

    // ── Shifts ────────────────────────────────────────────────────────

    fn shift_instr(&mut self, op: u16, is_asr: bool, is_lsr: bool) -> Result<u32, CfError> {
        let dn = (op & 0x07) as usize;
        let count = if op & 0x20 != 0 {
            self.regs.d[((op >> 9) & 0x07) as usize] & 0x3F
        } else {
            ((op >> 9) & 0x07) as u32
        };
        if count == 0 { return Ok(1); }
        let val = self.regs.d[dn];
        let (res, carry) = if is_asr {
            let sign = val >> 31;
            let shifted = val >> count.min(31);
            if count >= 32 { (sign as u32, (val >> 31) & 1) } else { (shifted, (val >> (count - 1)) & 1) }
        } else if is_lsr {
            if count >= 32 { (0, 0) } else { (val >> count, (val >> (count - 1)) & 1) }
        } else {
            // ROR
            let count = count & 0x1F;
            let carry = (val >> (count - 1)) & 1;
            ((val >> count) | (val << (32 - count)), carry)
        };
        self.regs.d[dn] = res;
        self.update_flags(res, 4, carry != 0, false);
        Ok(1)
    }

    fn op_asr(&mut self, op: u16) -> Result<u32, CfError> { self.shift_instr(op, true, false) }
    fn op_lsr(&mut self, op: u16) -> Result<u32, CfError> { self.shift_instr(op, false, true) }
    fn op_roxr(&mut self, op: u16) -> Result<u32, CfError> { self.shift_instr(op, false, false) }
    fn op_ror(&mut self, op: u16) -> Result<u32, CfError> { self.shift_instr(op, false, false) }

    // ── ADD/SUB stubs (opcode 0x8 range already handled by alu_arith) ──
    // These are aliases for the same instruction mnemonics in different
    // encoding positions. The actual implementations are covered by
    // op_add/op_sub for the main 0xD/0x9 ranges.

    fn op_add_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.alu_arith(op, 1, false, bus)
    }
    fn op_add_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.alu_arith(op, 2, false, bus)
    }
    fn op_add_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.alu_arith(op, 4, false, bus)
    }
    fn op_sub_byte(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.alu_arith(op, 1, true, bus)
    }
    fn op_sub_word(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.alu_arith(op, 2, true, bus)
    }
    fn op_sub_long(&mut self, op: u16, bus: &mut dyn BusInterface) -> Result<u32, CfError> {
        self.alu_arith(op, 4, true, bus)
    }

    // ── NBCD ─────────────────────────────────────────────────────────

    fn op_nbcd(&mut self, _op: u16) -> Result<u32, CfError> {
        Ok(1) // BCD — não implementado no ColdFire real, mantido como NOP
    }
}
