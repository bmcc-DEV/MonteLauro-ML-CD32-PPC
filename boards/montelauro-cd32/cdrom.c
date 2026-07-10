/*
 * MonteLauro ML-CD32 BSP — cdrom.c
 * Driver de CD-ROM via DMA controller + mailbox ColdFire.
 *
 * O CD-ROM 12x é controlado via registers em 0x0300_0000.
 * Dados são transferidos via DMA (canal 0 = CDROM > RAM).
 * O ColdFire gerencia a camada de baixo nível (SPI-like).
 */

#include <exec/types.h>
#include <exec/interrupts.h>

#include "board.h"

#define SECTOR_SIZE   2048
#define CD_STAT_REG   ML_CDROM_REG(0)
#define CD_FLAGS_REG  ML_CDROM_REG(1)
#define CD_LBA_REG    ML_CDROM_REG(2)
#define CD_REMAIN_REG ML_CDROM_REG(3)
#define CD_CMD_REG    ML_CDROM_REG(4)
#define CD_ARG0_REG   ML_CDROM_REG(5)
#define CD_ARG1_REG   ML_CDROM_REG(6)

#define CD_STAT_PRESENT   (1 << 0)
#define CD_FLAG_READY     (1 << 1)
#define CD_FLAG_DATA      (1 << 2)
#define CD_FLAG_ERROR     (1 << 3)

/* ── DMA channel 0 (CDROM → RAM) ─────────────────────────────────── */

#define DMA_CDROM_CHAN 0
#define DMA_REGS       ML_DMA_CHAN(DMA_CDROM_CHAN)

static void dma_wait(void)
{
    volatile uint32_t *ctrl = &DMA_REGS[ML_DMA_CTRL / 4];
    while (*ctrl & ML_DMA_CTRL_START) {}
}

static void dma_transfer(uint32_t src, uint32_t dst, uint32_t size)
{
    volatile uint32_t *chan = DMA_REGS;
    chan[ML_DMA_SRC / 4]   = src;
    chan[ML_DMA_DST / 4]   = dst;
    chan[ML_DMA_SIZE / 4]  = size;
    chan[ML_DMA_CTRL / 4]  = ML_DMA_CTRL_START;
    dma_wait();
}

/* ── CD-ROM API ─────────────────────────────────────────────────── */

int ml_cdrom_init(void)
{
    /* Verifica se há disco */
    if (!(CD_STAT_REG & CD_STAT_PRESENT))
        return -1;

    /* Aguarda spin-up */
    int timeout = 1000000;
    while (!(CD_FLAGS_REG & CD_FLAG_READY) && --timeout > 0) {}
    return (timeout > 0) ? 0 : -1;
}

int ml_cdrom_read_sector(uint32_t lba, uint8_t *buffer)
{
    if (!(CD_FLAGS_REG & CD_FLAG_READY))
        return -1;

    /* Inicia leitura do setor */
    CD_ARG0_REG = lba;
    CD_ARG1_REG = 1;  /* 1 setor */
    CD_CMD_REG  = ML_CDROM_CMD_READ;

    /* Aguarda dados prontos */
    int timeout = 100000;
    while (!(CD_FLAGS_REG & CD_FLAG_DATA) && --timeout > 0) {}

    if (timeout == 0 || (CD_FLAGS_REG & CD_FLAG_ERROR))
        return -1;

    /* Transfere via DMA do CDROM buffer para RAM */
    /* (endereço fonte é o buffer interno do CDROM, mapeado em 0x0300_1000) */
    dma_transfer(0x03001000, (uint32_t)buffer, SECTOR_SIZE);
    return 0;
}

int ml_cdrom_read_multi(uint32_t lba, int count, uint8_t *buffer)
{
    for (int i = 0; i < count; i++) {
        if (ml_cdrom_read_sector(lba + i, buffer + i * SECTOR_SIZE) < 0)
            return -1;
    }
    return 0;
}

/* ── ISO9660 helpers ────────────────────────────────────────────────
 *
 * Leitura do Volume Descriptor e navegação básica.
 * O CD-ROM controller já parseia ISO9660 (ver cdrom.rs do emulador).
 * Aqui apenas wrappers para acessar via DMA.
 */

int ml_cdrom_mount(void)
{
    CD_CMD_REG = ML_CDROM_CMD_MOUNT;
    int timeout = 100000;
    while (!(CD_FLAGS_REG & CD_FLAG_READY) && --timeout > 0) {}
    return (timeout > 0) ? 0 : -1;
}

/* ── Integração com AROS trackdisk.device ─────────────────────────
 *
 * Para integração completa com o sistema de storage do AROS, este
 * driver deve ser registrado como trackdisk.device, respondendo
 * aos comandos TD_READ, TD_SEEK, TD_FORMAT, etc.
 *
 * O mapping é direto:
 *   TD_READ(lba, buf, count) → ml_cdrom_read_multi(lba, count, buf)
 */
