//! Disassembler PPC + ColdFire.
//!
//! Decodifica instruções para formato textual legível, usado pelo --trace.

// ── PPC Disassembler ─────────────────────────────────────────────────

fn ppc_rd(insn: u32) -> u32 { (insn >> 21) & 0x1F }
fn ppc_ra(insn: u32) -> u32 { (insn >> 16) & 0x1F }
fn ppc_rb(insn: u32) -> u32 { (insn >> 11) & 0x1F }
fn ppc_rs(insn: u32) -> u32 { (insn >> 21) & 0x1F }
fn ppc_si(insn: u32) -> i16 { insn as i16 }
fn ppc_ui(insn: u32) -> u16 { insn as u16 }
fn ppc_bo(insn: u32) -> u32 { (insn >> 21) & 0x1F }
fn ppc_bi(insn: u32) -> u32 { (insn >> 16) & 0x1F }

fn crf_name(n: u32) -> &'static str {
    ["cr0","cr1","cr2","cr3","cr4","cr5","cr6","cr7"][(n & 7) as usize]
}

fn gpr(r: u32) -> String {
    if r == 0 { "r0".into() } else { format!("r{}", r) }
}

fn bo_name(bo: u32) -> &'static str {
    match bo & 0x1F {
        0 => "dnz",
        4 => "ne",
        12 => "eq",
        16 => "dnz",
        20 => "ne",
        _ => "??",
    }
}

fn bi_name(bi: u32) -> String {
    let crf = bi >> 2;
    let bit = bi & 3;
    let b = ["lt","gt","eq","so"][bit as usize];
    format!("{}_{}", crf_name(crf), b)
}

