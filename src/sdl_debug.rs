use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::render::Canvas;
use sdl2::video::Window;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::io::Write;

use crate::hardware::Cd32Hardware;

// ── Log capture ─────────────────────────────────────────────────────

static LOG_BUF: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

pub struct SdlLogger;
impl log::Log for SdlLogger {
    fn enabled(&self, _: &log::Metadata) -> bool { true }
    fn log(&self, record: &log::Record) {
        let msg = format!("[{}] {}", record.level(), record.args());
        // Tambem escreve no stderr (terminal)
        let _ = writeln!(std::io::stderr(), "{}", msg);
        // Captura para a janela de log
        if let Ok(mut buf) = LOG_BUF.lock() {
            buf.push_back(msg);
            if buf.len() > 500 { buf.pop_front(); }
        }
    }
    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

pub fn capture_log(buf: &mut Vec<String>) {
    if let Ok(mut log_buf) = LOG_BUF.lock() {
        buf.clear();
        buf.extend(log_buf.iter().cloned());
    }
}

// ── 8x8 bitmap font (ASCII 32-127) ─────────────────────────────────

const FONT8: &[u8] = include_bytes!("font8x8.bin");

fn font8_bitmap(c: u8) -> &'static [u8] {
    let idx = if c < 32 || c > 127 { 0 } else { (c - 32) as usize };
    &FONT8[idx * 8..idx * 8 + 8]
}

// ── Draw text on canvas ────────────────────────────────────────────

fn draw_char(canvas: &mut Canvas<Window>, c: u8, x: i32, y: i32, fg: Color, bg: Color) {
    let bmp = font8_bitmap(c);
    for row in 0..8 {
        for col in 0..8 {
            let px = (bmp[row] >> (7 - col)) & 1;
            canvas.set_draw_color(if px != 0 { fg } else { bg });
            let _ = canvas.draw_point(Point::new(x + col as i32, y + row as i32));
        }
    }
}

pub fn draw_text(canvas: &mut Canvas<Window>, x: i32, y: i32, text: &str, fg: Color, bg: Color) {
    let mut cx = x;
    let mut cy = y;
    for b in text.bytes() {
        if b == b'\n' { cx = x; cy += 10; continue; }
        draw_char(canvas, b, cx, cy, fg, bg);
        cx += 9;
    }
}

fn draw_text_clip(canvas: &mut Canvas<Window>, x: i32, y: i32, w: i32, text: &str, fg: Color, bg: Color) {
    let mut cx = x;
    let mut cy = y;
    for b in text.bytes() {
        if b == b'\n' { cx = x; cy += 10; if cy > y + w { break; } continue; }
        if cx + 9 > x + 640 { cx = x; cy += 10; }
        if cy > y + w { break; }
        draw_char(canvas, b, cx, cy, fg, bg);
        cx += 9;
    }
}

// ── Debug window rendering ─────────────────────────────────────────

pub fn render_debug_window(canvas: &mut Canvas<Window>, hw: &Cd32Hardware) {
    let bg = Color::RGB(16, 16, 32);
    let fg = Color::RGB(200, 200, 200);
    let hl = Color::RGB(100, 200, 255);
    let dim = Color::RGB(100, 100, 100);
    let warn = Color::RGB(255, 200, 100);
    let err = Color::RGB(255, 80, 80);
    let (w, _) = canvas.output_size().unwrap_or((640, 480));

    canvas.set_draw_color(bg);
    canvas.clear();

    let mut y = 4i32;
    let left = 4i32;

    macro_rules! line { ($color:expr, $fmt:expr $(, $arg:expr)*) => {{
        let s = format!($fmt $(, $arg)*);
        draw_text(canvas, left, y, &s, $color, bg);
        y += 10;
    }}}

    line!(hl, "CD32-RS  Debug");
    line!(dim, "────────────────────────────");

    line!(fg, "Cycles:  total={}  ppc={}  cf={}",
          hw.total_cycles, hw.ppc_cycles, hw.cf_cycles);
    line!(fg, "Boot: {}  Hold: {}",
          hw.boot_complete, hw.ppc_hold);

    let ppc_halt = if hw.ppc.halt { warn } else { fg };
    let cf_halt = if hw.coldfire.halt { warn } else { fg };
    line!(ppc_halt, "PPC: PC=0x{:08X} LR=0x{:08X} MSR=0x{:08X} {}",
          hw.ppc.regs.pc, hw.ppc.regs.lr, hw.ppc.regs.msr,
          if hw.ppc.halt { "HALT" } else { "" });
    line!(fg, "PPC: CR=0x{:08X} XER=0x{:08X} SRR0=0x{:08X}",
          hw.ppc.regs.cr, hw.ppc.regs.xer, hw.ppc.regs.srr0);
    line!(cf_halt, "CF:  PC=0x{:08X} SR=0x{:04X} {}",
          hw.coldfire.regs.pc, hw.coldfire.regs.sr,
          if hw.coldfire.halt { "HALT" } else { "" });
    line!(fg, "CF:  D0=0x{:08X} A0=0x{:08X} A7=0x{:08X}",
          hw.coldfire.regs.d[0], hw.coldfire.regs.a[0], hw.coldfire.regs.a[7]);

    y += 4;
    line!(dim, "── GPU ────────────────────");
    line!(fg, "Frame: {}  fb_addr=0x{:08X}  state={:?}",
          hw.bus.gpu.frame_count, hw.bus.gpu.fb_addr, hw.bus.gpu.state);
    line!(fg, "Display: {}  stride={}",
          hw.bus.gpu.display_enabled, hw.bus.gpu.fb_stride);

    y += 4;
    line!(dim, "── Peripherals ────────────");
    line!(fg, "CDROM: {:?}  Mailbox: cmd=0x{:02X} resp=0x{:08X}",
          hw.bus.cdrom.state, hw.bus.mailbox_cmd, hw.bus.mailbox_resp);
    line!(fg, "MIU: cfg=0x{:08X} stat=0x{:08X} arb=0x{:08X}",
          hw.bus.miu_cfg, hw.bus.miu_stat, hw.bus.miu_arb);

    // PPC registers D0-D7
    y += 4;
    line!(dim, "── PPC GPRs ───────────────");
    for i in (0..32).step_by(4) {
        line!(fg, "r{:02}=0x{:08X} r{:02}=0x{:08X} r{:02}=0x{:08X} r{:02}=0x{:08X}",
              i, hw.ppc.regs.gpr[i],
              i+1, hw.ppc.regs.gpr[(i+1) as usize],
              i+2, hw.ppc.regs.gpr[(i+2) as usize],
              i+3, hw.ppc.regs.gpr[(i+3) as usize]);
    }

    // CPU error status
    y += 4;
    let halt_count = if hw.ppc.halt { 1 } else { 0 } + if hw.coldfire.halt { 1 } else { 0 };
    if halt_count > 0 {
        line!(err, "⚠  CPU HALTED");
    }
}

