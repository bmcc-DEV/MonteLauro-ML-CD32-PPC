//! Gerador de ROM sintética para CD³².
//!
//! Modos:
//!   hello           — ROM "Hello CD³²" (validação de hardware)
//!   aros-bootstrap  — ROM com bootstrap AROS PPC
//!
//! Uso: gen_rom [--target hello|aros-bootstrap] [--kernel kernel.bin] [output.rom]

use std::fs;
use std::path::PathBuf;

// ── PPC instruction encodings ─────────────────────────────────────────

fn i_addi(rd: u32, ra: u32, si: i16) -> u32  { (14<<26)|(rd<<21)|(ra<<16)|(si as u16 as u32) }
fn i_addis(rd: u32, ra: u32, si: i16) -> u32 { (15<<26)|(rd<<21)|(ra<<16)|(si as u16 as u32) }
fn i_ori(rd: u32, ra: u32, ui: u16) -> u32   { (24<<26)|(rd<<21)|(ra<<16)|ui as u32 }
fn i_oris(rd: u32, ra: u32, ui: u16) -> u32  { (25<<26)|(rd<<21)|(ra<<16)|ui as u32 }
fn i_stw(rs: u32, d: i16, ra: u32) -> u32    { (36<<26)|(rs<<21)|(ra<<16)|(d as u16 as u32) }
fn i_lwz(rd: u32, d: i16, ra: u32) -> u32    { (32<<26)|(rd<<21)|(ra<<16)|(d as u16 as u32) }
fn i_cmpi(ra: u32, si: u16) -> u32           { (11<<26)|(ra<<16)|si as u32 }
fn i_bc(bo: u32, bi: u32, bd: u32) -> u32    { (16<<26)|(bo<<21)|(bi<<16)|((bd & 0x3FFF)<<2) }
fn i_b(target: u32, cur: u32) -> u32 {
    // I-form: LI at IBM bits 6-29 = u32 bits 25-2, AA=bit1, LK=bit0
    let delta = (target.wrapping_sub(cur)).wrapping_div(4);
    (18 << 26) | ((delta & 0x00FF_FFFF) << 2)
}
fn i_mtspr(spr: u32, rs: u32) -> u32 {
    // mtspr SPR, RS: opcd=31, xop=467
    let spr_lo = spr & 0x1F;
    let spr_hi = (spr >> 5) & 0x1F;
    (31<<26) | (rs<<21) | (spr_hi<<16) | (spr_lo<<11) | (467<<1)
}
fn i_mtmsr(rs: u32) -> u32 {
    // mtmsr RS: opcd=31, xop=178
    (31<<26) | (rs<<21) | (178<<1)
}
fn i_mfmsr(rd: u32) -> u32 {
    // mfmsr RD: opcd=31, xop=83
    (31<<26) | (rd<<21) | (83<<1)
}
fn i_rfi() -> u32 {
    // rfi: opcd=19, xop=50
    (19<<26) | (50<<1)
}
fn i_mtcrf(mask: u8, rs: u32) -> u32 {
    // mtcrf mask, RS: opcd=31, xop=144
    (31<<26) | (rs<<21) | ((mask as u32)<<12) | (144<<1)
}
fn i_mfcr(rd: u32) -> u32 {
    // mfcr RD: opcd=31, xop=19
    (31<<26) | (rd<<21) | (19<<1)
}

// ── ROM builder ──────────────────────────────────────────────────────

