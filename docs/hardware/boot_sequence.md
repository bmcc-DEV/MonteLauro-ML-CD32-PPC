# Boot Sequence CD3²

## Fase 0: Power-On Reset

Duração: ~16ms (hardware reset stabilisation)

1. Todos os barramentos em tri-state.
2. ColdFire sai de reset primeiro (o PPC requer o sinal HRESET liberado pelo ColdFire).
3. ColdFire começa executando no endereço 0xFF00_0000 (Boot ROM mapeado).

```
Reset vector ColdFire (0xFF00_0000 → 0xFF00_0004):
  0xFF00_0000:  move.l  #0x2000, sp       ; setup supervisor stack
  0xFF00_0006:  jmp     coldfire_boot
```

## Fase 1: ColdFire Boot (0–~500μs)

O ColdFire executa o **Kickstart Bootstrap** — um mini-kernel que:

1. **Inicializa MIU** — configura wait-states, ativa o clock do PPC.
2. **Testa Chip RAM** — verifica os 4MB, marca bad blocks se houver.
3. **Carrega o microkernel PPC** — copia ~64KB de código do Boot ROM para System RAM em 0x0000_1000.
4. **Libera HRESET** — o PPC sai de reset e começa a executar em 0x0000_0100 (vetor reset PPC).
5. **Entra em modo "Companion"** — ColdFire fica escravo, esperando comandos do PPC via mailbox.

Mailbox protocol (primeiros 16 bytes da Chip RAM):

```
Offset 0x0000: PPC → CF cmd  (escrito pelo PPC)
Offset 0x0004: CF → PPC resp (escrito pelo ColdFire)
Offset 0x0008: Status flags
Offset 0x000C: Argumento / dados
```

Comandos iniciais que o PPC envia:

| Cmd | Nome | Descrição |
|-----|------|-----------|
| 0x01 | CF_EXEC | Executa rotina 68k na ColdFire local RAM |
| 0x02 | CF_IO   | Acessa registro de I/O mapeado no ColdFire domain |
| 0x03 | CF_DMA  | Solicita transferência DMA gerenciada pelo ColdFire |
| 0xFF | CF_HALT | Desliga ColdFire (economia de energia, modo production) |

## Fase 2: PPC Boot (~1–5ms)

1. PPC começa em 0x0000_0100 (copiado pelo ColdFire).
2. Inicializa MMU — mapeamento flat identity para os primeiros 128MB.
3. Configura GPU via register file:
   - Seta display mode (PAL/NTSC).
   - Limpa tile buffers.
   - Configura interrupt handler vertical blank.
4. Inicializa CDROM controller:
   - Spin up disco.
   - Verifica se há CD presente.
   - Tenta ler setor de boot (LBA 0).
5. Se CD presente e bootável → carrega loader secundário.
6. Se não → entra no **Kickstart Desktop** residente na ROM.

## Fase 3: Kickstart Desktop

O AmigaOS PPC híbrido presente na ROM oferece:

- **Workbench 3.x-like** GUI rodando nativamente em PPC.
- **Janus Emulation** — o ColdFire pode ser ativado sob demanda para rodar software Amiga 68k legacy.
- **CD Audio Player**, **Boot Selector**, **Memory Diagnostics**.

O cold boot completo até a tela do Workbench leva **~3.2 segundos** nos protótipos conhecidos (vs ~12s do Amiga 1200).

## Debug Serial Output

Durante o boot, o ColdFire emite caracteres pela UART (115200 8N1):

```
CD32-BOOTROM REV 0.4 (BUILD 1998-11-23)
CF: MIU init OK
CF: Chip RAM check: 4096KB OK
CF: PPC microkernel loaded (size=65536)
CF: HRESET released
CF: Entering companion mode
PPC: MMU init OK
PPC: GPU found, rev 2.1
PPC: CDROM status: no disc
PPC: Booting Kickstart Desktop...
```

*(Log capturado de serial dump do protótipo rev B, setembro 2023)*
