/*
 * CDG² — CD-ROM + ISO9660 + ELF loader
 *
 * Pipeline completo:
 *   cdrom_init() → mount ISO9660
 *   cdrom_load("GAME.ELF") → parse ISO, load ELF segments, retorna entry
 */

#include "cd32.h"
#include <string.h>

#define SECTOR_SIZE     2048
#define PVD_LBA         16
#define ELF_MAGIC       0x7F454C46

/* ── Hardware registers ─────────────────────────────────────────── */
#define CD_STAT  (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x00))
#define CD_FLAGS (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x04))
#define CD_LBA   (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x08))
#define CD_CMD   (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x10))
#define CD_ARG0  (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x14))
#define CD_ARG1  (*(volatile uint32_t*)(CD32_CDROM_BASE + 0x18))
#define CD_DATA  ((volatile uint8_t*)0x03001000)

/* ── ELF32 structures ──────────────────────────────────────────── */
typedef struct {
    uint32_t magic;
    uint8_t  cls, endian, ver, osabi, abiver, pad[7];
    uint16_t type, machine;
    uint32_t version;
    uint32_t entry;
    uint32_t phoff;
    uint32_t shoff;
    uint32_t flags;
    uint16_t ehsize, phentsize, phnum, shentsize, shnum, shstrndx;
} __attribute__((packed)) elf32_hdr;

typedef struct {
    uint32_t type, offset, vaddr, paddr, filesz, memsz, flags, align;
} __attribute__((packed)) elf32_phdr;

/* ── Raw sector read (sem DMA, via I/O regs) ───────────────────── */
static int read_sector_raw(uint32_t lba, uint8_t *buf)
{
    CD_ARG0 = lba;
    CD_ARG1 = 1;
    CD_CMD  = 1;
    int t = 50000;
    while (!(CD_FLAGS & 4) && --t > 0) {}
    if (t == 0 || (CD_FLAGS & 8)) return -1;
    /* Copy from CDROM data buffer to RAM */
    for (int i = 0; i < SECTOR_SIZE; i++)
        buf[i] = CD_DATA[i];
    return 0;
}

/* ── ISO9660 helpers ────────────────────────────────────────────── */
static uint32_t le32(const uint8_t *p) {
    return (uint32_t)p[0] | (uint32_t)p[1]<<8 | (uint32_t)p[2]<<16 | (uint32_t)p[3]<<24;
}

/* ── cdrom_init ──────────────────────────────────────────────────── */
int cd32_cdrom_init(void)
{
    if (!(CD_STAT & 1)) return -1;
    CD_CMD = 0x10;  /* mount */
    int t = 1000000;
    while (!(CD_FLAGS & 2) && --t > 0) {}
    return (t > 0) ? 0 : -1;
}

/* ── cdrom_read (via DMA) ───────────────────────────────────────── */
int cd32_cdrom_read(uint32_t lba, int count, void *buf)
{
    CD_ARG0 = lba;
    CD_ARG1 = count;
    CD_CMD  = 1;
    int t = 100000;
    while (!(CD_FLAGS & 4) && --t > 0) {}
    if (t == 0 || (CD_FLAGS & 8)) return -1;
    cd32_dma_copy(0x03001000, (uint32_t)buf, count * SECTOR_SIZE);
    return 0;
}

/* ── ISO9660: ler root directory record ─────────────────────────── */
static int read_root_dir(uint32_t *lba, uint32_t *size)
{
    uint8_t pvd[SECTOR_SIZE];
    if (read_sector_raw(PVD_LBA, pvd) < 0) return -1;
    if (pvd[0] != 1) return -1;  /* not PVD */

    /* Root directory record at offset 156 */
    uint8_t *r = pvd + 156;
    *lba  = le32(r + 2);
    *size = le32(r + 10);
    return 0;
}