struct Rom(Vec<u8>);
impl Rom {
    fn new() -> Self { Self(vec![0u8; 512*1024]) }
    fn w16(&mut self, off: usize, v: u16) { self.0[off..off+2].copy_from_slice(&v.to_be_bytes()); }
    fn w32(&mut self, off: usize, v: u32) { self.0[off..off+4].copy_from_slice(&v.to_be_bytes()); }
    fn words(&mut self, off: usize, ws: &[u16]) { for (i,&w) in ws.iter().enumerate() { self.w16(off+i*2, w); } }
    fn ppc(&mut self, off: usize, code: &[u32]) { for (i,&w) in code.iter().enumerate() { self.w32(off+i*4, w); } }
    fn bin(&mut self, off: usize, data: &[u8]) {
        let end = (off + data.len()).min(self.0.len());
        self.0[off..end].copy_from_slice(&data[..end - off]);
    }
    fn save(&self, p: &PathBuf) {
        fs::write(p, &self.0).unwrap();
        println!("ROM: {} ({}B)", p.display(), self.0.len());
    }
}

// ── Build PPC "Hello CD³²" demo ──────────────────────────────────────

fn build_ppc_hello() -> Vec<u32> {
    let mut c = Vec::new();
    macro_rules! w { ($x:expr) => { c.push($x); } }

    // Spin: wait for ColdFire to write 1 to address 0
    w!(i_lwz(3, 0, 0));
    w!(i_cmpi(3, 1));
    // bc: bne (BO=4, BI=2=cr0_eq) BD=-3 → branch back to spin
    w!((16<<26) | (4<<21) | (2<<16) | (0xFFFD & 0x3FFF));

    w!(i_addis(1,0,0x0001));      // r1 = stack
    w!(i_ori(1,1,0));
    w!(i_addis(3,0,0x0400));      // r3 = GPU
    w!(i_ori(3,3,0));
    w!(i_addis(4,0,0x0401));      // r4 = VRAM
    w!(i_ori(4,4,0));
    w!(i_stw(4,4,3));             // GPU_LIST_ADDR
    w!(i_addis(5,0,0));
    w!(i_ori(5,5,1));
    w!(i_stw(5,0,3));             // GPU_CTRL = 1
    w!(i_addis(6,0,0x03D0));      // r6 = DSP
    w!(i_ori(6,6,0));
    w!(i_addis(7,0,0));
    w!(i_ori(7,7,0x00FF));
    w!(i_stw(7,0,6));             // DSP_CTRL = 0xFF
    w!(i_addis(8,0,0x0220));      // r8 = GPIO
    w!(i_ori(8,8,0x0020));

    let loop_adr = (0x100 + c.len()*4) as u32;
    w!(i_lwz(9,0,8));
    w!(i_addis(10,0,0x0100));
    w!(i_ori(10,10,0));
    w!(i_stw(9,0,10));
    w!(i_lwz(11,0x10,3));
    w!(i_b(loop_adr, (0x100 + c.len()*4 + 4) as u32));
    c
}

// ── Build ColdFire bootstrap "Hello" ─────────────────────────────────

fn build_cf_hello() -> Vec<u16> {
    let mut c = Vec::new();
    c.push(0x6000); c.push(0x4E71); // BRA.S placeholder, NOP
    let s = 0xFF00_0100u32;
    let d = 0x0000_0100u32;
    c.push(0x41F9); c.push((s>>16)as u16); c.push(s as u16);
    c.push(0x43F9); c.push((d>>16)as u16); c.push(d as u16);
    c.push(0x203C); c.push(0x0000); c.push(0x0100);
    let cp = c.len();
    c.push(0x2658); // MOVE.L (A0)+, (A1)+
    c.push(0x5900); // SUBQ.L #4, D0
    let cur = 0xFF00_0000 + (c.len()as u32)*2 + 2;
    let tgt = 0xFF00_0000 + (cp as u32)*2;
    c.push(0x6600 | ((tgt as i32 - cur as i32) as i8 as u16 & 0xFF));
    c.push(0x4280); // CLR.L D0
    c.push(0x2200); // MOVEA D0, A0
    c.push(0x243C); c.push(0x0000); c.push(0x0001); // MOVE.L #1, (A0)
    c.push(0x4E72); c.push(0x2000); // STOP
    c.push(0x60FE); // BRA *
    c[0] = 0x6002; // BRA.S +2
    c
}

