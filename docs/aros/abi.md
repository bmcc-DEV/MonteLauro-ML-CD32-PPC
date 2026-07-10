# AROS PPC ABI para CD³² / MontêLauro CD3²

## Visão Geral

O bootstrap do CD³² carrega o kernel AROS PPC e passa controle com
os registradores e estado de máquina padronizados abaixo. Este documento
rege a interface entre o **CD³² Boot Wrapper** e o **AROS kernel PPC**.

## Estado da Máquina na Entrada do Kernel

### MMU (PowerPC 603e)

| Parâmetro | Valor |
|-----------|-------|
| MSR[IR] | 0 (translation desligada) |
| MSR[DR] | 0 (translation desligada) |
| BATs | 4 × IBAT + 4 × DBAT identity mapping |
| BL (Block Length) | 256MB cada BAT (0x00000000–0x0FFFFFFF) |
| WIMG | 0 (write-through off, cache off) |

A MMU está configurada mas desligada. O kernel AROS deve ligar
MSR[IR] e MSR[DR] quando sua própria tabela de páginas estiver pronta.
Até lá, o acesso é flat (EA = PA).

### Interrupções

| Parâmetro | Valor |
|-----------|-------|
| MSR[EE] | 0 (desligadas) |
| MSR[CE] | 0 |
| MSR[ME] | 1 |
| Vetor externo | 0x0000_0500 |
| Vetor decrementer | 0x0000_0900 |
| VPA/SMI | não usado |

### Cache

| Parâmetro | Valor |
|-----------|-------|
| HID0[DCE] | 0 (data cache off) |
| HID0[ICE] | 0 (instruction cache off) |
| HID0[BTIC] | 1 (branch target instruction cache on) |

O kernel deve ligar caches quando ready.

### Stack Pointer

| Registrador | Valor |
|-------------|-------|
| r1 | Topo da System RAM — 64KB (0x00FF_0000) |
| r2 | TOC pointer (reservado para AROS, 0 se não usado) |

### Registradores de Parâmetro

| Reg | Nome | Descrição |
|-----|------|-----------|
| r3 | SysBase | Ponteiro para estrutura `struct CD32Platform` (ver abaixo) |
| r4 | CPUType | 0x0001_0001 (PPC603e, revision 1) |
| r5 | MemSize | Tamanho total de RAM em bytes (20MB = 0x013F_FFF8) |
| r6 | PlatformInfo | 0x0000_0002 (CD32/PPC v1) |
| r7 | ColdFireMailbox | Endereço da mailbox PPC↔ColdFire (0x0100_0000) |
| r8 | GPUBase | Endereço base da GPU (0x0400_0000) |
| r9 | VRAMBase | Endereço base da VRAM (0x0401_0000) |
| r10 | DSPBase | Endereço base do DSP de áudio (0x03D0_0000) |
| r11 | DMABase | Endereço base do controlador DMA (0x03E0_0000) |
| r12 | CDROMBase | Endereço base do CD-ROM (0x0300_0000) |

## Estrutura `struct CD32Platform`

Apontada por r3, alocada no início da Chip RAM (0x0100_0000).

```c
struct CD32Platform {
    uint32_t  magic;           // 0xCD32_0001
    uint32_t  total_ram;       // bytes (20MB)
    uint32_t  chip_ram_base;   // 0x0100_0000
    uint32_t  chip_ram_size;   // 4MB
    uint32_t  sys_ram_base;    // 0x0000_0000
    uint32_t  sys_ram_size;    // 16MB
    uint32_t  vram_base;       // 0x0401_0000
    uint32_t  vram_size;       // 8MB
    uint32_t  boot_rom_base;   // 0xFF00_0000
    uint32_t  boot_rom_size;   // 512KB
    uint32_t  cf_mailbox;      // 0x0100_0000
    uint32_t  gpu_base;        // 0x0400_0000
    uint32_t  dsp_base;        // 0x03D0_0000
    uint32_t  dma_base;        // 0x03E0_0000
    uint32_t  cdrom_base;      // 0x0300_0000
    uint32_t  gpio_base;       // 0x0220_0020
    uint32_t  coldfire_base;   // 0x0220_0000
    uint32_t  intc_base;       // MIU regs + soft IRQ controller
    uint32_t  tick_hz;         // frequencia do timer (0 = usar PPC dec)
    uint8_t   pad[512 - 5*16]; // padding para 512 bytes
};
```

## Serviços de Runtime via ColdFire Mailbox

O kernel AROS se comunica com o ColdFire através da mailbox em 0x0100_0000.

### Protocolo

```c
// PPC escreve comando, ColdFire processa, PPC lê resposta
// Spin-lock no status byte

void cf_send_cmd(uint32_t cmd, uint32_t arg) {
    while (*(volatile uint32_t*)MAILBOX_STATUS != 0); // aguarda pronto
    *(volatile uint32_t*)MAILBOX_ARG    = arg;
    *(volatile uint32_t*)MAILBOX_CMD    = cmd;
    *(volatile uint32_t*)MAILBOX_STATUS = 1;          // pending
}

uint32_t cf_read_resp() {
    while (*(volatile uint32_t*)MAILBOX_STATUS != 0); // aguarda done
    return *(volatile uint32_t*)MAILBOX_RESP;
}
```

