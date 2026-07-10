//! Gera ROM sintética "Hello CD³²" — valida hardware do emulador.
//! Uso: cargo run --bin gen-rom -- [output.rom]

use std::fs;
use std::path::PathBuf;

// ── PPC instruction encodings ─────────────────────────────────────────

fn i_addi(rd: u32, ra: u32, si: i16) -> u32  { (14<<26)|(rd<<21)|(ra<<16)|(si as u16 as u32) }
fn i_addis(rd: u32, ra: u32, si: i16) -> u32 { (15<<26)|(rd<<21)|(ra<<16)|(si as u16 as u32) }
fn i_ori(rd: u32, ra: u32, ui: u16) -> u32   { (24<<26)|(rd<<21)|(ra<<16)|ui as u32 }
fn i_stw(rs: u32, d: i16, ra: u32) -> u32    { (36<<26)|(rs<<21)|(ra<<16)|(d as u16 as u32) }
fn i_lwz(rd: u32, d: i16, ra: u32) -> u32    { (32<<26)|(rd<<21)|(ra<<16)|(d as u16 as u32) }
fn i_cmpi(ra: u32, si: u16) -> u32           { (11<<26)|(ra<<16)|si as u32 }
fn i_bc(bo: u32, bi: u32, bd: u32) -> u32    { (16<<26)|(bo<<21)|(bi<<16)|((bd & 0x3FFF)<<2) }
fn i_b(target: u32, cur: u32) -> u32 {
    ((target.wrapping_sub(cur)).wrapping_div(4) & 0x03FF_FFFF) | (18<<26)
}

// ── ROM builder ──────────────────────────────────────────────────────

struct Rom(Vec<u8>);
impl Rom {
    fn new() -> Self { Self(vec![0u8; 512*1024]) }
    fn w16(&mut self, off: usize, v: u16) { self.0[off..off+2].copy_from_slice(&v.to_be_bytes()); }
    fn w32(&mut self, off: usize, v: u32) { self.0[off..off+4].copy_from_slice(&v.to_be_bytes()); }
    fn words(&mut self, off: usize, ws: &[u16]) { for (i,&w) in ws.iter().enumerate() { self.w16(off+i*2, w); } }
    fn ppc(&mut self, off: usize, code: &[u32]) { for (i,&w) in code.iter().enumerate() { self.w32(off+i*4, w); } }
    fn save(&self, p: &PathBuf) { 
        fs::write(p, &self.0).unwrap(); 
        println!("ROM: {} ({}B)", p.display(), self.0.len()); 
    }
}

// ── Build PPC demo ────────────────────────────────────────────────────

fn build_ppc() -> Vec<u32> {
    let mut c = Vec::new();
    macro_rules! w { ($x:expr) => { c.push($x); } }

    // Spin: wait for ColdFire to write 1 to address 0
    let spin_loop = 0x100u32;
    w!(i_lwz(3, 0, 0));           // r3 = *(uint32*)0
    w!(i_cmpi(3, 1u16));          // compare r3 with 1
    let bc_val = i_bc(4, 2, 0xFFFD);
    eprintln!("DEBUG: bc_val=0x{:08X}", bc_val);
    w!(bc_val);       // bne spin_loop (disp = (0x100 - 0x10C) / 4 = -3 = 0xFFFD
                                   // but actually offset from current PC: target=0x100, cur=0x10C
                                   // wait: lwz at 0x100, cmpi at 0x104, bc at 0x108
                                   // target = 0x100, bc addr = 0x108, next instr = 0x10C
                                   // delta = (0x100 - 0x10C) / 4 = -12/4 = -3
                                   // 0xFFFD & 0x3FFF = 0x3FFD

    w!(i_addis(1,0,0x0001));      // r1 = 0x0001_0000 (stack)
    w!(i_ori(1,1,0));
    w!(i_addis(3,0,0x0400));      // r3 = GPU regs
    w!(i_ori(3,3,0));
    w!(i_addis(4,0,0x0401));      // r4 = VRAM base
    w!(i_ori(4,4,0));
    w!(i_stw(4,4,3));             // GPU_LIST_ADDR = VRAM
    w!(i_addis(5,0,0));
    w!(i_ori(5,5,1));
    w!(i_stw(5,0,3));             // GPU_CTRL = 1 (kick)
    w!(i_addis(6,0,0x03D0));      // r6 = DSP
    w!(i_ori(6,6,0));
    w!(i_addis(7,0,0));
    w!(i_ori(7,7,0x00FF));
    w!(i_stw(7,0,6));             // DSP_CTRL = 0xFF
    w!(i_addis(8,0,0x0220));      // r8 = GPIO
    w!(i_ori(8,8,0x0020));

    let loop_adr = (0x100 + c.len()*4) as u32;
    w!(i_lwz(9,0,8));             // r9 = joypad
    w!(i_addis(10,0,0x0100));
    w!(i_ori(10,10,0));
    w!(i_stw(9,0,10));            // mailbox area = joypad
    w!(i_lwz(11,0x10,3));         // r11 = GPU_FRAME
    let cur = (0x100 + c.len()*4 + 4) as u32;
    w!(i_b(loop_adr, cur));

    // Fix BC displacement for spin loop:
    // spin_loop is at 0x100 (instr 0 → lwz)
    // bc at 0x108 (instr 2)
    // delta = (0x100 - 0x10C) / 4 = -3 = 0x3FFD (14-bit signed)
    c[2] = (16<<26) | (4<<21) | (2<<16) | (0xFFFD & 0x3FFF);
    // BO=4 (bne), BI=2 (CR0 EQ), BD=0xFFFD (-3)
    // But wait: the Δ is from the current instruction's address to the target, and BD is
    // added to the NIA (next instruction address = PC of next instr).
    // Target = 0x100, NIA (next) = 0x10C, delta = 0x100 - 0x10C = -12
    // BD = (delta / 4) & 0x3FFF = -3 & 0x3FFF = 0x3FFD
    // Signed 14-bit: 0x3FFD = -3 in 14-bit two's complement ✓

    c
}

