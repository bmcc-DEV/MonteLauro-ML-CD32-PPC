/*
 * MonteLauro ML-CD32 BSP — kernel_cpu.c
 * Inicialização da CPU PPC603e: MMU, BATs, IRQ controller, timer.
 *
 * Chamado pelo kernel AROS durante startup (kernel_init).
 * Recebe struct MLCD32Platform via r3 (conforme abi.md).
 */

#include <exec/types.h>
#include <exec/execbase.h>
#include <hardware/intbits.h>

#include "board.h"

/* Ponteiro global para a plataforma — setado em InitBoard */
const MLCD32Platform *ml_platform = NULL;

/* ── Utilitários PPC (inline assembly) ──────────────────────────── */

static inline void mtmsr(uint32_t val) {
    __asm__ __volatile__ ("mtmsr %0" : : "r"(val));
}

static inline uint32_t mfmsr(void) {
    uint32_t val;
    __asm__ __volatile__ ("mfmsr %0" : "=r"(val));
    return val;
}

static inline void mtspr(int spr, uint32_t val) {
    switch (spr) {
    case 528: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(528), "r"(val)); break;
    case 529: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(529), "r"(val)); break;
    case 530: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(530), "r"(val)); break;
    case 531: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(531), "r"(val)); break;
    case 536: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(536), "r"(val)); break;
    case 537: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(537), "r"(val)); break;
    case 538: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(538), "r"(val)); break;
    case 539: __asm__ __volatile__ ("mtspr %0,%1" : : "K"(539), "r"(val)); break;
    case 25:  __asm__ __volatile__ ("mtspr %0,%1" : : "K"(25),  "r"(val)); break;
    }
}

/* ── InitBoard ─────────────────────────────────────────────────────
 *
 * Chamado pelo kernel AROS assim que o bootloader passa controle.
 * Recebe:
 *   r3 = ponteiro para MLCD32Platform (na Chip RAM)
 *   r4 = CPUType
 *   r5 = MemSize
 *   r6 = PlatformInfo
 */

void InitBoard(const MLCD32Platform *platform)
{
    ml_platform = platform;

    /* 1. Configura BATs identity mapping (se não configurado pelo boot) */
    /* IBAT0/DBAT0: 0..256MB, Vs=Vp=1, PP=r/w */
    mtspr(528, 0xC000002CUL);  /* IBAT0U: BEPI=0, BL=256MB, Vs=1, Vp=1 */
    mtspr(529, 0x00000001UL);  /* IBAT0L: BRPN=0, WIMG=0, PP=1 */
    mtspr(536, 0xC000002CUL);  /* DBAT0U */
    mtspr(537, 0x00000001UL);  /* DBAT0L */

    /* 2. SDR1 — zero até o kernel criar page tables */
    mtspr(25, 0);

    /* 3. HID0 — liga BTIC (Branch Target Instruction Cache) */
    __asm__ __volatile__ (
        "mfspr 3, 1008\n\t"
        "ori    3, 3, 0x0200\n\t"  /* BTIC enable */
        "mtspr  1008, 3\n\t"
        : : : "r3"
    );

    /* 4. Configura vetor de interrupção externa (0x500) */
    /* O bootloader já configura IVPR, mas garantimos */
    __asm__ __volatile__ (
        "li  3, 0x0000\n\t"
        "mtspr 63, 3\n\t"     /* IVPR = 0 */
        : : : "r3"
    );

    /* 5. Habilita interrupções externas no MSR */
    uint32_t msr = mfmsr();
    msr |= (1 << 15);   /* MSR[EE] = 1 (external interrupt enable) */
    msr |= (1 << 12);   /* MSR[ME] = 1 (machine check enable) */
    mtmsr(msr);
}

/* ── Interrupt handler ─────────────────────────────────────────────
 *
 * Registrado no vetor 0x500. Chamado quando GPU VBlank,
 * CD-ROM, DMA ou timer disparam IRQ.
 */

void __attribute__((interrupt)) ml_irq_handler(void)
{
    uint32_t irq_status = ML_GPU_IRQ;

    if (irq_status & 1) {
        /* VBlank — sinaliza para o kernel */
        ML_GPU_IRQ = 0;  /* clear */
    }

    /* Leitura adicional: verifica CD-ROM, DMA */
    /* (placeholder — implementação completa com o scheduler AROS) */
}

/* ── Platform info — chamado pelo kernel AROS ───────────────────── */

uint32_t ReadMemSize(void)
{
    if (ml_platform) return ml_platform->total_ram;
    return ML_SYSRAM_SIZE + ML_CHIPRAM_SIZE;
}

uint32_t ReadPlatformInfo(void)
{
    return 0x0002;  /* ML-CD32 v1 */
}
