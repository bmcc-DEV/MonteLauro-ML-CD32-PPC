/*
 * MontêLauro CD+G² — DMA (transferencias entre RAM e perifericos)
 *
 * 4 canais: 0=CDROM, 1=GPU, 2=Audio, 3=ColdFire
 */

#include "cd32.h"

#define DMA_CHAN(n) ((volatile uint32_t*)(CD32_DMA_BASE + (n) * 0x10))

void cd32_dma_copy(uint32_t src, uint32_t dst, uint32_t size)
{
    volatile uint32_t *ch = DMA_CHAN(0);
    ch[0] = src;       /* DMA_SRC */
    ch[1] = dst;       /* DMA_DST */
    ch[2] = size;      /* DMA_SIZE */
    ch[3] = 1;         /* DMA_CTRL: start */
    while (ch[3] & 1) {}
}

void cd32_dma_wait(void)
{
    volatile uint32_t *ch = DMA_CHAN(0);
    while (ch[3] & 1) {}
}

void cd32_halt(void)
{
    while (1) {}
}