### Tabela de Comandos

| Cmd | Nome | Descrição | Arg | Resp |
|-----|------|-----------|-----|------|
| 0x01 | CF_EXEC | Executa rotina 68k na ColdFire RAM | endereço | status |
| 0x02 | CF_IO_READ | Lê registro de I/O do ColdFire | offset (0x00-0x3F) | valor 16-bit |
| 0x03 | CF_IO_WRITE | Escreve registro de I/O do ColdFire | offset \| (val<<16) | status |
| 0x04 | CF_JOYPAD | Lê estado do joypad | 0 | bits (ver sdk/api.md) |
| 0x05 | CF_CDROM_STATUS | Status do CD-ROM | 0 | flags |
| 0x06 | CF_DMA_AUDIO | Transfere sample de áudio via DMA | (buf_hi<<16)\|buf_lo | status |
| 0x07 | CF_UART_WRITE | Escreve byte na UART de debug | byte | status |
| 0xFF | CF_HALT | Desliga ColdFire (modo economia) | 0 | 0 |

### Mapeamento de I/O do ColdFire (offset 0x00–0x3F)

| Offset | Periférico | Descrição |
|--------|------------|-----------|
| 0x00 | UART_DATA | Dado serial TX/RX |
| 0x02 | UART_STATUS | Bit 7=TX ready, bit 6=RX ready |
| 0x10 | SPI_DATA | CD-ROM controle (SPI-like) |
| 0x20 | GPIO | Joypad (bits 0–7, ver sdk/api.md) |
| 0x30 | RTC | Contador de tempo real |

## Mapa de Memória Final (visão do AROS)

```
0x0000_0000 ┌────────────────────┐
            │ System RAM (16MB)  │  AROS kernel + apps
0x0100_0000 ├────────────────────┤
            │ Chip RAM (4MB)     │  Mailbox, framebuffers,
            │                    │  audio DMA, primitive lists
0x0140_0000 └────────────────────┘
              ...
0x0220_0000 ┌────────────────────┐
            │ ColdFire I/O       │  GPIO, UART, SPI, RTC
0x0300_0000 ├────────────────────┤
            │ CD-ROM Regs        │
0x03D0_0000 ├────────────────────┤
            │ Audio DSP          │  Registers de 64 words
0x03E0_0000 ├────────────────────┤
            │ DMA Controller     │  4 canais
0x0400_0000 ├────────────────────┤
            │ GPU Regs (64KB)    │  Lisa II TBDR
0x0401_0000 ├────────────────────┤
            │ VRAM (8MB)         │  Framebuffer + texturas
0x0500_0000 ├────────────────────┤
            │ MIU Regs           │  Memory Interface Unit
0x0800_0000 ├────────────────────┤
            │ DVD Expansion      │  Slot opcional
0xFF00_0000 └────────────────────┘
            │ Boot ROM (512KB)   │  Read-only após boot
```

## Sequência de Boot Completa

```
Power On
  │
  ▼
ColdFire reset (PC = 0xFF00_0000)
  ├─ Auto-teste MIU, Chip RAM
  ├─ Inicializa DMA + Audio DSP
  ├─ Copia PPC microkernel + AROS kernel da ROM pra SysRAM
  ├─ Escreve struct CD32Platform na Chip RAM
  ├─ Escreve 0x0000_0001 no endereço 0 (handoff signature)
  └─ STOP (halt, aguarda IRQ)
  │
  ▼
PPC reset (PC = 0x0000_0100), após ler handoff signature
  ├─ Configura BAT identity mapping (256MB)
  ├─ Configura stack pointer (r1)
  ├─ Passa params nos registradores r3–r12
  └─ JMP para kernel AROS (0x0000_2000)
  │
  ▼
AROS kernel PPC (_start em 0x0000_2000)
  ├─ Lê struct CD32Platform de r3
  ├─ Inicializa exec.library, expansion.library
  ├─ Liga caches (HID0[DCE|ICE])
  ├─ Liga MMU (MSR[IR|DR])
  ├─ Liga interrupções externas (MSR[EE])
  └─ Inicia Workbench / ROM-Wedge
```

## Compilando o Bootstrap

O bootstrap é gerado pelo `gen_rom.rs` com a flag `--target aros-bootstrap`.

```bash
# Gerar ROM com AROS bootstrap + kernel placeholder
cargo run --bin gen_rom -- --target aros-bootstrap --output rom/aros_cd32.rom

# Com kernel AROS real (após compilar AROS para sam440-ppc)
cargo run --bin gen_rom -- --target aros-bootstrap \
    --kernel aros-ppc.bin --output rom/aros_cd32.rom
```
