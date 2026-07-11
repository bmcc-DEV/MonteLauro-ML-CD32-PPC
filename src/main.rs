//! CD³²-rs — Amiga CD³² Community Emulator
//!
//! CLI para carregar BIOS, bootar o hardware, e executar (com ou sem frontend SDL).

use clap::Parser;
use std::path::PathBuf;

use cd32_rs::hardware::Cd32Hardware;

// CD³² Joypad bit positions (lido pelo ColdFire via GPIO 0x0220_0020)
const JOY_UP: u16 = 1 << 0;
const JOY_DOWN: u16 = 1 << 1;
const JOY_LEFT: u16 = 1 << 2;
const JOY_RIGHT: u16 = 1 << 3;
const JOY_A: u16 = 1 << 4;
const JOY_B: u16 = 1 << 5;
const JOY_START: u16 = 1 << 6;
const JOY_SELECT: u16 = 1 << 7;

#[derive(Parser)]
#[command(name = "cd32-rs", version, about = "Amiga CD³² Community Emulator")]
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

    if cli.verbose || cli.trace {
        std::env::set_var("RUST_LOG", "debug");
    } else {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

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
        match cd32_rs::save::load_state(&mut hw, load_path) {
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
        match cd32_rs::save::save_state(&hw, save_path) {
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
    log::info!("  PPC hold:         {}", false); // hw.ppc_hold is private
    log::info!("  ColdFire halted:  {}", hw.coldfire.halt);
    log::info!("  GPU frame count:  {}", hw.bus.gpu.frame_count);
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
    use sdl2::pixels::PixelFormatEnum;
    use std::time::Duration;

    let sdl = sdl2::init().expect("SDL init failed");
    let video = sdl.video().expect("SDL video init failed");
    let window = video.window("CD³²-rs", 640, 480)
        .position_centered()
        .build()
        .expect("Window creation failed");
    let mut canvas = window.into_canvas().build().expect("Canvas creation failed");
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator.create_texture_streaming(
        PixelFormatEnum::ARGB8888, 640, 480,
    ).expect("Texture creation failed");

    let mut running = true;
    let mut event_pump = sdl.event_pump().expect("event pump");

    canvas.clear();
    canvas.present();

    let mut joypad: u16 = 0;

    while running {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }
                Event::KeyDown { keycode: Some(k), .. } => {
                    joypad |= match k {
                        Keycode::Up => JOY_UP,
                        Keycode::Down => JOY_DOWN,
                        Keycode::Left => JOY_LEFT,
                        Keycode::Right => JOY_RIGHT,
                        Keycode::Z => JOY_A,
                        Keycode::X => JOY_B,
                        Keycode::Return => JOY_START,
                        Keycode::RShift | Keycode::LShift => JOY_SELECT,
                        _ => 0,
                    };
                }
                Event::KeyUp { keycode: Some(k), .. } => {
                    joypad &= !match k {
                        Keycode::Up => JOY_UP,
                        Keycode::Down => JOY_DOWN,
                        Keycode::Left => JOY_LEFT,
                        Keycode::Right => JOY_RIGHT,
                        Keycode::Z => JOY_A,
                        Keycode::X => JOY_B,
                        Keycode::Return => JOY_START,
                        Keycode::RShift | Keycode::LShift => JOY_SELECT,
                        _ => 0,
                    };
                }
                _ => {}
            }
        }

        hw.bus.set_joypad(joypad);

        // Simula ~16ms de hardware (1 frame a ~60fps)
        hw.run_cycles(4_400_000); // ~16ms a 266MHz

        // VRAM unificada (mesmo buffer que o guest escreve)
        let fb = hw.bus.framebuffer_rgba();
        texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
            for y in 0..480 {
                let src_start = y * 640 * 4;
                let src_end = src_start + (640 * 4).min(fb.len().saturating_sub(src_start));
                if src_start >= fb.len() {
                    break;
                }
                let src = &fb[src_start..src_end];
                let dst_start = y * pitch;
                let dst_end = dst_start + src.len().min(buffer.len().saturating_sub(dst_start));
                if dst_start < buffer.len() {
                    buffer[dst_start..dst_end].copy_from_slice(&src[..dst_end - dst_start]);
                }
            }
        });

        canvas.copy(&texture, None, None).expect("texture copy");
        canvas.present();

        std::thread::sleep(Duration::from_millis(16));
    }
}
