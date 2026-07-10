# MonteLauro CD³²

**MonteLauro CD³²** — Plataforma aberta baseada no console "phantom" Amiga CD³².

Este repositório contém o emulador, firmware bootstrap e SDK para a plataforma MonteLauro CD³².
O sistema usa AROS como firmware padrão (open source, APL license), eliminando a dependência
de BIOS proprietária.

## Status

Projeto completo de engenharia reversa e emulação do hardware especulado. Compila e executa, aguardando BIOS dump real para validação.

### Funcionalidades Implementadas

- [x] Documentação de hardware (memory map, boot sequence, BIOS structure)
- [x] Núcleo PPC603e — interpretador com ~40 instruções + MMU (SR, BAT, page walk)
- [x] Núcleo ColdFire V4e — interpretador com ~50 instruções (subset 68k)
- [x] Barramento MIU com arbitragem PPC↔ColdFire e mailbox
- [x] Mapa de memória: 16MB SysRAM + 4MB ChipRAM + 2MB CF local + 8MB VRAM + 512KB ROM
- [x] GPU TBDR "Lisa II" — tile-based deferred rendering (32×32 tiles, 640×480)
- [x] Áudio DSP + ColdFire — 8 canais estéreo com FIFO por canal
- [x] CD-ROM 12x controller com DMA e ISO9660 parser
- [x] Interrupt controller — 8 níveis, roteamento PPC/ColdFire
- [x] DMA — 4 canais (CDROM > GPU > Audio > ColdFire), bursts de 64 bytes
- [x] Input — joypad via GPIO ColdFire, mapeamento SDL
- [x] Trace de instruções com disassembler PPC + ColdFire
- [x] Timing cycle-accurate — interleave PPC:CF na proporção 266:140
- [x] Frontend SDL opcional — janela 640×480 com framebuffer da VRAM
- [x] Save states — serialização completa do estado
- [x] ROM sintética "Hello CD³²" — exercita CPU, GPU, DSP, DMA
- [x] DVD expansion slot stub

### Pendentes

- [ ] BIOS dump real (Kickstart 4.0) — único item que realmente falta pra validar o boot completo

## Compilando

```bash
cargo build --release
```

Para frontend gráfico (requer SDL2 dev libs):

```bash
cargo build --release --features sdl-frontend
```

## Executando

```bash
# Boot completo (sem BIOS real — usa ROM vazia)
cargo run --release

# Com dump de BIOS
cargo run --release -- --bios kickstart.rom

# Com imagem de CD
cargo run --release -- --bios kickstart.rom --disc game.iso

# Com janela gráfica e input (setas/Z/X/Enter/Shift)
cargo run --release --features sdl-frontend -- --sdl

# Trace de instruções com disassembler
cargo run --release -- --cycles 1000 --trace

# Save / Load state
cargo run --release -- --cycles 100000 --save-state estado.sav
cargo run --release -- --load-state estado.sav
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

## Estrutura

```
src/
├── main.rs          CLI + frontend SDL com input
├── lib.rs
├── bus.rs           MIU, arbitragem, mailbox, DVD, CF I/O
├── memory.rs        Memory map (20MB RAM + VRAM + ROM)
├── hardware.rs      Orquestração de boot, cycle-accurate interleave
├── interrupt.rs     Controlador de interrupções (8 níveis)
├── dma.rs           Controlador DMA (4 canais)
├── save.rs          Save states
├── disasm.rs        Disassembler PPC + ColdFire
├── cdrom.rs         CD-ROM 12x + ISO9660 parser
├── cpu/
│   ├── ppc603e.rs   PowerPC 603e + MMU (SR, BAT, page walk)
│   └── coldfire.rs  ColdFire V4e (~50 instruções)
├── gpu/
│   └── tbdr.rs      GPU "Lisa II" TBDR
└── audio/
    └── dsp.rs       DSP + ColdFire áudio (8 canais)

docs/hardware/
├── memory_map.md    Layout de endereçamento completo
├── boot_sequence.md Fases de boot: ColdFire -> PPC
└── bios_dump_notes.md  Estrutura do Kickstart 4.0

sdk/
└── api.md           Documentação da API libcd32

src/bin/
└── gen_rom.rs       Gerador da ROM sintética "Hello CD³²"
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
| SO | AmigaOS PPC híbrido / AROS |

## Makefile

```bash
make build              # Compila o emulador
make rom-hello          # Gera ROM "Hello CD3²" (validação)
make rom-aros           # Gera ROM com bootstrap AROS
make headers            # Gera headers C/Rust da ABI
make test-hello         # Testa ROM hello
make test-aros          # Testa bootstrap AROS
make trace-hello        # Trace detalhado da ROM hello
make trace-aros         # Trace detalhado do bootstrap AROS
make sdl-hello          # Frontend gráfico com ROM hello
make sdl-aros           # Frontend gráfico com bootstrap AROS
make save ROM=rom/aros_cd32.rom  # Salva estado
```

## Licença

O código original MonteLauro (emulador, bootstrap, tooling, SDK) é distribuído
sob licença MIT. Componentes derivados de AROS seguem a AROS Public License (APL).
Consulte LICENSE, LICENSE.APL e LICENSE.GPL/LGPL para detalhes.

Repositório: [github.com/bmcc-DEV/MonteLauro-ML-CD32-PPC](https://github.com/bmcc-DEV/MonteLauro-ML-CD32-PPC)
