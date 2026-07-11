/*
 * CDG2 CDG2 Board Support Package
 * board.h — Definições canônicas do hardware
 *
 * ABI Version: 1.0
 * Conforme docs/aros/abi.md
 */
#ifndef CDG2_BOARD_H
#define CDG2_BOARD_H

#include <stdint.h>

#define CDG2_ABI_VERSION 0x0100

/* ── Mapa de Memória ────────────────────────────────────────────── */

#define ML_SYSRAM_BASE      0x00000000UL
#define ML_SYSRAM_SIZE      0x01000000UL  /* 16 MB */
#define ML_CHIPRAM_BASE     0x01000000UL
#define ML_CHIPRAM_SIZE     0x00400000UL  /* 4 MB */
#define ML_MAILBOX_BASE     0x01000000UL
#define ML_COLDFIRE_LOCAL   0x02000000UL
#define ML_COLDFIRE_LOCAL_SZ 0x00200000UL /* 2 MB */
#define ML_CF_IO_BASE       0x02200000UL
#define ML_CDROM_BASE       0x03000000UL
#define ML_DSP_BASE         0x03D00000UL
#define ML_DMA_BASE         0x03E00000UL
#define ML_GPU_BASE         0x04000000UL
#define ML_VRAM_BASE        0x01B00000UL
#define ML_VRAM_SIZE        0x00800000UL  /* 8 MB */
#define ML_MIU_BASE         0x05000000UL
#define ML_DVD_BASE         0x08000000UL
#define ML_BOOTROM_BASE     0xFF000000UL
#define ML_BOOTROM_SIZE     0x00080000UL  /* 512 KB */

/* ── GPU Lisa II ─────────────────────────────────────────────────── */

#define ML_GPU_CTRL         (*(volatile uint32_t*)(ML_GPU_BASE + 0x00))
#define ML_GPU_LIST_ADDR    (*(volatile uint32_t*)(ML_GPU_BASE + 0x04))
#define ML_GPU_STATUS       (*(volatile uint32_t*)(ML_GPU_BASE + 0x08))
#define ML_GPU_FRAME        (*(volatile uint32_t*)(ML_GPU_BASE + 0x10))
#define ML_GPU_IRQ          (*(volatile uint32_t*)(ML_GPU_BASE + 0x20))

#define ML_GPU_CTRL_KICK    1
#define ML_GPU_STATUS_IDLE  0
#define ML_GPU_STATUS_BUSY  1
#define ML_GPU_STATUS_VBLANK 2

/* ── ColdFire I/O (via mailbox) ─────────────────────────────────── */

#define ML_MAILBOX_CMD      (*(volatile uint32_t*)(ML_MAILBOX_BASE + 0x00))
#define ML_MAILBOX_RESP     (*(volatile uint32_t*)(ML_MAILBOX_BASE + 0x04))
#define ML_MAILBOX_STATUS   (*(volatile uint32_t*)(ML_MAILBOX_BASE + 0x08))
#define ML_MAILBOX_ARG      (*(volatile uint32_t*)(ML_MAILBOX_BASE + 0x0C))

#define ML_CF_STAT_IDLE     0
#define ML_CF_STAT_PENDING  1
#define ML_CF_STAT_BUSY     2

/* Comandos ColdFire */
#define ML_CF_CMD_EXEC      0x01
#define ML_CF_CMD_IO_READ   0x02
#define ML_CF_CMD_IO_WRITE  0x03
#define ML_CF_CMD_JOYPAD    0x04
#define ML_CF_CMD_CDROM_STAT 0x05
#define ML_CF_CMD_DMA_AUDIO 0x06
#define ML_CF_CMD_UART      0x07
#define ML_CF_CMD_HALT      0xFF

/* GPIO bits (joystick) */
#define ML_JOY_UP           (1 << 0)
#define ML_JOY_DOWN         (1 << 1)
#define ML_JOY_LEFT         (1 << 2)
#define ML_JOY_RIGHT        (1 << 3)
#define ML_JOY_A            (1 << 4)
#define ML_JOY_B            (1 << 5)
#define ML_JOY_START        (1 << 6)
#define ML_JOY_SELECT       (1 << 7)

/* ── DSP Audio ───────────────────────────────────────────────────── */

#define ML_DSP_CTRL         (*(volatile uint32_t*)(ML_DSP_BASE + 0x00))
#define ML_DSP_MASTER_VOL   (*(volatile uint32_t*)(ML_DSP_BASE + 0x04))
#define ML_DSP_OUT_L        (*(volatile uint32_t*)(ML_DSP_BASE + 0x08))
#define ML_DSP_OUT_R        (*(volatile uint32_t*)(ML_DSP_BASE + 0x0C))

/* ── DMA Controller ─────────────────────────────────────────────── */

#define ML_DMA_CHAN(n)      ((volatile uint32_t*)(ML_DMA_BASE + (n) * 0x10))
#define ML_DMA_SRC          0
#define ML_DMA_DST          4
#define ML_DMA_SIZE         8
#define ML_DMA_CTRL         12

#define ML_DMA_CTRL_START   1
#define ML_DMA_CTRL_RESET   2

/* ── CD-ROM ──────────────────────────────────────────────────────── */

#define ML_CDROM_REG(n)     (*(volatile uint32_t*)(ML_CDROM_BASE + (n) * 4))
#define ML_CDROM_CMD_READ   0x01
#define ML_CDROM_CMD_PAUSE  0x02
#define ML_CDROM_CMD_RESUME 0x03
#define ML_CDROM_CMD_MOUNT  0x10
#define ML_CDROM_CMD_LIST   0x11

/* ── CD32Platform (conforme abi.md) ──────────────────────────────── */

typedef struct __attribute__((packed)) {
    uint32_t magic;          /* 0xCD320001 */
    uint32_t total_ram;      /* 0x01800000 (24MB unified) */
    uint32_t chip_ram_base;  /* 0x00000000 (alias unified) */
    uint32_t chip_ram_size;  /* 24MB */
    uint32_t sys_ram_base;   /* 0x00000000 */
    uint32_t sys_ram_size;   /* 24MB */
    uint32_t vram_base;      /* 0x04010000 */
    uint32_t vram_size;      /* 8MB */
    uint32_t boot_rom_base;  /* 0xFF000000 */
    uint32_t boot_rom_size;  /* 512KB */
    uint32_t cf_mailbox;     /* 0x01000000 */
    uint32_t gpu_base;       /* 0x04000000 */
    uint32_t dsp_base;       /* 0x03D00000 */
    uint32_t dma_base;       /* 0x03E00000 */
    uint32_t cdrom_base;     /* 0x03000000 */
    uint32_t gpio_base;      /* 0x02200020 */
    uint32_t coldfire_base;  /* 0x02200000 */
} CDG2Platform;

/* ── Helper functions ─────────────────────────────────────────────── */

static inline void ml_mailbox_send(uint32_t cmd, uint32_t arg) {
    while (ML_MAILBOX_STATUS != ML_CF_STAT_IDLE) {}
    ML_MAILBOX_ARG = arg;
    ML_MAILBOX_CMD = cmd;
    ML_MAILBOX_STATUS = ML_CF_STAT_PENDING;
}

static inline uint32_t ml_mailbox_recv(void) {
    while (ML_MAILBOX_STATUS != ML_CF_STAT_IDLE) {}
    return ML_MAILBOX_RESP;
}

static inline uint16_t ml_read_joypad(void) {
    ml_mailbox_send(ML_CF_CMD_IO_READ, 0x20); /* GPIO offset */
    return (uint16_t)ml_mailbox_recv();
}

#endif /* CDG2_BOARD_H */
