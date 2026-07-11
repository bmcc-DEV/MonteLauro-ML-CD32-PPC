#include "cd32.h"
#include "cd32_gfx.h"

static cd32_dl_t dl_instance;

void cd32_gfx_init(void) { cd32_video_init(); }

cd32_dl_t *cd32_gfx_begin(void) {
    dl_instance.count = 0;
    dl_instance.ptr = 64; /* offset em words para dados de vertices */
    return &dl_instance;
}

static void dl_push(cd32_dl_t *dl, uint16_t op, uint16_t flags, uint32_t data) {
    if (dl->count >= 2036) return;
    int i = dl->count;
    uint32_t *buf = dl->buffer;
    buf[i * 2 + 0] = ((uint32_t)op << 16) | flags;
    buf[i * 2 + 1] = data;
    dl->count++;
}

void cd32_gfx_clear(cd32_dl_t *dl, uint32_t color) {
    dl_push(dl, CD32_DL_CLEAR, 0, color & 0x00FFFFFF);
}

void cd32_gfx_tri(cd32_dl_t *dl, int x0, int y0, int x1, int y1, int x2, int y2, uint32_t color) {
    int wi = dl->ptr;
    uint32_t *buf = dl->buffer;
    buf[wi + 0] = ((uint32_t)(int16_t)x0 << 16) | (uint16_t)(int16_t)y0;
    buf[wi + 1] = ((uint32_t)(int16_t)x1 << 16) | (uint16_t)(int16_t)y1;
    buf[wi + 2] = ((uint32_t)(int16_t)x2 << 16) | (uint16_t)(int16_t)y2;
    buf[wi + 3] = color & 0x00FFFFFF;
    dl->ptr += 4; /* 4 words por triangulo */
    /* data = byte offset no cmd_buf = wi * 4 */
    dl_push(dl, CD32_DL_TRIANGLE, 0, wi * 4);
}

void cd32_gfx_submit(cd32_dl_t *dl) {
    dl_push(dl, CD32_DL_END, 0, 0);
    /* Copia tudo para Chip RAM */
    volatile uint32_t *dest = (volatile uint32_t *)0x00100000;
    int total = dl->ptr + 2; /* comandos + vertices */
    if (total > 2048) total = 2048;
    for (int i = 0; i < total; i++)
        dest[i] = dl->buffer[i];
    CD32_GPU_LIST = 0x00100000;
    CD32_GPU_CTRL = CD32_GPU_KICK;
    cd32_video_wait_vblank();
}
