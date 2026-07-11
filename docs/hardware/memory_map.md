# CD3² Memory Map (v0.3 — RAM unificada 24MB)

## Visão Geral

O CD³² usa um espaço de endereçamento de 32 bits. A partir da rev C da
plataforma CDG2, **System RAM e Chip RAM foram unificadas** num único
banco de **24MB** acessível por PPC e ColdFire (via MIU).

- **Barramento PPC** (domínio primário, 266MHz)
- **Barramento ColdFire** (domínio legado, 140MHz)
- **VRAM** (acessível via GPU/bus, 8MB)

A arbitragem entre PPC e ColdFire é feita pelo **Memory Interface Unit (MIU)**.

```
0x0000_0000 ┌─────────────────────────────────────────────┐
            │  Unified RAM (24MB)                         │  PPC + ColdFire
            │  Acesso PPC: 0 wait-states                  │
            │  Acesso ColdFire: 1 wait-state (via MIU)    │
            │  Contém: kernel, apps, audio buffers,       │
            │  primitive lists, stack (topo − 64KB)       │
0x0100_0000 ├─ overlay ───────────────────────────────────┤
            │  Mailbox PPC↔ColdFire (16 bytes MMIO)       │  overlay na RAM
0x0100_0010 │  … continua Unified RAM …                   │
0x017F_FFFF └─────────────────────────────────────────────┘
0x0200_0000 ┌─────────────────────────────────────────────┐
            │  ColdFire Local Memory (2MB)                │  Domínio CF
            │  Kickstart shadow / I/O scratch             │
0x0220_0000 ├─────────────────────────────────────────────┤
            │  I/O Registers — ColdFire Peripherals        │
            │  0x0220_0000: UART (debug)                   │
            │  0x0220_0010: SPI (CDROM controle)           │
            │  0x0220_0020: GPIO / Joyports (active-low)   │
            │  0x0220_0030: RTC                            │
0x0300_0000 ├─────────────────────────────────────────────┤
            │  CDROM Register Block                        │
0x03D0_0000 ├─────────────────────────────────────────────┤
            │  Audio DSP                                   │
0x03E0_0000 ├─────────────────────────────────────────────┤
            │  DMA Controller (4 canais)                   │
0x0400_0000 ├─────────────────────────────────────────────┤
            │  GPU Register File (64KB)                    │
0x0401_0000 ├─────────────────────────────────────────────┤
            │  VRAM (8MB) — framebuffer único (guest=SDL)  │
0x0481_0000 ├─────────────────────────────────────────────┤
            │  DVD Expansion Slot (opcional)               │
0x0500_0000 ├─────────────────────────────────────────────┤
            │  MIU Regs                                    │
0xFF00_0000 ├─────────────────────────────────────────────┤
            │  Boot ROM / Kickstart (512KB, read-only)     │
0xFF08_0000 └─────────────────────────────────────────────┘
```

## Layout de software na Unified RAM

| Região | Endereço | Uso |
|--------|----------|-----|
| Handoff | 0x0000_0000 | assinatura ColdFire→PPC |
| PPC bootstrap | 0x0000_0100 | copiado da ROM |
| Kernel / jogo | 0x0000_2000+ | runtime + ELF |
| Platform struct | 0x0000_FC00 | `CD32Platform` |
| Mailbox | 0x0100_0000 | 16 bytes MMIO |
| Stack (default) | 0x017F_0000 | r1 no boot |

## MIU (Memory Interface Unit)

| Offset | Nome | Descrição |
|--------|------|-----------|
| 0x0000 | MIU_CFG | Config: endianness swap, cache coherency mode |
| 0x0004 | MIU_STAT | Status: barramento ocupado, erro de alinhamento |
| 0x0008 | MIU_ARB | Prioridade de arbitragem |
| 0x000C | MIU_TIMING | Wait-state override (debug) |

## DMA Channels

1. **CDROM → RAM**
2. **RAM → GPU**
3. **RAM → Audio FIFO**
4. **ColdFire ↔ RAM**

Prioridade fixa: CDROM > GPU > Audio > ColdFire.

## Notas

- **v0.2** usava 16MB SysRAM + 4MB Chip RAM (20MB). Unificação em 24MB remove a
  contenção artificial e simplifica o memory map para homebrew.
- Campos ABI `chip_ram_*` / `sys_ram_*` permanecem por compatibilidade e
  apontam ambos para a RAM unificada (base 0, size 24MB).
- A VRAM é um buffer único: o guest escreve em `0x0401_0000` e o frontend SDL
  lê o mesmo conteúdo (sem cópia GPU fantasma).