// ── Build ColdFire bootstrap ──────────────────────────────────────────

fn build_cf() -> Vec<u16> {
    let mut c = Vec::new();

    // Words 0-1: BRA.S copyloop + padding
    c.push(0x6000); c.push(0x4E71);

    // Real code at word index 2 (ROM offset 4)
    let src = 0xFF00_0100u32;
    let dst = 0x0000_0100u32;
    c.push(0x41F9); c.push((src>>16) as u16); c.push(src as u16);   // LEA src, A0
    c.push(0x43F9); c.push((dst>>16) as u16); c.push(dst as u16);   // LEA dst, A1
    c.push(0x203C); c.push(0x0000); c.push(0x0100);                 // MOVE.L #256, D0

    // copy loop (word index 10)
    let copy_pos = c.len();
    c.push(0x2658); // MOVE.L (A0)+, (A1)+
    c.push(0x5900); // SUBQ.L #4, D0
    let cur_ad = 0xFF00_0000 + (c.len() as u32)*2 + 2;
    let tgt_ad = 0xFF00_0000 + (copy_pos as u32)*2;
    let disp = (tgt_ad as i32 - cur_ad as i32) as i8 as u16;
    c.push(0x6600 | (disp & 0xFF));
    // CLR.L D0 + MOVE.L D0, A0 (A0 = 0)
    c.push(0x4280); // CLR.L D0
    c.push(0x2200); // MOVEA.L D0, A0
    // MOVE.L #0xCD32, (A0) — escreve handoff signature na RAM[0]
    c.push(0x243C); c.push(0x0000); c.push(0x0001);
    // Enter companion mode: STOP + infinite loop
    c.push(0x4E72); c.push(0x2000); // STOP #$2000 (SR=0x2000, ints enabled)
    c.push(0x60FE);                  // BRA * (infinite loop, safe)

    // Fix BRA.S: jump from 0xFF00_0002 to 0xFF00_0004 (2 bytes)
    c[0] = 0x6002;

    c
}

fn main() {
    // Debug: verify encoding directly
    let bc_test = i_bc(4, 2, 0xFFFD);
    if bc_test != 0x4082FFF4 {
        panic!("BC encoding mismatch: 0x{:08X} != 0x4082FFF4", bc_test);
    }
    eprintln!("BC encoding verified: 0x{:08X}", bc_test);
    let out = std::env::args().nth(1).map(PathBuf::from).unwrap_or(PathBuf::from("hello_cd32.rom"));
    let mut rom = Rom::new();
    let ppc = build_ppc();
    let cf = build_cf();

    rom.words(0x0000, &cf);
    rom.ppc(0x0100, &ppc);

    // Kickstart stubs
    rom.w32(0x20000, 0x4C000064);
    rom.w16(0x7FFFE, 0xFEED);

    rom.save(&out);
    println!("PPC: {} instr  CF: {} words", ppc.len(), cf.len());
}
