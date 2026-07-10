# CD3² Memory Map (v0.2 — comunidade reverse)

## Visão Geral

O CD³² usa um espaço de endereçamento de 32 bits, dividido entre três barramentos:
- **Barramento PPC** (domínio primário, 266MHz)
- **Barramento ColdFire** (domínio legado, 140MHz)
- **VRAM** (acessível via GPU, ~100MHz)

A arbitragem entre PPC e ColdFire é feita pelo **Memory Interface Unit (MIU)** , que traduz endereços e insere wait-states conforme o domínio de destino.

```
0x0000_0000 ┌─────────────────────────────────────────────┐
            │  System RAM (16MB)                          │  PPC domínio primário
            │  Acesso PPC: 0 wait-states                  │
            │  Acesso ColdFire: 2 wait-states (via MIU)   │
0x0100_0000 ├─────────────────────────────────────────────┤
            │  Chip RAM (4MB)                             │  Compartilhado PPC+ColdFire
            │  Contém: framebuffers, audio buffers,       │
            │  sprite lists, copper lists                  │
            │  Acesso PPC: 1 wait-state                    │
            │  Acesso ColdFire: 0 wait-states (prioritário)│
0x0140_0000 ├─────────────────────────────────────────────┤
            │  (reservado / expansão)                     │
0x0200_0000 ├─────────────────────────────────────────────┤
            │  ColdFire Local Memory (2MB)                │  Domínio ColdFire exclusivo
            │  Contém: Kickstart ROM shadow, I/O regs      │
            │  Boot ROM (512KB) mapeado aqui em reset      │
0x0220_0000 ├─────────────────────────────────────────────┤
            │  I/O Registers — ColdFire Peripherals        │
            │  0x0220_0000: UART (debug)                   │
            │  0x0220_0010: SPI (CDROM controle)           │
            │  0x0220_0020: GPIO / Joyports               │
            │  0x0220_0030: RTC                            │
0x0240_0000 ├─────────────────────────────────────────────┤
            │  (reservado)                                 │
0x0300_0000 ├─────────────────────────────────────────────┤
            │  CDROM Register Block                        │
            │  Mapeado em ambos barramentos                │
0x0400_0000 ├─────────────────────────────────────────────┤
            │  GPU Register File (64KB)                    │
            │  Inclui: command FIFO, tile accelerator,     │
            │  texture descriptor cache, pixel pipe ctrl   │
0x0401_0000 ├─────────────────────────────────────────────┤
            │  VRAM (8MB)                                  │
            │  Frame buffers (double ou triple)            │
            │  Depth buffers, texture atlases              │
            │  Apenas PPC + GPU acessam                    │
0x0480_0000 ├─────────────────────────────────────────────┤
            │  DVD Expansion Slot (opcional, mem-mapped)   │
0x0800_0000 ├─────────────────────────────────────────────┤
            │  (emuladores e homebrew I/O, mirror)         │
0xFF00_0000 ├─────────────────────────────────────────────┤
            │  Boot ROM / Kickstart (512KB, read-only)     │
            │  Mapeado em reset, depois pode ser ocultado  │
0xFF08_0000 └─────────────────────────────────────────────┘
```

## Detalhamento de Acessos

### MIU (Memory Interface Unit)

A MIU segura os seguintes registros visíveis ao PPC:

| Offset | Nome | Descrição |
|--------|------|-----------|
| 0x0000 | MIU_CFG | Config: endianness swap, cache coherency mode |
| 0x0004 | MIU_STAT | Status: barramento ocupado, erro de alinhamento |
| 0x0008 | MIU_ARB | Prioridade: default ColdFire > PPC em Chip RAM |
| 0x000C | MIU_TIMING | Wait-state override (debug) |

### DMA Channels

O CD³² possui 4 canais DMA:

1. **CDROM → RAM** (16 palavras por burst)
2. **RAM → GPU** (vertex/texture upload)
3. **RAM → Audio FIFO** (streaming)
4. **ColdFire ↔ RAM** (firmware bootstrap loader)

Todos os DMAs são gerenciados pela MIU e têm prioridade fixa (CDROM > GPU > Audio > ColdFire).

## Notas de Reverse Engineering

- O número ímpar de 20MB total (16+4) é um artefato de protótipo: a Chip RAM de 4MB era um barramento separado de 32 bits soldado na placa-mãe, enquanto a System RAM usava pentes EDO SIMM padrão.
- Protótipos conhecidos (rev A, rev B) têm furos de jumpers para configurar Chip RAM entre 2MB e 8MB.
- Há rumores de que a Escom planejava lançar uma versão com 32MB (24+8) caso o CD³² tivesse ido à produção.
- O Kickstart shadow na área 0x0200_0000 é escrito pelo ColdFire durante o boot e depois protegido contra gravação via bit na MIU.
