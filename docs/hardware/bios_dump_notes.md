# BIOS Dump Notes — CD3² Kickstart ROM

## Origem

O dump da ROM do CD³² (apelidado "Kickstart 4.0" pela comunidade) vazou em 2019 de um ex-funcionário da Escom que manteve placas de protótipo. O dump tem 512KB (0x80000 bytes) e corresponde exatamente ao tamanho do Boot ROM mapeado em 0xFF00_0000.

**Hash SHA-256:** `3b6a8c9f1e2d4a7b5c8f0e3d6a9b2c5f8e1a4d7b0c3f6e9a2d5b8c1f4e7a0d3`

## Estrutura Interna (512KB)

| Offset | Tamanho | Conteúdo |
|--------|---------|----------|
| 0x00000 | 2KB | ColdFire reset vectors + early init |
| 0x00800 | 60KB | ColdFire "Kickstart Compatibility Layer" |
| 0x10000 | 64KB | PPC microkernel (copiado pra RAM no boot) |
| 0x20000 | 128KB | AmigaOS PPC kernel (comprimido LZSS) |
| 0x40000 | 64KB | GPU firmware / microcode |
| 0x50000 | 64KB | Fontes, bitmaps, default palette |
| 0x60000 | 32KB | Hardware abstraction layer (HAL) |
| 0x68000 | 96KB | Dispositivos / drivers (CDROM, joy, audio) |

## Strings Notáveis Extraídas

```
CD32-Kickstart v4.0 (1998-11-23)
Copyright (C) 1998 Amiga Technologies GmbH / Escom AG
Based on AmigaOS 3.1 (C) 1994 Commodore-Amiga Inc.

ColdFire Boot Agent v0.7
PPC Microkernel v1.2 (Build 98-Nov-18)
GPU Microcode Rev 2.1 "Lisa-II"

"Only Amiga Makes It Possible" -> "Only CD³² Makes It Real" (texto alternado)
```

## Checksums

Há três checksums de 32-bit embutidos na ROM:

1. **CRC32** da região ColdFire (0x0000–0x0FFFF) em 0x0FFFC
2. **CRC32** da região PPC (0x10000–0x1FFFF) em 0x1FFFC
3. **Adler-32** de toda ROM em 0x7FFF8

Os protótipos conhecidos (rev A e rev B) têm checksums diferentes — indicando revisões de firmware.

## Áreas Não Documentadas

- **0x18000–0x18FFF**: Bloco de 4KB que parece conter uma "assinatura" RSA de 2048 bits. Possivelmente um mecanismo de verified boot que nunca foi ativado.
- **0x70000–0x71FFF**: Tabela de 8KB com ponteiros que não referenciam nenhum endereço válido no dump. Especula-se que seja um remapeamento pra um chip NVRAM externo que não estava presente na placa de protótipo.
- **0x7FFFE**: Dois bytes `0xFE 0xED` (comentário nos source leaks: "FEED the cat" — easter egg do engenheiro de firmware).

## Mídia de Boot

O setor de boot do CD-ROM segue o padrão **AmigaCD Boot Block** (compatível com o CD³² loader):

```
Offset 0x000: 'CD32' magic (4 bytes)
Offset 0x004: checksum complement
Offset 0x008: loader size in sectors
Offset 0x00C: entry point (offset em bytes do loader)
Offset 0x010: flags (bit 0: PPC native, bit 1: CF compat required)
Offset 0x014: reserved
Offset 0x020: loader code (até 48KB)
```

O CD³² introduz o flag `bit 0 = PPC native` — se setado, o ColdFire não precisa estar ativo durante a execução do jogo. Isso permite que o jogo rode 100% em PPC e use toda a RAM de 20MB sem contenção de barramento.

*(Documentação comunitária — dados extraídos de dumps de protótipos rev B serial #007 e #011)*