// ── Log window rendering ───────────────────────────────────────────

pub fn render_log_window(canvas: &mut Canvas<Window>) {
    let bg = Color::RGB(8, 8, 12);
    let fg = Color::RGB(180, 200, 180);
    let dim = Color::RGB(80, 100, 80);
    let (w, h) = canvas.output_size().unwrap_or((640, 480));

    canvas.set_draw_color(bg);
    canvas.clear();

    let mut lines: Vec<String> = Vec::new();
    capture_log(&mut lines);

    let mut y = h as i32 - 10;
    for msg in lines.iter().rev() {
        if y < 0 { break; }
        let color = if msg.contains("ERROR") || msg.contains("error") {
            Color::RGB(255, 100, 80)
        } else if msg.contains("WARN") || msg.contains("warn") {
            Color::RGB(255, 200, 80)
        } else {
            fg
        };
        draw_text(canvas, 4, y, msg, color, bg);
        y -= 10;
    }
}

pub fn format_debug_text(hw: &Cd32Hardware, lines: &mut Vec<String>) {
    lines.push(format!("CD3²-rs Debug"));
    lines.push(format!("──────────────────────────"));
    lines.push(format!("Cycles:  total={}  ppc={}  cf={}",
          hw.total_cycles, hw.ppc_cycles, hw.cf_cycles));
    lines.push(format!("Boot: {}  Hold: {}", hw.boot_complete, hw.ppc_hold));
    lines.push(format!("PPC: PC={:08X} LR={:08X} MSR={:08X} {}",
          hw.ppc.regs.pc, hw.ppc.regs.lr, hw.ppc.regs.msr,
          if hw.ppc.halt { "HALT" } else { "" }));
    lines.push(format!("PPC: CR={:08X} XER={:08X} SRR0={:08X}",
          hw.ppc.regs.cr, hw.ppc.regs.xer, hw.ppc.regs.srr0));
    lines.push(format!("CF:  PC={:08X} SR={:04X} {}",
          hw.coldfire.regs.pc, hw.coldfire.regs.sr,
          if hw.coldfire.halt { "HALT" } else { "" }));
    lines.push(format!("CF:  D0={:08X} A0={:08X} A7={:08X}",
          hw.coldfire.regs.d[0], hw.coldfire.regs.a[0], hw.coldfire.regs.a[7]));
    lines.push(format!("GPU Frame={} fb={:08X} state={:?}",
          hw.bus.gpu.frame_count, hw.bus.gpu.fb_addr, hw.bus.gpu.state));
    lines.push(format!("CDROM={:?} Mailbox={:02X}/{:08X}",
          hw.bus.cdrom.state, hw.bus.mailbox_cmd, hw.bus.mailbox_resp));
    lines.push(format!("PPC halted={} CF halted={}", hw.ppc.halt, hw.coldfire.halt));
    for i in (0..32).step_by(4) {
        lines.push(format!("r{:02}={:08X} r{:02}={:08X} r{:02}={:08X} r{:02}={:08X}",
              i, hw.ppc.regs.gpr[i],
              i+1, hw.ppc.regs.gpr[(i+1) as usize],
              i+2, hw.ppc.regs.gpr[(i+2) as usize],
              i+3, hw.ppc.regs.gpr[(i+3) as usize]));
    }
}
