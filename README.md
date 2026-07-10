# MonteLauro CD³²

**MonteLauro CD³²** — Plataforma aberta baseada no console "phantom" Amiga CD³².

Este repositório contém o emulador cycle-accurate, firmware bootstrap AROS e
SDK para desenvolvimento de software nativo na plataforma MonteLauro ML-CD32-PPC.
O sistema usa **AROS** como firmware padrão (open source, APL license),
eliminando a dependência de BIOS proprietária.

CI: `make ci` — pipeline completo local, sem dependência de serviços externos.

**Repositório:** [github.com/bmcc-DEV/MonteLauro-ML-CD32-PPC](https://github.com/bmcc-DEV/MonteLauro-ML-CD32-PPC)

---

## Funcionalidades

| Componente | Status |
|------------|--------|
| Emulador PPC603e + ColdFire V4e cycle-accurate | ✅ |
| MMU (SR, BAT identity, page table walk) | ✅ |
| GPU TBDR "Lisa II" (tile-based, 640x480) | ✅ |
| Áudio DSP 8 canais estéreo | ✅ |
| DMA 4 canais (CDROM > GPU > Audio > CF) | ✅ |
| CD-ROM 12x + ISO9660 parser | ✅ |
| Interrupt controller (8 níveis) | ✅ |
| Save states (serialização completa) | ✅ |
| Disassembler PPC + ColdFire integrado ao `--trace` | ✅ |
| Frontend SDL opcional | ✅ |
| **AROS bootstrap — boot chain validada** | ✅ |
| **ABI documentada — struct CD32Platform + mailbox protocol** | ✅ |
| **BSP AROS — HAL C (kernel_cpu, console, input, cdrom)** | ✅ |
| **Headers C/Rust automáticos da ABI** | ✅ |
| **ABI conformance checker** | ✅ |
| **CI/CD via GitHub Actions + Makefile** | ✅ |

### Pendente

- Port do AROS com BSP MonteLauro (`make aros-build`)

## Compilando

```bash
# Emulador
cargo build --release

# Com frontend SDL (requer SDL2 dev libs)
cargo build --release --features sdl-frontend
```

## Executando

```bash
# Boot ROM "Hello CD³²" (valida hardware)
make test-hello

# Boot AROS bootstrap
make test-aros

# Trace detalhado com disassembler
make trace-hello CYCLES=50000

# Frontend gráfico
make sdl-hello

# Save / Load state
make save
make load
```

### CLI

```
Usage: cd32-rs [OPTIONS]

Options:
  -b, --bios <BIOS>        Caminho para a Kickstart ROM (512KB)
  -d, --disc <DISC>        Imagem de CD (ISO9660 .bin/.iso)
  -c, --cycles <CYCLES>    Número de ciclos (0 = boot completo)
  -v, --verbose            Modo verbose
      --trace              Trace de instruções com disassembler
      --sdl                Frontend SDL (requer sdl-frontend)
      --save-state <PATH>  Salvar estado do emulador
      --load-state <PATH>  Carregar estado do emulador
  -h, --help               Print help
```

## ABI (Application Binary Interface)

A especificação canônica do hardware está em `docs/aros/abi.md`.
A struct `CD32Platform` é passada ao kernel AROS via registrador r3.

```bash
# Gerar headers C e Rust da ABI
make headers
# → include/cd32_platform.h
# → src/cd32_abi.rs

# Validar conformidade de offsets
cargo run --bin check-abi
```

## Makefile

```bash
make build                    # Compila emulador
make headers                  # Gera headers C/Rust da ABI
make check-abi                # Valida conformidade ABI
make rom-hello                # Gera ROM "Hello CD³²"
make rom-aros                 # Gera ROM bootstrap AROS
make test-hello               # Testa ROM hello
make test-aros                # Testa bootstrap AROS
make trace-hello CYCLES=50000 # Trace com disassembler
make trace-aros CYCLES=50000  # Trace do bootstrap AROS
make sdl-hello                # Frontend gráfico
make sdl-aros                 # Frontend com AROS
make save                     # Save state
make load                     # Load state
make stress                   # Stress test (500M ciclos)
make ci                       # Pipeline completo
make aros-setup AROS=/path    # Integra BSP na árvore AROS
make aros-build AROS=/path    # Compila AROS + gera ROM
make clean                    # Limpa artefatos
```

## Estrutura do Projeto

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
│   ├── memory.rs                 # Memory map (20MB RAM + VRAM + ROM)
│   ├── hardware.rs               # Boot cycle-accurate interleave
│   ├── interrupt.rs              # Controlador de interrupções
│   ├── dma.rs                    # DMA 4 canais
│   ├── save.rs                   # Save states
│   ├── disasm.rs                 # Disassembler PPC + ColdFire
│   ├── cdrom.rs                  # CD-ROM + ISO9660
│   ├── cd32_abi.rs               # Struct CD32Platform (gerado)
│   ├── cpu/
│   │   ├── ppc603e.rs            # PPC603e + MMU
│   │   └── coldfire.rs           # ColdFire V4e
│   ├── gpu/tbdr.rs               # GPU Lisa II TBDR
│   └── audio/dsp.rs              # DSP áudio 8 canais
│
├── boards/montelauro-cd32/       # BSP AROS (C)
│   ├── board.h                   # Definições canônicas
│   ├── kernel_cpu.c              # InitBoard, MMU, IRQ
│   ├── console.c                 # Framebuffer Lisa II
│   ├── input.c                   # Joypad via mailbox
│   ├── cdrom.c                   # CD-ROM via DMA
│   ├── Makefile                  # Compila libmlcd32.a
│   └── README.md
│
├── docs/
│   ├── hardware/                 # Documentação do hardware
│   │   ├── memory_map.md
│   │   ├── boot_sequence.md
│   │   └── bios_dump_notes.md
│   └── aros/abi.md               # ABI AROS PPC
│
├── include/cd32_platform.h       # Header C da ABI (gerado)
├── sdk/api.md                    # API libcd32
│
├── tools/
│   ├── gen_headers.rs            # Gerador headers C/Rust
│   ├── check_abi_conformance.rs  # Validador de offsets
│   └── setup-aros.sh             # Integração com AROS
│
├── src/bin/gen_rom.rs            # Gerador de ROM sintética
├── .github/workflows/ci.yml      # GitHub Actions
└── rom/                          # ROMs geradas
```

## Boot Chain AROS

```
Power On
  │
  ▼
ColdFire (0xFF00_0000)
  ├── Auto-teste MIU, Chip RAM
  ├── Copia PPC bootstrap + kernel AROS da ROM para SysRAM
  ├── Escreve struct CD32Platform na Chip RAM
  ├── Handoff signature → STOP
  │
  ▼
PPC (0x0000_0100)
  ├── Spin no handoff
  ├── BAT identity mapping (256MB)
  ├── Stack pointer, CD32Platform struct
  ├── Registradores r3-r12 conforme ABI
  └── Jump para kernel AROS (0x0000_2000)
  │
  ▼
AROS kernel
  ├── InitBoard() → lê CD32Platform → HW init
  ├── Console: banner "MonteLauro CD3² v1.0"
  ├── Input: joypad via ColdFire mailbox
  └── CD-ROM: DMA channel 0 + ISO9660
```

## Hardware Especulado

| Componente | Spec |
|---|---|
| CPU | PowerPC 603e @ 266MHz |
| Coprocessador | ColdFire V4e @ 140MHz |
| GPU | TBDR custom, 6M polys/s ("Lisa II") |
| RAM | 20MB unificada (16MB + 4MB Chip RAM) |
| VRAM | 8MB (framebuffers, depth, texturas) |
| Áudio | DSP + ColdFire, 8 canais estéreo |
| Mídia | CD-ROM 12x (expansão DVD opcional) |
| SO | AROS |

## Licença

O código original MonteLauro (emulador, bootstrap, tooling, SDK) é distribuído
sob licença MIT. Componentes derivados de AROS seguem a AROS Public License (APL).
Consulte `LICENSE`, `LICENSE.APL` e `LICENSE.GPL/LGPL` para detalhes.
