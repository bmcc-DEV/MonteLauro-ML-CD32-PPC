/*
 * MonteLauro CD³² — Game Runtime API (libcd32)
 * 
 * Header público para desenvolvimento de jogos.
 * Consulte sdk/api.md para documentação completa.
 *
 * ABI v1.0 — MIT License
 */
#ifndef CD32_H
#define CD32_H

#include <stdint.h>
#include <stddef.h>

/* ── Hardware base addresses ─────────────────────────────────────── */
#define CD32_GPU_BASE     0x04000000UL
#define CD32_VRAM_BASE    0x04010000UL
#define CD32_DSP_BASE     0x03D00000UL
#define CD32_DMA_BASE     0x03E00000UL
#define CD32_CDROM_BASE   0x03000000UL
#define CD32_GPIO_BASE    0x02200020UL
#define CD32_MAILBOX      0x01000000UL

/* ── GPU Lisa II ─────────────────────────────────────────────────── */
#define CD32_GPU_CTRL     (*(volatile uint32_t*)(CD32_GPU_BASE + 0x00))
#define CD32_GPU_LIST     (*(volatile uint32_t*)(CD32_GPU_BASE + 0x04))
#define CD32_GPU_STATUS   (*(volatile uint32_t*)(CD32_GPU_BASE + 0x08))
#define CD32_GPU_FRAME    (*(volatile uint32_t*)(CD32_GPU_BASE + 0x10))
#define CD32_FB_W         640
#define CD32_FB_H         480

enum { CD32_GPU_KICK = 1, CD32_GPU_IDLE = 0, CD32_GPU_VBLANK = 2 };

typedef struct { float x, y, z; uint32_t color; } cd32_vertex;
typedef struct { uint32_t type; uint32_t count; uint32_t data; uint32_t color; } cd32_prim;

void cd32_video_init(void);
void cd32_video_kick(void);
void cd32_video_wait_vblank(void);
void cd32_video_clear(uint32_t color);
void cd32_video_putpixel(int x, int y, uint32_t color);
void cd32_video_rect(int x, int y, int w, int h, uint32_t color);

/* ── Input ───────────────────────────────────────────────────────── */
enum {
    CD32_JOY_UP    = 1 << 0,
    CD32_JOY_DOWN  = 1 << 1,
    CD32_JOY_LEFT  = 1 << 2,
    CD32_JOY_RIGHT = 1 << 3,
    CD32_JOY_A     = 1 << 4,
    CD32_JOY_B     = 1 << 5,
    CD32_JOY_START = 1 << 6,
    CD32_JOY_SEL   = 1 << 7,
};

void     cd32_input_init(void);
void     cd32_input_poll(void);
uint16_t cd32_joypad_read(void);
uint16_t cd32_joypad_pressed(int btn);
uint16_t cd32_joypad_just_pressed(int btn);

/* ── Audio ───────────────────────────────────────────────────────── */
#define CD32_AUDIO_CHANNELS 8

void cd32_audio_init(void);
void cd32_audio_play(int ch, int16_t *samples, int count, int loop);
void cd32_audio_stop(int ch);
void cd32_audio_volume(int ch, int vol);  /* 0-1024 */
void cd32_audio_pan(int ch, int pan);     /* 0-255 */

/* ── CD-ROM ───────────────────────────────────────────────────────── */
int  cd32_cdrom_init(void);
int  cd32_cdrom_read(uint32_t lba, int count, void *buf);
void *cd32_cdrom_load(const char *path);   /* retorna entry point */

/* ── DMA ──────────────────────────────────────────────────────────── */
void cd32_dma_copy(uint32_t src, uint32_t dst, uint32_t size);
void cd32_dma_wait(void);

/* ── System ────────────────────────────────────────────────────────── */
void cd32_halt(void) __attribute__((noreturn));
void cd32_printf(const char *fmt, ...);

#endif
