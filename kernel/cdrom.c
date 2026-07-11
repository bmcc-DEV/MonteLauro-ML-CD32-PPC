/*
 * MonteLauro CD3² — CD-ROM (loader de jogos)
 *
 * Le setores do CD-ROM via DMA, monta ISO9660 basico,
 * encontra arquivo "GAME.ELF;1" e carrega na RAM.
 */

#include "cd32.h"

#define SECTOR_SIZE 2048
#define CD_STAT  (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x00))
#define CD_FLAGS (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x04))
#define CD_LBA   (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x08))
#define CD_CMD   (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x10))
#define CD_ARG0  (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x14))
#define CD_ARG1  (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x18))

int cd32_cdrom_init(void)
{
    if (!(CD_STAT & 1)) return -1;  /* sem disco */
    CD_CMD = 0x10;                   /* mount ISO9660 */
    int t = 1000000;
    while (!(CD_FLAGS & 2) && --t > 0) {}
    return (t > 0) ? 0 : -1;
}

int cd32_cdrom_read(uint32_t lba, int count, void *buf)
{
    CD_ARG0 = lba;
    CD_ARG1 = count;
    CD_CMD  = 1;                     /* read sectors */
    int t = 100000;
    while (!(CD_FLAGS & 4) && --t > 0) {}
    if (t == 0 || (CD_FLAGS & 8)) return -1;

    /* DMA do buffer do CDROM pra RAM */
    cd32_dma_copy(0x03001000, (uint32_t)buf, count * SECTOR_SIZE);
    return 0;
}

void *cd32_cdrom_load(const char *path)
{
    /* Stub: procura "GAME.ELF" na raiz do CD e carrega */
    /* Implementacao real faria parse ISO9660 e DMA do arquivo */
    (void)path;
    return NULL;
}