// ── Build PPC AROS bootstrap ─────────────────────────────────────────

fn build_ppc_aros() -> Vec<u32> {
    let mut c = Vec::new();
    macro_rules! w { ($x:expr) => { c.push($x); } }

    // === Fase 1: Spin no handoff (ColdFire escreve 1 em addr 0) ===
    w!(i_lwz(3, 0, 0));
    w!(i_cmpi(3, 1));
    w!((16<<26) | (4<<21) | (2<<16) | (0xFFFD & 0x3FFF)); // bne

    // === Fase 2: Configurar BATs identity mapping ===
    // 4 BATs cobrindo 0..256MB, 256MB..512MB, 512MB..768MB, 768MB..1GB
    // IBAT0U = 0x0000_3FFF (BEPI=0, BL=256MB, Vs=Vp=1)
    // IBAT0L = 0x0000_0001 (BRPN=0, WIMG=0, PP=1)
    // DBAT0U = 0x0000_3FFF
    // DBAT0L = 0x0000_0001
    // (GLE: BL field: 0x3F = 63 → block size = 2^(63-31+16) = 2^48... hmm)
    // BL para 256MB: bl=17 (0x11) → bits 15-19 = 10001 = 0x11 << 15
    // BATU = BEPI(14:0) | BL(19:15) | Vs(30) | Vp(31)
    // BATL = BRPN(14:0) | WIMG | PP(31:30)

    // BL=17 → 2^17=128KB? No: BL=b + 17 → b = BL-17. Para 256MB=2^28:
    // BL = 28-17 = 11 = 0b01011 → bits 15-19 = 01011

    // BATs: identity mapping 0..256MB, BL=11 (256MB), Vs=1, Vp=1, PP=1
    // BAT0U = (BEPI & mask) | (BL << 2) | (Vs << 31) | (Vp << 30)
    //       = 0 | (11 << 2) | (1 << 31) | (1 << 30)
    //       = 44 | 0x80000000 | 0x40000000 = 0xC000_002C

    // IBAT0 — build via addis + ori (valores são constantes de config)
    // BAT0U = 0xC000_002C (BEPI=0, BL=11→256MB, Vs=1, Vp=1)
    // Precisamos carregar via addis+ori porque 0xC000002C não cabe em i16
    w!(i_addis(3, 0, (-0x4000i16)));
    w!(i_ori(3, 3, 0x002C));     // r3 = 0xC000_002C (BAT0U)
    w!(i_mtspr(528, 3));          // IBAT0U
    w!(i_addis(3, 0, 0));
    w!(i_ori(3, 3, 0x0001));     // r3 = BAT0L (BRPN=0, PP=1)
    w!(i_mtspr(529, 3));          // IBAT0L
    w!(i_mtspr(536, 3));          // DBAT0U = IBAT0U
    w!(i_mtspr(537, 3));          // DBAT0L = IBAT0L

    // === Fase 3: Stack pointer ===
    w!(i_addis(1, 0, 0x00FF));    // r1 = 0x00FF_0000
    w!(i_ori(1, 1, 0x0000));

    // === Fase 4: Construir struct CD32Platform na Chip RAM ===
    w!(i_addis(3, 0, 0x0100));   // r3 = 0x0100_0100 (&platform)
    w!(i_ori(3, 3, 0x0100));

    // magic = 0xCD32_0001
    w!(i_addis(4, 0, (-0x32CEi16)));
    w!(i_ori(4, 4, 0x0001));     // r4 = 0xCD32_0001
    w!(i_stw(4, 0, 3));
    // total_ram = 20MB = 0x013F_FFF8
    w!(i_addis(4, 0, 0x013F));
    w!(i_ori(4, 4, 0xFFF8));
    w!(i_stw(4, 4, 3));
    // chip_ram_base = 0x0100_0000
    w!(i_addis(4, 0, 0x0100));
    w!(i_stw(4, 8, 3));
    // chip_ram_size = 4MB
    w!(i_addis(4, 0, 0x0040));
    w!(i_stw(4, 12, 3));
    // sys_ram_base = 0
    w!(i_addis(4, 0, 0));
    w!(i_stw(4, 16, 3));
    // sys_ram_size = 16MB
    w!(i_addis(4, 0, 0x0100));
    w!(i_stw(4, 20, 3));
    // vram_base = 0x0401_0000
    w!(i_addis(4, 0, 0x0401));
    w!(i_stw(4, 24, 3));
    // vram_size = 8MB
    w!(i_addis(4, 0, 0x0080));
    w!(i_stw(4, 28, 3));
    // boot_rom_base = 0xFF00_0000
    w!(i_addis(4, 0, (-0x0100i16)));
    w!(i_stw(4, 32, 3));
    // boot_rom_size = 512KB
    w!(i_addis(4, 0, 0x0008));
    w!(i_stw(4, 36, 3));

    // Periféricos
    w!(i_addis(4, 0, 0x0100)); w!(i_stw(4, 40, 3)); // cf_mailbox
    w!(i_addis(4, 0, 0x0400)); w!(i_stw(4, 44, 3)); // gpu_base
    w!(i_addis(4, 0, 0x03D0)); w!(i_stw(4, 48, 3)); // dsp_base
    w!(i_addis(4, 0, 0x03E0)); w!(i_stw(4, 52, 3)); // dma_base
    w!(i_addis(4, 0, 0x0300)); w!(i_stw(4, 56, 3)); // cdrom_base
    w!(i_addis(4, 0, 0x0220)); w!(i_ori(4,4,0x0020)); w!(i_stw(4, 60, 3)); // gpio
    w!(i_addis(4, 0, 0x0220)); w!(i_stw(4, 64, 3)); // coldfire_base

    // === Fase 5: Passar params nos registradores ===
    w!(i_addis(4, 0, 0x0001));
    w!(i_ori(4, 4, 0x0001));     // r4 = CPUType
    w!(i_addis(5, 0, 0x013F));
    w!(i_ori(5, 5, 0xFFF8));     // r5 = MemSize
    w!(i_addis(6, 0, 0));
    w!(i_ori(6, 6, 0x0002));     // r6 = PlatformInfo
    w!(i_addis(7, 0, 0x0100));   // r7 = ColdFireMailbox
    w!(i_addis(8, 0, 0x0400));   // r8 = GPUBase
    w!(i_addis(9, 0, 0x0401));   // r9 = VRAMBase
    w!(i_addis(10, 0, 0x03D0));  // r10 = DSPBase
    w!(i_addis(11, 0, 0x03E0));  // r11 = DMABase
    w!(i_addis(12, 0, 0x0300));  // r12 = CDROMBase

    // === Fase 6: Jump para kernel AROS ===
    // Kernel em 0x0000_2000 (copiado da ROM pelo ColdFire)
    w!(i_b(0x2000, (0x100 + c.len()*4 + 4) as u32));

    c
}

