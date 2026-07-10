// Auto-generated. DO NOT EDIT.
#ifndef CD32_PLATFORM_H
#define CD32_PLATFORM_H

#include <stdint.h>

#define CD32_PLATFORM_MAGIC   0xCD320001u
#define CD32_PLATFORM_VERSION 1u

typedef struct __attribute__((packed)) {
    uint32_t magic;
    uint32_t total_ram;
    uint32_t chip_ram_base;
    uint32_t chip_ram_size;
    uint32_t sys_ram_base;
    uint32_t sys_ram_size;
    uint32_t vram_base;
    uint32_t vram_size;
    uint32_t boot_rom_base;
    uint32_t boot_rom_size;
    uint32_t cf_mailbox;
    uint32_t gpu_base;
    uint32_t dsp_base;
    uint32_t dma_base;
    uint32_t cdrom_base;
    uint32_t gpio_base;
    uint32_t coldfire_base;
} CD32Platform;

// Mailbox commands
#define CF_CMD_EXEC          0x01
#define CF_CMD_IO_READ       0x02
#define CF_CMD_IO_WRITE      0x03
#define CF_CMD_JOYPAD        0x04
#define CF_CMD_CDROM_STATUS  0x05
#define CF_CMD_DMA_AUDIO     0x06
#define CF_CMD_UART_WRITE    0x07
#define CF_CMD_HALT          0xFF

#endif
