# MonteLauro CD+G² Memory Map (v0.4 — RAM unificada 28MB)

## Visão Geral

O MonteLauro CD+G² (ML GD²) usa um espaço de endereçamento de 32 bits. 
A partir da v0.4, **System RAM e Chip RAM foram unificadas** num único 
banco de **28MB** acessível por PPC e ColdFire (via MIU).

- **Barramento PPC** (domínio primário, 266MHz)
- **Barramento ColdFire** (domínio legado, 140MHz)

## Map

| Endereço | Região | Tamanho | Acesso |
|----------|--------|---------|--------|
| 0x0000_0000 | Unified RAM | 28 MB | PPC + CF |
| 0x0100_0000 | Mailbox (overlay) | 16 B | PPC + CF |
| 0x0200_0000 | ColdFire Local | 2 MB | CF |
| 0x0220_0000 | ColdFire I/O | — | CF |
| 0x0300_0000 | CDROM | — | PPC + CF |
| 0x03D0_0000 | Audio DSP | — | PPC |
| 0x03E0_0000 | DMA | — | PPC + CF |
| 0x0400_0000 | GPU Regs | 64 KB | PPC |
| 0x0500_0000 | MIU | — | PPC |
| 0xFF00_0000 | Boot ROM | 512 KB | CF |

## Organização da Unified RAM

| Intervalo | Finalidade |
|-----------|------------|
| 0x0000_0000 – 0x01AFFFFF | RAM geral (~26 MB) |
| 0x01B0_0000 – 0x01BFFFFF | Framebuffer / Texturas (1 MB) |
| 0x01C0_0000 – 0x01FFFFFF | Reservado (~3 MB) |

## Memory-Mapped I/O (visão PPC)

| Base | Tamanho | Descrição |
|------|---------|-----------|
| 0x0100_0000 | 16 B | Mailbox PPC↔ColdFire |
| 0x0220_0020 | 64 B | GPIO (joypad, etc) |
| 0x0300_0000 | 1 MB | CD-ROM controller |
| 0x03D0_0000 | 256 B | Audio DSP registers |
| 0x03E0_0000 | 64 B | DMA controller (4 canais × 16B) |
| 0x0400_0000 | 64 KB | GPU register file |
| 0x0500_0000 | 16 B | Memory Interface Unit |

## Notas

- O framebuffer começa em 0x01B0_0000 por padrão (`CD32_VRAM_BASE`)
- Stack pointer default: 0x01BF_0000 (64KB abaixo do topo da RAM)
- Boot ROM é endereçável apenas pelo ColdFire no reset; PPC acessa após MMU init