pub fn disasm_ppc(insn: u32) -> String {
    let opcd = (insn >> 26) & 0x3F;
    match opcd {
        3 => format!("addi   {}, {}, {}", gpr(ppc_rd(insn)), gpr(ppc_ra(insn)), ppc_si(insn)),
        4 => format!("addic. {}, {}, {}", gpr(ppc_rd(insn)), gpr(ppc_ra(insn)), ppc_si(insn)),
        7 => format!("mulli  {}, {}, {}", gpr(ppc_rd(insn)), gpr(ppc_ra(insn)), ppc_si(insn)),
        10 => format!("cmpli  {}, {}, 0x{:04X}", crf_name(ppc_rd(insn)>>2), gpr(ppc_ra(insn)), ppc_ui(insn)),
        11 => format!("cmpi   {}, {}, {}", crf_name(ppc_rd(insn)>>2), gpr(ppc_ra(insn)), ppc_si(insn)),
        14 => format!("addi   {}, {}, {}", gpr(ppc_rd(insn)), gpr(ppc_ra(insn)), ppc_si(insn)),
        15 => format!("addis  {}, {}, {}", gpr(ppc_rd(insn)), gpr(ppc_ra(insn)), ppc_si(insn)),
        16 => {
            let bo = ppc_bo(insn);
            let bi = ppc_bi(insn);
            let bd = (((insn >> 2) & 0x3FFF) as i16) << 2 >> 2;
            let aa = (insn >> 1) & 1;
            let lk = insn & 1;
            let bname = bo_name(bo);
            let target = if aa == 1 { format!("0x{:X}", (bd << 2) as u32) } else { format!("{}", bd) };
            format!("bc     {}, {}, {} {} {}", bname, bi_name(bi), target, if aa==1{"|a"}else{""}, if lk==1{"|l"}else{"   "})
        }
        18 => {
            let li = ((insn >> 2) & 0x3FF_FFFF) as u32;
            let aa = (insn >> 1) & 1;
            let lk = insn & 1;
            let target = if aa == 1 { format!("0x{:X}", li << 2) } else { format!("0x{:X}", li) };
            format!("b{}     {}", if lk==1{"l"}else{""}, target)
        }
        19 => {
            let xop = (insn >> 1) & 0x3FF;
            if xop == 50 { return "rfi".into(); }
            let bo = ppc_bo(insn);
            let bi = ppc_bi(insn);
            let bd = (((insn >> 2) & 0x3FFF) as i16) << 2 >> 2;
            let lk = insn & 1;
            format!("bc{}   {}, {}, {}", if lk==1{"l"}else{""}, bo_name(bo), bi_name(bi), bd)
        }
        20 => {
            let rs = ppc_rs(insn);
            let ra = ppc_ra(insn);
            let sh = (insn >> 11) & 0x1F;
            let mb = (insn >> 6) & 0x1F;
            let me = (insn >> 1) & 0x1F;
            format!("rlwimi {}, {}, {}, {}, {}", gpr(ra), gpr(rs), sh, mb, me)
        }
        21 => {
            let rs = ppc_rs(insn);
            let ra = ppc_ra(insn);
            let sh = (insn >> 11) & 0x1F;
            let mb = (insn >> 6) & 0x1F;
            let me = (insn >> 1) & 0x1F;
            format!("rlwinm {}, {}, {}, {}, {}", gpr(ra), gpr(rs), sh, mb, me)
        }
        24 => format!("ori    {}, {}, 0x{:04X}", gpr(ppc_ra(insn)), gpr(ppc_rd(insn)), ppc_ui(insn)),
        25 => format!("oris   {}, {}, 0x{:04X}", gpr(ppc_ra(insn)), gpr(ppc_rd(insn)), ppc_ui(insn)),
        28 => format!("andi.  {}, {}, 0x{:04X}", gpr(ppc_ra(insn)), gpr(ppc_rd(insn)), ppc_ui(insn)),
        31 => {
            let xop = (insn >> 1) & 0x3FF;
            let rd = ppc_rd(insn);
            let ra = ppc_ra(insn);
            let rb = ppc_rb(insn);
            let rs = ppc_rs(insn);
            let _rc = insn & 1;
            match xop {
                0b0000001000 => format!("subfc  {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0000100011 => format!("or     {}, {}, {}", gpr(ra), gpr(rs), gpr(rb)),
                0b0000100111 => format!("nor    {}, {}, {}", gpr(ra), gpr(rs), gpr(rb)),
                0b0001000100 => format!("and    {}, {}, {}", gpr(ra), gpr(rs), gpr(rb)),
                0b0001000111 => format!("andc   {}, {}, {}", gpr(ra), gpr(rs), gpr(rb)),
                0b0001011000 => format!("neg    {}, {}", gpr(rd), gpr(ra)),
                0b0001100000 => format!("cntlzw {}, {}", gpr(ra), gpr(rs)),
                0b0001110100 => format!("add    {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0010000000 => format!("subf   {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0010011011 => format!("subfe  {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0010101010 => format!("addme  {}, {}", gpr(rd), gpr(ra)),
                0b0010101100 => format!("mulhw  {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0010101110 => format!("mulhwu {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0011111011 => format!("mullw  {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0111011011 => format!("divw   {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0111110111 => format!("divwu  {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0000010010 => format!("mfsr   {}, {}", gpr(rd), ppc_ra(insn) & 0xF),
                0b0000110110 => format!("mtsr   {}, {}", ppc_ra(insn) & 0xF, gpr(rs)),
                0b0010000010 => format!("cmp    {}, {}, {}", crf_name(ppc_rd(insn)>>2), gpr(ra), gpr(rb)),
                0b0010000011 => format!("cmpl   {}, {}, {}", crf_name(ppc_rd(insn)>>2), gpr(ra), gpr(rb)),
                0b0010010111 => format!("mtmsr  {}", gpr(rs)),
                0b0100000110 => format!("mfmsr  {}", gpr(rd)),
                0b0010010000 => format!("mtcrf  0x{:02X}, {}", ((insn>>12)&0xFF) as u8, gpr(rs)),
                0b0000010011 => format!("mfcr   {}", gpr(rd)),
                0b0010011001 => format!("lwzx   {}, {}, {}", gpr(rd), gpr(ra), gpr(rb)),
                0b0010001001 => format!("stwx   {}, {}, {}", gpr(rs), gpr(ra), gpr(rb)),
                0b0010010010 => format!("mtspr  {}, {}", (insn >> 11)&0x3FF, gpr(rd)),
                0b0101010011 => format!("mfspr  {}, {}", gpr(rd), (insn >> 11)&0x3FF),
                _ => format!("x-form xop={}", xop),
            }
        }
        32 => format!("lwz    {}, {}({})", gpr(ppc_rd(insn)), insn as i16, gpr(ppc_ra(insn))),
        33 => format!("lwzu   {}, {}({})", gpr(ppc_rd(insn)), insn as i16, gpr(ppc_ra(insn))),
        36 => format!("stw    {}, {}({})", gpr(ppc_rs(insn)), insn as i16, gpr(ppc_ra(insn))),
        37 => format!("stwu   {}, {}({})", gpr(ppc_rs(insn)), insn as i16, gpr(ppc_ra(insn))),
        _ => format!("opcd={}", opcd),
    }
}

// ── ColdFire / M68k Disassembler ─────────────────────────────────────

fn cf_dn(n: u16) -> String { format!("d{}", n & 7) }
fn cf_an(n: u16) -> String { format!("a{}", n & 7) }

fn cf_ea_mode(insn: u16) -> (u16, u16) { ((insn >> 3) & 7, insn & 7) }

fn cf_ea_string(mode: u16, reg: u16, _size: u16, ext_word: Option<u16>) -> String {
    match mode {
        0 => cf_dn(reg),
        1 => cf_an(reg),
        2 => format!("({})", cf_an(reg)),
        3 => format!("({})+", cf_an(reg)),
        4 => format!("-({})", cf_an(reg)),
        5 => format!("({}, ${:04X})", cf_an(reg), ext_word.unwrap_or(0) as i16),
        6 => format!("({}, ${:04X}.w)", cf_an(reg), ext_word.unwrap_or(0)),
        7 => match reg {
            0 => format!("${:04X}.w", ext_word.unwrap_or(0)),
            1 => {
                let hi = ext_word.unwrap_or(0) as u32;
                let lo = ext_word.unwrap_or(0) as u32 >> 16;
                format!("${:08X}.l", (hi << 16) | lo)
            }
            2 => format!("${:04X}(pc)", ext_word.unwrap_or(0) as i16),
            3 => format!("${:04X}(pc, ...)", ext_word.unwrap_or(0)),
            4 => format!("#${:04X}", ext_word.unwrap_or(0)),
            _ => "??".into(),
        },
        _ => "??".into(),
    }
}

fn cf_bcc_name(cond: u16) -> &'static str {
    match cond {
        0x0 => "bt",  0x1 => "bf",  0x2 => "bhi", 0x3 => "bls",
        0x4 => "bcc", 0x5 => "bcs", 0x6 => "bne", 0x7 => "beq",
        0x8 => "bvc", 0x9 => "bvs", 0xA => "bpl", 0xB => "bmi",
        0xC => "bge", 0xD => "blt", 0xE => "bgt", 0xF => "ble",
        _ => "b??",
    }
}

pub fn disasm_cf(op: u16) -> String {
    let top = op >> 12;
    match top {
        0 => {
            match op >> 8 {
                0x00 => format!("ori.b  #${:02X}, d0", op & 0xFF),
                0x01 => format!("btst   #${:x}, ...", (op>>9)&7),
                0x02 => format!("bchg   #${:x}, ...", (op>>9)&7),
                0x03 => format!("bclr   #${:x}, ...", (op>>9)&7),
                0x04 => format!("bset   #${:x}, ...", (op>>9)&7),
                _ => format!("bitop  0x{:04X}", op),
            }
        }
        0x1 => format!("move.b ...{}", cf_ea_string((op>>3)&7, op&7, 1, None)),
        0x2 => format!("move.l ...{}", cf_ea_string((op>>3)&7, op&7, 4, None)),
        0x3 => format!("move.w ...{}", cf_ea_string((op>>3)&7, op&7, 2, None)),
        0x4 => {
            let upper = (op >> 8) & 0xFF;
            match upper {
                0x40 => {
                    let (m, r) = cf_ea_mode(op);
                    format!("lea    {}", cf_ea_string(m, r, 4, None))
                }
                0x41 => {
                    let (m, r) = cf_ea_mode(op);
                    format!("chk    {}", cf_ea_string(m, r, 2, None))
                }
                0x42 => format!("clr.b  ..."),
                0x43 => format!("move   sr, ..."),
                0x44 => format!("move   ..., ccr"),
                0x46 => format!("move   ..., sr"),
                0x48 => format!("swap   {}", cf_dn(op)),
                0x49 => format!("ext.w  {}", cf_dn(op)),
                0x4A => format!("tst.w  ..."),
                0x4B => format!("tst.l  ..."),
                0x4C => format!("div{}  ...", if (op>>8)&1!=0{"s"}else{"u"}),
                0x4E => match op & 0xFF {
                    0x71 => "nop".into(),
                    0x72 => format!("stop   #${:04X}", op),
                    0x73 => "rte".into(),
                    0x75 => "rts".into(),
                    0x80..=0xBF => format!("jsr    ..."),
                    _ => format!("jmp    ..."),
                },
                0x4F => {
                    if op & 8 != 0 { format!("unlk   {}", cf_an(op>>9)) }
                    else { format!("link   {}, ...", cf_an(op>>9)) }
                }
                0x50 => "stop".into(),
                0x54..=0x5C => format!("addq   ..."),
                0x60 => "negx".into(),
                0x62 => format!("clr.b  {}", cf_dn(op)),
                0x64 => format!("clr.w  {}", cf_dn(op)),
                0x66 => format!("clr.l  {}", cf_dn(op)),
                0x68 => format!("neg.w  {}", cf_dn(op)),
                0x6A => format!("neg.l  {}", cf_dn(op)),
                0x6C => format!("not.b  {}", cf_dn(op)),
                0x6E => format!("not.w  {}", cf_dn(op)),
                0x70 => format!("not.l  {}", cf_dn(op)),
                0x7A => "tas    ...".into(),
                0x80 => "move   usp, ...".into(),
                0x88 => format!("movem  ..."),
                0xC0..=0xCE => format!("and    ..."),
                0xD0..=0xD6 => format!("add    ..."),
                0xD8..=0xDE => format!("sub    ..."),
                _ => format!("$04xx  0x{:04X}", op),
            }
        }
        0x5 => {
            let cond = (op >> 8) & 0xF;
            format!("s{}    ...", cf_bcc_name(cond))
        }
        0x6 => {
            let cond = (op >> 8) & 0xF;
            let disp = op as i8;
            format!("{}     ${:x}", cf_bcc_name(cond), disp)
        }
        0x7 => {
            if (op & 0xFF00) == 0x7000 {
                format!("moveq  #${:x}, {}", op as i8 as i8, cf_dn(op>>9))
            } else {
                format!("$07xx  ...")
            }
        }
        0x8 => format!("or/sub ..."),
        0x9 => format!("sub    ..."),
        0xA => format!("cmpm   ..."),
        0xB => format!("cmp    ..."),
        0xC => format!("and/eor..."),
        0xD => format!("add    ..."),
        0xE => format!("shift  ..."),
        0xF => format!("$0Fxx  ..."),
        _ => format!("????   0x{:04X}", op),
    }
}
