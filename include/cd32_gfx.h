#ifndef CD32_GFX_H
#define CD32_GFX_H

#include <stdint.h>

/* ── Comandos da Display List ────────────────────────────────────── */
enum {
    CD32_DL_NOP      = 0x0000,
    CD32_DL_CLEAR    = 0x0001, /* data = 0x00RRGGBB */
    CD32_DL_RECT     = 0x0002, /* flags=(x:6|y:6|w:6|h:6), data=color */
    CD32_DL_TRIANGLE = 0x0003, /* data = ponteiro p/ vertices no buffer */
    CD32_DL_LINE     = 0x0004, /* flags=(x0:8|y0:8), data=(x1:16|y1:16|color:32) */
    CD32_DL_END      = 0xFFFF,
};

/* ── Display List ───────────────────────────────────────────────── */
typedef struct {
    uint32_t buffer[2048]; /* 16KB de comandos */
    int      count;         /* numero de comandos */
    uint32_t ptr;           /* offset dentro do buffer p/ dados de vertices */
} cd32_dl_t;

/* Inicializa video */
void cd32_gfx_init(void);

/* Cria uma nova display list */
cd32_dl_t *cd32_gfx_begin(void);

/* Adiciona comandos a lista */
void cd32_gfx_clear(cd32_dl_t *dl, uint32_t color);
void cd32_gfx_rect(cd32_dl_t *dl, int x, int y, int w, int h, uint32_t color);
void cd32_gfx_tri(cd32_dl_t *dl, int x0, int y0, int x1, int y1, int x2, int y2, uint32_t color);
void cd32_gfx_line(cd32_dl_t *dl, int x0, int y0, int x1, int y1, uint32_t color);

/* Submete a display list para GPU e aguarda VBlank */
void cd32_gfx_submit(cd32_dl_t *dl);

#endif