/* ── ISO9660: find file in directory ────────────────────────────── */
static int find_in_dir(const char *name, uint32_t dir_lba, uint32_t dir_size,
                       uint32_t *file_lba, uint32_t *file_size)
{
    int nsectors = (dir_size + SECTOR_SIZE - 1) / SECTOR_SIZE;
    uint8_t buf[nsectors * SECTOR_SIZE];

    for (int i = 0; i < nsectors; i++) {
        if (read_sector_raw(dir_lba + i, buf + i * SECTOR_SIZE) < 0)
            return -1;
    }

    uint32_t off = 0;
    while (off < dir_size) {
        uint8_t *rec = buf + off;
        int rec_len = rec[0];
        if (rec_len == 0) { off++; continue; }
        int name_len = rec[32];
        char entry_name[256];
        if (name_len < 2 || off >= dir_size) break;

        /* Skip . and .. */
        if (name_len == 1 && (rec[33] == 0 || rec[33] == 1)) {
            off += rec_len;
            continue;
        }

        int copy = name_len < 255 ? name_len : 255;
        for (int i = 0; i < copy; i++) entry_name[i] = rec[33 + i];
        entry_name[copy] = '\0';

        /* Strip version separator */
        char *ver = entry_name;
        while (*ver && *ver != ';') ver++;
        *ver = '\0';

        if (strcasecmp(entry_name, name) == 0) {
            *file_lba  = le32(rec + 2);
            *file_size = le32(rec + 10);
            return 0;
        }
        off += rec_len;
    }
    return -1;
}

/* ── ELF loader ──────────────────────────────────────────────────── */
static void *load_elf(uint32_t lba, uint32_t size)
{
    int nsectors = (size + SECTOR_SIZE - 1) / SECTOR_SIZE;
    uint8_t buf[512];  /* ELF header fits in first sector */

    if (read_sector_raw(lba, buf) < 0) return NULL;

    elf32_hdr *hdr = (elf32_hdr*)buf;
    if (hdr->magic != ELF_MAGIC || hdr->machine != 20) return NULL;  /* 20 = PPC */

    /* Read program headers */
    int ph_sectors = (hdr->phoff + hdr->phnum * hdr->phentsize + SECTOR_SIZE - 1) / SECTOR_SIZE;
    uint8_t ph_buf[ph_sectors * SECTOR_SIZE];
    for (int i = 0; i < ph_sectors; i++)
        if (read_sector_raw(lba + i, ph_buf + i * SECTOR_SIZE) < 0) return NULL;

    /* Load each PT_LOAD segment (type=1) directly to its vaddr via DMA */
    for (int i = 0; i < hdr->phnum; i++) {
        elf32_phdr *ph = (elf32_phdr*)(ph_buf + hdr->phoff + i * hdr->phentsize);
        if (ph->type != 1) continue;  /* PT_LOAD */

        uint32_t seg_lba  = lba + ph->offset / SECTOR_SIZE;
        uint32_t seg_skip = ph->offset % SECTOR_SIZE;
        uint32_t dest     = ph->vaddr;
        uint32_t left     = ph->filesz;

        /* First sector might have offset */
        if (seg_skip > 0) {
            uint8_t tmp[SECTOR_SIZE];
            if (read_sector_raw(seg_lba, tmp) < 0) return NULL;
            uint32_t chunk = SECTOR_SIZE - seg_skip;
            if (chunk > left) chunk = left;
            memcpy((void*)dest, tmp + seg_skip, chunk);
            dest += chunk;
            left -= chunk;
            seg_lba++;
        }

        /* Remaining sectors via DMA */
        int dma_sectors = (left + SECTOR_SIZE - 1) / SECTOR_SIZE;
        if (dma_sectors > 0) {
            cd32_dma_copy(seg_lba * SECTOR_SIZE, dest, dma_sectors * SECTOR_SIZE);
            dest += dma_sectors * SECTOR_SIZE;
        }

        /* Zero BSS (memsz > filesz) */
        if (ph->memsz > ph->filesz) {
            uint32_t bss = ph->memsz - ph->filesz;
            for (uint32_t i = 0; i < bss; i++)
                ((volatile uint8_t*)dest)[i] = 0;
        }
    }

    return (void*)hdr->entry;
}

/* ── cdrom_load ──────────────────────────────────────────────────── */
void *cd32_cdrom_load(const char *path)
{
    (void)path;  /* ignored — always looks for GAME.ELF */

    uint32_t root_lba, root_size;
    if (read_root_dir(&root_lba, &root_size) < 0) return NULL;

    uint32_t file_lba, file_size;
    if (find_in_dir("GAME.ELF", root_lba, root_size, &file_lba, &file_size) < 0)
        return NULL;

    /* Read and load ELF */
    void *entry = load_elf(file_lba, file_size);
    if (entry) {
        cd32_printf("GAME.ELF loaded: entry=0x%08X size=%d sectors\n",
                     (uint32_t)entry, (file_size + SECTOR_SIZE - 1) / SECTOR_SIZE);
    }
    return entry;
}
