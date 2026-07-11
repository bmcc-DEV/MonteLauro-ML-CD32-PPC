//! CD³²-rs — Amiga CD³² Community Emulator
//!
//! CLI para carregar BIOS, bootar o hardware, e executar (com ou sem frontend SDL).

use clap::Parser;
use std::path::PathBuf;

use ml_gd2_rs::hardware::Cd32Hardware;

// CD³² Joypad bit positions (lido pelo ColdFire via GPIO 0x0220_0020)
#[cfg(feature = "sdl-frontend")]
const JOY_UP: u16 = 1 << 0;
#[cfg(feature = "sdl-frontend")]
const JOY_DOWN: u16 = 1 << 1;
#[cfg(feature = "sdl-frontend")]
const JOY_LEFT: u16 = 1 << 2;
#[cfg(feature = "sdl-frontend")]
const JOY_RIGHT: u16 = 1 << 3;
#[cfg(feature = "sdl-frontend")]
const JOY_A: u16 = 1 << 4;
#[cfg(feature = "sdl-frontend")]
const JOY_B: u16 = 1 << 5;
#[cfg(feature = "sdl-frontend")]
const JOY_START: u16 = 1 << 6;
#[cfg(feature = "sdl-frontend")]
const JOY_SELECT: u16 = 1 << 7;

#[derive(Parser)]
#[command(name = "ml-gd2-rs", version, about = "MonteLauro CD+G² Emulator")]
struct Cli {
    /// Caminho para o dump da Kickstart ROM (512KB)
    #[arg(short = 'b', long = "bios", default_value = "kickstart.rom")]
    bios: PathBuf,

    /// Imagem de CD (ISO9660 .bin/.iso) para montar
    #[arg(short = 'd', long = "disc")]
    disc: Option<PathBuf>,

    /// Número de ciclos a executar (0 = boot completo)
    #[arg(short = 'c', long = "cycles", default_value = "0")]
    cycles: u64,

    /// Modo verbose
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Trace de instruções PPC e ColdFire
    #[arg(long = "trace")]
    trace: bool,

    /// Habilita frontend SDL (requer feature sdl-frontend)
    #[arg(long = "sdl")]
    sdl: bool,

    /// Caminho para salvar estado (save state)
    #[arg(long = "save-state")]
    save_state: Option<PathBuf>,

    /// Caminho para carregar estado (save state)
    #[arg(long = "load-state")]
    load_state: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    // Logger: se SDL, usamos o logger customizado que captura para a janela de log.
    // Senao, usamos env_logger normal.
    if cli.verbose || cli.trace {
        std::env::set_var("RUST_LOG", "debug");
    } else {
        std::env::set_var("RUST_LOG", "info");
    }
    if !cli.sdl {
        env_logger::init();
    }

    // Carrega BIOS
    let bios_data = match std::fs::read(&cli.bios) {
        Ok(data) => {
            log::info!("Loaded BIOS: {} ({} bytes)", cli.bios.display(), data.len());
            data
        }
        Err(e) => {
            log::warn!("Could not load BIOS from {}: {}", cli.bios.display(), e);
            log::warn!("Using empty ROM (512KB of zeros)");
            vec![0u8; 512 * 1024]
        }
    };

    // Cria hardware
    let mut hw = Cd32Hardware::new(bios_data);
    if cli.trace {
        hw.set_trace(true, true);
    }

    // Load state (se fornecido, pula boot)
    if let Some(load_path) = &cli.load_state {
        match ml_gd2_rs::save::load_state(&mut hw, load_path) {
            Ok(_) => log::info!("State loaded, skipping boot"),
            Err(e) => log::error!("Failed to load state: {}", e),
        }
    }

    // Monta disco opcional
    if let Some(disc_path) = &cli.disc {
        if let Ok(data) = std::fs::read(disc_path) {
            log::info!("Loaded disc image: {} ({} bytes)", disc_path.display(), data.len());
            hw.bus.cdrom.insert_disc(data);
        } else {
            log::error!("Failed to load disc image: {}", disc_path.display());
        }
    }

    // Boot ou execução manual
    if cli.cycles == 0 {
        if cli.load_state.is_none() {
            hw.boot();
        }
        print_status(&hw);
    } else {
        hw.run_cycles(cli.cycles);
    }

    // Save state
    if let Some(save_path) = &cli.save_state {
        match ml_gd2_rs::save::save_state(&hw, save_path) {
            Ok(_) => log::info!("State saved to {}", save_path.display()),
            Err(e) => log::error!("Failed to save state: {}", e),
        }
    }