// ── Build ColdFire AROS bootstrap ────────────────────────────────────

fn build_cf_aros() -> Vec<u16> {
    let mut c = Vec::new();
    c.push(0x6000); c.push(0x4E71);

    // Copia PPC bootstrap (0x100..0x2??) para RAM
    // e kernel AROS (0x10000 pra frente) para RAM em 0x2000
    // Para simplicidade, copiamos 8KB do ROM offset 0x100 para RAM 0x0100

    let src = 0xFF00_0100u32;
    let dst = 0x0000_0100u32;
    c.push(0x41F9); c.push((src>>16)as u16); c.push(src as u16);
    c.push(0x43F9); c.push((dst>>16)as u16); c.push(dst as u16);
    c.push(0x203C); c.push(0x0000); c.push(0x0800); // 2KB de PPC code

    let cp = c.len();
    c.push(0x2658);
    c.push(0x5900);
    let cur = 0xFF00_0000 + (c.len()as u32)*2 + 2;
    let tgt = 0xFF00_0000 + (cp as u32)*2;
    c.push(0x6600 | ((tgt as i32 - cur as i32) as i8 as u16 & 0xFF));

    // Copia kernel AROS da ROM (offset 0x10000) para RAM (0x0000_2000)
    let ksrc = 0xFF01_0000u32;
    let kdst = 0x0000_2000u32;
    c.push(0x41F9); c.push((ksrc>>16)as u16); c.push(ksrc as u16);
    c.push(0x43F9); c.push((kdst>>16)as u16); c.push(kdst as u16);
    c.push(0x203C); c.push(0x0002); c.push(0x0000); // 512KB max

    let kcp = c.len();
    c.push(0x2658);
    c.push(0x5900);
    let kcur = 0xFF00_0000 + (c.len()as u32)*2 + 2;
    let ktgt = 0xFF00_0000 + (kcp as u32)*2;
    c.push(0x6600 | ((ktgt as i32 - kcur as i32) as i8 as u16 & 0xFF));

    // Handoff
    c.push(0x4280); // CLR.L D0
    c.push(0x2200); // MOVEA D0, A0
    c.push(0x243C); c.push(0x0000); c.push(0x0001);
    c.push(0x4E72); c.push(0x2000); // STOP
    c.push(0x60FE);

    c[0] = 0x6002;
    c
}

