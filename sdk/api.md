# libcd32 — SDK do Amiga CD³²

API de baixo nível para desenvolvimento de software nativo PPC no CD³².
O acesso ao hardware é feito via memory-mapped I/O nos barramentos PPC e ColdFire.

## Mapa de I/O (visão do programador PPC)

| Endereço | Região | Descrição |
|----------|--------|-----------|
| 0x0000_0000 | Unified RAM (24MB) | Memória principal, zero wait-state (PPC) |
| 0x0100_0000 | Mailbox (overlay) | 16 bytes MMIO PPC↔ColdFire |
| 0x0200_0000 | ColdFire Local RAM (2MB) | Escrita apenas pelo ColdFire |
| 0x0220_0000 | ColdFire I/O | UART, SPI, GPIO (joypad), RTC |
| 0x0300_0000 | CDROM Regs | Controle do CD-ROM |
| 0x03D0_0000 | Audio DSP | 64 registers de controle de áudio |
| 0x03E0_0000 | DMA Controller | 4 canais, 16 registers |
| 0x0400_0000 | GPU Regs (64KB) | Controle do renderizador TBDR |
| 0x0401_0000 | VRAM (8MB) | Framebuffers, depths, texturas |
| 0x0500_0000 | MIU Regs | Memory Interface Unit |

---

## 1. Vídeo — GPU "Lisa II"

A GPU é um tile-based deferred renderer. O pipeline:

1. Programa lista de primitivas na Chip RAM
2. Escreve endereço da lista no registrador GPU
3. Kick do render (reg 0x00 bit 0)
4. GPU processa tiles, escreve framebuffer na VRAM
5. VBlank gera interrupção (IRQ level 1, vetor 0x64)

### Registros GPU (base 0x0400_0000)

| Offset | Nome | Descrição |
|--------|------|-----------|
| 0x00 | GPU_CTRL | Kick bit (bit 0: iniciar render) |
| 0x04 | GPU_LIST_ADDR | Endereço da primitive list na Chip RAM |
| 0x08 | GPU_STATUS | 0=idle, 1=rendering, 2=vblank |
| 0x10 | GPU_FRAME | Contador de frames (incrementa a cada VBlank) |
| 0x20 | GPU_IRQ | Status de interrupção (bit 0: VBlank occorreu) |

### Exemplo: Inicializar vídeo

```c
// Pseudo-código C para PPC
#define GPU_BASE     0x04000000
#define GPU_CTRL     (*(volatile uint32_t*)(GPU_BASE + 0x00))
#define GPU_LIST_ADDR (*(volatile uint32_t*)(GPU_BASE + 0x04))
#define GPU_FRAME    (*(volatile uint32_t*)(GPU_BASE + 0x10))
#define VRAM_BASE    0x04010000

void video_init() {
    // Framebuffer começa no início da VRAM
    GPU_LIST_ADDR = VRAM_BASE;

    // Kick primeiro render
    GPU_CTRL = 1;
}

void wait_vblank() {
    uint32_t frame = GPU_FRAME;
    while (GPU_FRAME == frame) {}  // spin até próximo VBlank
}
```

### Formato da Primitive List

Cada primitiva é um bloco de 16 bytes na Chip RAM:

```
Offset 0x00: type (uint32) — 0=triângulo, 1=sprite, 2=retângulo
Offset 0x04: vertex_count (uint32)
Offset 0x08: data_offset (uint32) — offset para array de vértices
Offset 0x0C: color (uint32) — ARGB 8:8:8:8
```

---

## 2. Áudio — DSP + ColdFire

8 canais estéreo, 16-bit, 44.1kHz. O áudio é processado pelo ColdFire
em conjunto com um DSP interno. Cada canal tem um FIFO de 2KB.

### Registros DSP (base 0x03D0_0000)

Acessados como words (32-bit) alinhados.

| Offset | Nome | Descrição |
|--------|------|-----------|
| 0x00 | DSP_CTRL | Channel enable bits (bit 0-7) |
| 0x04 | DSP_MASTER_VOL | Volume master (0-1024) |
| 0x08 | DSP_OUT_L | Sample esquerdo atual (read-only) |
| 0x0C | DSP_OUT_R | Sample direito atual (read-only) |

Canais individuais são configurados via DMA — o programa escreve
buffers de áudio na Chip RAM e agenda transferências DMA para o FIFO.

### Exemplo: Tocar tom

```c
#define DSP_BASE    0x03D00000
#define DSP_CTRL    (*(volatile uint32_t*)(DSP_BASE + 0x00))

void audio_init() {
    DSP_CTRL = 0xFF;  // Habilita todos os 8 canais
}

void audio_tone(int channel, int16_t sample) {
    // Push sample diretamente via DMA ou mailbox
    // (implementação simplificada — vide DMA)
}
```

---

## 3. Input — Joypad via ColdFire GPIO

O estado do joypad é lido do ColdFire GPIO no offset 0x20.
O ColdFire atualiza este registro conforme a entrada do jogador.

### Mapa de Bits do Joypad