    // Frontend SDL (opcional)
    if cli.sdl {
        #[cfg(feature = "sdl-frontend")]
        run_sdl_frontend(hw);
        #[cfg(not(feature = "sdl-frontend"))]
        log::warn!("SDL frontend not available (compile with --features sdl-frontend)");
    } else {
        log::info!("Emulation complete. Use --sdl for graphical output.");
    }
}

fn print_status(hw: &Cd32Hardware) {
    log::info!("═══════════════════════════════════════");
    log::info!("  CD³² System Status");
    log::info!("═══════════════════════════════════════");
    log::info!("  Total cycles:     {}", hw.total_cycles);
    log::info!("  PPC cycles:       {}", hw.ppc_cycles);
    log::info!("  ColdFire cycles:  {}", hw.cf_cycles);
    log::info!("  Boot complete:    {}", hw.boot_complete);
    log::info!("  PPC halted:       {}", hw.ppc.halt);
    log::info!("  PPC hold:         {}", hw.ppc_hold);
    log::info!("  ColdFire halted:  {}", hw.coldfire.halt);
    log::info!("  GPU frame count:  {}", hw.bus.gpu.frame_count);
    log::info!("  CDROM state:      {:?}", hw.bus.cdrom.state);
    log::info!("  CDROM state:      {:?}", hw.bus.cdrom.state);
    log::info!("  Mailbox cmd:      0x{:02X}", hw.bus.mailbox_cmd);
    log::info!("  Mailbox resp:     0x{:08X}", hw.bus.mailbox_resp);
    log::info!("  MIU config:       0x{:08X}", hw.bus.miu_cfg);
    log::info!("═══════════════════════════════════════");

    // Dump PPC registers
    log::info!("  PPC: PC=0x{:08X} LR=0x{:08X} MSR=0x{:08X}",
        hw.ppc.regs.pc, hw.ppc.regs.lr, hw.ppc.regs.msr);

    // Dump ColdFire registers
    log::info!("  CF:  PC=0x{:08X} SR=0x{:04X}",
        hw.coldfire.regs.pc, hw.coldfire.regs.sr);
}