// ── Main ─────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut target = "hello";
    let mut kernel_path: Option<PathBuf> = None;
    let mut out_path = PathBuf::from("hello_cd32.rom");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" if i+1 < args.len() => { target = &args[i+1]; i += 2; }
            "--kernel" if i+1 < args.len() => { kernel_path = Some(PathBuf::from(&args[i+1])); i += 2; }
            "--output" if i+1 < args.len() => { out_path = PathBuf::from(&args[i+1]); i += 2; }
            _ => { out_path = PathBuf::from(&args[i]); i += 1; }
        }
    }

    let mut rom = Rom::new();

    match target {
        "hello" => {
            let ppc = build_ppc_hello();
            let cf = build_cf_hello();
            rom.words(0x0000, &cf);
            rom.ppc(0x0100, &ppc);
            rom.w32(0x20000, 0x4C000064);
            rom.w16(0x7FFFE, 0xFEED);
            rom.save(&out_path);
            println!("PPC: {} instr  CF: {} words", ppc.len(), cf.len());
        }
        "aros-bootstrap" => {
            let ppc = build_ppc_aros();
            let cf = build_cf_aros();
            rom.words(0x0000, &cf);
            rom.ppc(0x0100, &ppc);

            // Kernel AROS opcional
            if let Some(kp) = kernel_path {
                if let Ok(kdata) = fs::read(&kp) {
                    let max_size = 512 * 1024 - 0x10000;
                    if kdata.len() <= max_size {
                        rom.bin(0x10000, &kdata);
                        println!("Kernel: {} ({} bytes)", kp.display(), kdata.len());
                    } else {
                        eprintln!("Kernel too large: {} > {}", kdata.len(), max_size);
                    }
                } else {
                    eprintln!("Warning: kernel file not found: {}", kp.display());
                }
            } else {
                // Kernel placeholder (stack of RFI instructions)
                for off in (0x10000..0x12000).step_by(4) {
                    rom.w32(off, 0x4C000064); // RFI as NOP
                }
                // Write AROS entry signature
                rom.w32(0x2000, 0x7C000000); // wait instruction
            }

            rom.w16(0x7FFFE, 0xFEED);
            rom.save(&out_path);
            println!("AROS bootstrap: PPC {} instr  CF {} words", ppc.len(), cf.len());
        }
        _ => {
            eprintln!("Unknown target: {}. Use 'hello' or 'aros-bootstrap'", target);
            std::process::exit(1);
        }
    }
}