| Bit | Botão |
|-----|-------|
| 0 | UP |
| 1 | DOWN |
| 2 | LEFT |
| 3 | RIGHT |
| 4 | A |
| 5 | B |
| 6 | START |
| 7 | SELECT |

### Exemplo: Ler joypad

```c
#define CF_GPIO     0x02200020  // GPIO register

// Lê o GPIO — a leitura passa pelo ColdFire, não por mailbox
uint16_t read_joypad() {
    return *(volatile uint16_t*)CF_GPIO;
}

int is_pressed(uint16_t joy, int bit) {
    return (joy & (1 << bit)) == 0;  // bits ativos em low
}
```

### Mailbox PPC↔ColdFire

A mailbox permite que o PPC envie comandos ao ColdFire.
São 16 bytes em overlay MMIO em 0x0100_0000 (dentro da janela da RAM unificada).

| Offset | Nome | Descrição |
|--------|------|-----------|
| 0x00 | MAILBOX_CMD | Comando do PPC para ColdFire |
| 0x04 | MAILBOX_RESP | Resposta do ColdFire ao PPC |
| 0x08 | MAILBOX_STATUS | 0=pronto, 1=pendente, 2=lido |
| 0x0C | MAILBOX_ARG | Argumento do comando |

Comandos:

| Cmd | Nome | Descrição |
|-----|------|-----------|
| 0x01 | CF_EXEC | Executa rotina 68k na ColdFire local RAM |
| 0x02 | CF_IO | Acessa registro de I/O do ColdFire |
| 0x03 | CF_DMA | Solicita transferência DMA |
| 0xFF | CF_HALT | Desliga ColdFire |

---

## 4. DMA Controller

4 canais com prioridade fixa: CDROM > GPU > Audio > ColdFire.

### Registros DMA (base 0x03E0_0000)

Cada canal ocupa 16 bytes:

| Offset (canal) | Registro |
|----------------|----------|
| +0x00 | DMA_SRC — endereço fonte |
| +0x04 | DMA_DST — endereço destino |
| +0x08 | DMA_SIZE — tamanho em bytes |
| +0x0C | DMA_CTRL — controle (bit 0=start, bit 1=reset) |

### Exemplo: DMA de CDROM para RAM

```c
#define DMA_BASE    0x03E00000

void dma_copy(uint32_t src, uint32_t dst, uint32_t size) {
    volatile uint32_t *chan = (volatile uint32_t*)DMA_BASE;
    chan[0] = src;   // DMA_SRC
    chan[1] = dst;   // DMA_DST
    chan[2] = size;  // DMA_SIZE
    chan[3] = 1;     // DMA_CTRL: start
    while (chan[3] & 1) {}  // wait for completion
}
```

---

## 5. CD-ROM

Controlador mapeado em 0x0300_0000. Suporta ISO9660.

### Registros CDROM

| Offset | Nome | Descrição |
|--------|------|-----------|
| 0x00 | CD_STAT | Status: bit 0=disc present |
| 0x04 | CD_FLAGS | bit 1=ready, bit 2=data ready, bit 3=error |
| 0x08 | CD_LBA | Último LBA lido |
| 0x0C | CD_REMAIN | Setores restantes |
| 0x10 | CD_CMD | Comando (escrever para executar) |
| 0x14 | CD_ARG0 | LBA para leitura |
| 0x18 | CD_ARG1 | Número de setores |

Comandos:

| Cmd | Descrição |
|-----|-----------|
| 0x01 | Read sectors (LBA em ARG0, count em ARG1) |
| 0x02 | Pause |
| 0x03 | Resume |
| 0x10 | Mount ISO9660 |
| 0x11 | List root directory → data_buffer |

---

## 6. Memória

| Região | Endereço | Tamanho |
|--------|----------|---------|
| Unified RAM | 0x0000_0000 | 24 MB |
| Boot ROM | 0xFF00_0000 | 512 KB |
| VRAM | 0x0401_0000 | 8 MB |

### Layout de Boot

1. Reset: ColdFire executa de 0xFF00_0000
2. ColdFire copia microkernel PPC da ROM para 0x0000_0100
3. ColdFire libera reset do PPC
4. PPC começa em 0x0000_0100
5. PPC inicializa MMU (mapeamento flat identity), GPU, interrupções
6. PPC lê CD-ROM ou entra no Kickstart Desktop

---

## 7. Constantes Úteis

```c
// Clock
#define PPC_CLOCK_HZ    266000000
#define CF_CLOCK_HZ     140000000

// Interrupt vectors (PPC)
#define PPC_EXT_INT_VEC     0x0500

// Interrupt vectors (ColdFire)
#define CF_IRQ1_VEC     0x64   // GPU VBlank
#define CF_IRQ2_VEC     0x68   // CDROM data
#define CF_IRQ3_VEC     0x6C   // Timer
#define CF_IRQ4_VEC     0x70   // DMA done
#define CF_IRQ6_VEC     0x78   // GPU VBlank (alt)

// Protection (MMU)
#define MSR_EE          (1 << 15)  // External interrupt enable
#define MSR_IR          (1 << 8)   // Instruction translation
#define MSR_DR          (1 << 9)   // Data translation
```