#[cfg(feature = "sdl-frontend")]
fn run_sdl_frontend(mut hw: Cd32Hardware) {
    use sdl2::event::Event;
    use sdl2::keyboard::Keycode;
    use sdl2::keyboard::Mod;
    use sdl2::mouse::MouseButton;
    use sdl2::pixels::PixelFormatEnum;
    use std::time::Duration;

    use ml_gd2_rs::sdl_debug::{SdlLogger, render_debug_window, render_log_window, capture_log};

    log::set_boxed_logger(Box::new(SdlLogger))
        .expect("SdlLogger already set (try sem --sdl primeiro)");
    log::set_max_level(log::LevelFilter::Debug);
    log::info!("CD³²-rs SDL frontend iniciado");

    let sdl = sdl2::init().expect("SDL init failed");
    let video = sdl.video().expect("SDL video init failed");

    const WW: u32 = 640;
    const WH: u32 = 480;
    const DW: u32 = 500;
    const DH: u32 = 640;
    const LW: u32 = 640;
    const LH: u32 = 400;

    let win = video.window("CD²-rs  ← Clique p/ selecionar, direito p/ copiar", WW, WH)
        .position(50, 50).build().expect("Main window");
    let main_id = win.id();
    let mut canvas = win.into_canvas().build().expect("Main canvas");
    let tc = canvas.texture_creator();
    let mut tex = tc.create_texture_streaming(PixelFormatEnum::ARGB8888, 640, 480)
        .expect("Texture");

    let dbg_w = video.window("CD²-rs Debug  [clique direito = copiar]", DW, DH)
        .position(50 + WW as i32 + 20, 50).build().expect("Debug window");
    let dbg_id = dbg_w.id();
    let mut dbg_canvas = dbg_w.into_canvas().build().expect("Debug canvas");

    let log_w = video.window("CD²-rs Log  [clique direito = copiar]", LW, LH)
        .position(50, 50 + WH as i32 + 20).build().expect("Log window");
    let log_id = log_w.id();
    let mut log_canvas = log_w.into_canvas().build().expect("Log canvas");

    let mut running = true;
    let mut ep = sdl.event_pump().expect("event pump");

    // Janela ativa para copia: 0=main, 1=debug, 2=log
    let mut active: u8 = 0;
    let mut joypad: u16 = 0;

    fn copy_text(text: &str) {
        use std::process::Command;
        let _ = Command::new("sh").args(["-c", &format!("echo -n {} | xclip -selection clipboard", sh_escape(text))]).output();
        // fallback: xclip sem echo
        let _ = Command::new("xclip").args(["-selection", "clipboard"]).arg(text).output();
        log::info!("Copied {} bytes to clipboard", text.len());
    }

    fn sh_escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 4);
        out.push('\'');
        for c in s.chars() {
            if c == '\'' { out.push_str("'\\''"); } else { out.push(c); }
        }
        out.push('\'');
        out
    }

    while running {
        for event in ep.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }
                // Clique do mouse: seleciona janela e copia se for direito
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, window_id, .. } => {
                    if window_id == main_id { active = 0; }
                    else if window_id == dbg_id { active = 1; }
                    else if window_id == log_id { active = 2; }
                    log::debug!("Window {} selected (active={})", window_id, active);
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Right, window_id, .. } => {
                    let text = if window_id == dbg_id {
                        let mut lines = Vec::new();
                        cd32_rs::sdl_debug::format_debug_text(&hw, &mut lines);
                        lines.join("\n")
                    } else if window_id == log_id {
                        let mut lines = Vec::new();
                        capture_log(&mut lines);
                        lines.join("\n")
                    } else {
                        format!(
                            "CD²-rs\nPPC: PC={:08X} LR={:08X}  CF: PC={:08X} SR={:04X}\nGPU frames={}  Boot={}",
                            hw.ppc.regs.pc, hw.ppc.regs.lr,
                            hw.coldfire.regs.pc, hw.coldfire.regs.sr,
                            hw.bus.gpu.frame_count, hw.boot_complete
                        )
                    };
                    copy_text(&text);
                }
                _ => {}
            }
            // Joypad tambem via teclado
            if let Event::KeyDown { keycode: Some(k), .. } = event {
                joypad |= match k {
                    Keycode::Up => JOY_UP, Keycode::Down => JOY_DOWN,
                    Keycode::Left => JOY_LEFT, Keycode::Right => JOY_RIGHT,
                    Keycode::Z => JOY_A, Keycode::X => JOY_B,
                    Keycode::Return => JOY_START,
                    Keycode::RShift | Keycode::LShift => JOY_SELECT,
                    _ => 0,
                };
            }
            if let Event::KeyUp { keycode: Some(k), .. } = event {
                joypad &= !match k {
                    Keycode::Up => JOY_UP, Keycode::Down => JOY_DOWN,
                    Keycode::Left => JOY_LEFT, Keycode::Right => JOY_RIGHT,
                    Keycode::Z => JOY_A, Keycode::X => JOY_B,
                    Keycode::Return => JOY_START,
                    Keycode::RShift | Keycode::LShift => JOY_SELECT,
                    _ => 0,
                };
            }
        }

        hw.bus.set_joypad(joypad);
        hw.run_cycles(4_400_000);

        // Render main
        let fb = hw.bus.framebuffer_rgba();
        let _ = tex.with_lock(None, |buf: &mut [u8], pitch: usize| {
            for y in 0..480 {
                let src_start = y * 640 * 4;
                let src_end = fb.len().min(src_start + 640 * 4);
                if src_start >= fb.len() { break; }
                let src = &fb[src_start..src_end];
                let dst_start = y * pitch;
                let dst_end = buf.len().min(dst_start + src.len());
                if dst_start >= buf.len() { break; }
                let dst = &mut buf[dst_start..dst_end];
                for i in 0..src.len() / 4 {
                    dst[i*4+0] = src[i*4+3]; dst[i*4+1] = src[i*4+2];
                    dst[i*4+2] = src[i*4+1]; dst[i*4+3] = 0xFF;
                }
            }
        });
        canvas.copy(&tex, None, None).expect("copy");
        canvas.present();

        // Debug
        render_debug_window(&mut dbg_canvas, &hw);
        // Borda verde se ativa
        dbg_canvas.set_draw_color(if active == 1 { sdl2::pixels::Color::RGB(0, 255, 80) } else { sdl2::pixels::Color::RGB(60, 60, 60) });
        let _ = dbg_canvas.draw_rect(sdl2::rect::Rect::new(0, 0, DW, DH));
        dbg_canvas.present();

        // Log
        render_log_window(&mut log_canvas);
        log_canvas.set_draw_color(if active == 2 { sdl2::pixels::Color::RGB(0, 255, 80) } else { sdl2::pixels::Color::RGB(60, 60, 60) });
        let _ = log_canvas.draw_rect(sdl2::rect::Rect::new(0, 0, LW, LH));
        log_canvas.present();

        std::thread::sleep(Duration::from_millis(16));
    }
}
