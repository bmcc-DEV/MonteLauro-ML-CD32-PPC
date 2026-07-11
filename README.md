# MonteLauro CD³²

**MonteLauro CD³²** — Plataforma aberta de videogame baseada no console "phantom" Amiga CD³².

Este repositório contém o emulador cycle-accurate, a game runtime (`libcd32.a`),
o SDK e as ferramentas de build para desenvolvimento de jogos na plataforma
MonteLauro ML-CD32-PPC.

**Repositório:** [github.com/bmcc-DEV/MonteLauro-ML-CD32-PPC](https://github.com/bmcc-DEV/MonteLauro-ML-CD32-PPC)

**CI:** `make ci` — pipeline completo local, sem dependência externa.

---

## Funcionalidades

| Componente | Status |
|------------|--------|
| Emulador PPC603e + ColdFire V4e cycle-accurate | ✅ |
| GPU TBDR "Lisa II" (tile-based, 640×480) | ✅ |
| Áudio DSP 8 canais estéreo | ✅ |
| DMA 4 canais (CDROM > GPU > Audio > CF) | ✅ |
| CD-ROM 12x + ISO9660 parser | ✅ |
| Interrupt controller (8 níveis) | ✅ |
| Save states | ✅ |
| Disassembler PPC + ColdFire (`--trace`) | ✅ |
| Frontend SDL | ✅ |
| **Game Runtime (libcd32.a)** — kernel C para jogos | ✅ |
| **Game demo** — retângulos + input + contador de frames | ✅ |
| **ISO mastering** — empacota jogo em ISO9660 | ✅ |
| **ROM generator** — `--target game` com kernel real | ✅ |
| **Docker PPC toolchain** — powerpc-linux-gnu-gcc (5min) | ✅ |
| **Bootstrap AROS** — boot chain alternativa (legacy) | ✅ |

## Pipeline "liga o console → joga"

```bash
# 1. Build kernel + demo (via Docker, 5min setup)
make docker-build             # toolchain PPC
make -C kernel demo           # kernel/demo.elf + kernel/demo.bin

# 2. Gerar ROM bootável
cargo run --bin gen-rom -- --target game \
  --kernel kernel/demo.bin --output rom/game_cd32.rom

# 3. Empacotar demo em ISO
tools/mkcd.sh kernel/demo.elf rom/jogo.iso

# 4. Bootar no emulador
cargo run --release -- --bios rom/game_cd32.rom \
  --disc rom/jogo.iso --sdl
```

O kernel (`kernel/kernel.c`) inicializa hardware, monta CD-ROM, carrega
`GAME.ELF` via ISO9660 + ELF loader, e pula para `game_main()`.

---

## Compilando

```bash
# Emulador
cargo build --release

# Com frontend SDL (requer SDL2 dev libs)
cargo build --release --features sdl-frontend
```

## Executando

```bash
# Pipeline completo de validação
make ci

# Boot RAM "Hello CD³²" (valida hardware)
make sdl-hello

# Boot game demo
make sdl-game

# Boot AROS bootstrap
make sdl-aros

# Trace com disassembler
make trace-hello CYCLES=50000

# Save / Load state
make save
make load
```

### CLI

```
Usage: cd32-rs [OPTIONS]

Options:
  -b, --bios <BIOS>        Caminho para a ROM (512KB)
  -d, --disc <DISC>        Imagem de CD (ISO9660)
  -c, --cycles <CYCLES>    Número de ciclos
  -v, --verbose            Modo verbose
      --trace              Trace com disassembler
      --sdl                Frontend SDL
      --save-state <PATH>  Salvar estado
      --load-state <PATH>  Carregar estado
```

## ABI (Application Binary Interface)

A struct `CD32Platform` contém o mapa de hardware completo, passada ao kernel
via registrador r3 (conforme `docs/aros/abi.md`).

```bash
make headers        # → include/cd32_platform.h + src/cd32_abi.rs
cargo run --bin check-abi   # valida conformidade de offsets
```

## Makefile

```bash
make build                    # Compila emulador
make headers                  # Gera headers ABI
make check-abi                # Valida conformidade
make rom-hello                # ROM "Hello CD³²"
make rom-game                 # ROM com game kernel
make test-hello               # Testa hello
make test-game                # Testa game demo
make trace-hello              # Trace hello
make sdl-game                 # Frontend gráfico com demo
make docker-build             # Builda toolchain PPC (Docker)
make docker-kernel            # Builda kernel + gera ROM via Docker
make ci                       # Pipeline completo
make clean                    # Limpa artefatos
```

## Estrutura

```
MonteLauro-ML-CD32-PPC/
├── Cargo.toml
├── Makefile
├── LICENSE
├── README.md
│
├── src/                          # Emulador Rust
│   ├── main.rs                   # CLI + frontend SDL
│   ├── bus.rs                    # MIU, mailbox, DVD, CF I/O
│   ├── memory.rs                 # Memory map
│   ├── hardware.rs               # Boot cycle-accurate
│   ├── interrupt.rs              # Controlador de interrupções
│   ├── dma.rs                    # DMA 4 canais
│   ├── save.rs                   # Save states
│   ├── disasm.rs                 # Disassembler PPC + ColdFire
│   ├── cdrom.rs                  # CD-ROM + ISO9660
│   ├── cd32_abi.rs               # Struct CD32Platform
│   ├── cpu/
│   │   ├── ppc603e.rs            # PPC603e + MMU
│   │   └── coldfire.rs           # ColdFire V4e
│   ├── gpu/tbdr.rs               # GPU Lisa II TBDR
│   └── audio/dsp.rs              # DSP áudio 8 canais
│
├── kernel/                       # Game runtime (libcd32.a)
│   ├── kernel.c                  # Entry point + game loader
│   ├── video.c                   # Framebuffer 640×480
│   ├── input.c                   # Joypad via mailbox
│   ├── audio.c                   # DSP 8 canais
│   ├── cdrom.c                   # ISO9660 + ELF loader
│   ├── dma.c                     # DMA helper
│   ├── string.c                  # memset/memcpy
│   ├── linker.ld                 # Linker script
│   ├── Makefile                  # Compila libcd32.a + demo
│   └── demo/demo.c               # Exemplo de jogo
│
├── include/
│   ├── cd32.h                    # API pública para jogos
│   └── cd32_platform.h           # Header C da ABI
│
├── boards/montelauro-cd32/       # BSP AROS (legacy)
├── docker/
│   ├── Dockerfile                # Imagem com toolchain PPC
│   └── entrypoint.sh
├── tools/
│   ├── gen_headers.rs            # Gerador headers ABI
│   ├── check_abi_conformance.rs  # Validador de offsets
│   ├── setup-aros.sh             # Integração AROS (legacy)
│   ├── build-aros.sh             # Build AROS (legacy)
│   └── mkcd.sh                   # Mastering ISO9660
├── src/bin/gen_rom.rs            # Gerador de ROMs
├── docs/                          # Documentação
│   ├── hardware/
│   │   ├── memory_map.md
│   │   ├── boot_sequence.md
│   │   └── bios_dump_notes.md
│   └── aros/abi.md
└── rom/                          # ROMs geradas
```

## Boot Chain (Game Runtime)

```
Power On
  │
  ▼
ColdFire
  ├── Copia PPC bootstrap + kernel da ROM para SysRAM
  ├── Escreve struct CD32Platform em 0x0000_FC00
  ├── Handoff → STOP
  │
  ▼
PPC bootstrap
  ├── Spin no handoff
  ├── Stack pointer (r1 = 0x017F_0000)
  ├── Platform struct em r3
  └── Jump para kernel (0x0000_2000)
  │
  ▼
kernel.c:_start()
  ├── cd32_video_init() → framebuffer Lisa II
  ├── cd32_printf() → banner de boot
  ├── cd32_audio_init() → DSP 8 canais
  ├── cd32_cdrom_init() → monta ISO9660
  ├── cd32_cdrom_load("GAME.ELF") → ELF parser + DMA
  └── game_main() → o jogo
```

## Hardware Especulado

| Componente | Spec |
|---|---|
| CPU | PowerPC 603e @ 266MHz |
| Coprocessador | ColdFire V4e @ 140MHz |
| GPU | TBDR custom, 6M polys/s ("Lisa II") |
| RAM | 28MB unificada (0x00000000–0x01BFFFFF) |
| Áudio | DSP + ColdFire, 8 canais estéreo |
| Mídia | CD-ROM 12x (expansão DVD opcional) |
| SO | Runtime próprio + AROS (legacy) |

## Licença

O código original MonteLauro (emulador, runtime, ferramentas) é MIT.
Componentes derivados de AROS seguem a AROS Public License (APL).
