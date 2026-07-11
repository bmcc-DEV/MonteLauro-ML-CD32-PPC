/*
 * MonteLauro CD+G² — Vídeo Lisa II (TBDR)
 * Framebuffer 640x480 RGBA32 com font 8x16 para debug.
 * Sem variaveis globais (usa defines diretos).
 */

#include <stdarg.h>
#include "cd32.h"

#define FB ((volatile uint32_t*)CD32_VRAM_BASE)
#define FONT_W 8
#define FONT_H 16

void cd32_video_init(void)
{
    cd32_video_clear(0x00000000);
    cd32_video_kick();
}

void cd32_video_kick(void)
{
    CD32_GPU_LIST = CD32_VRAM_BASE;
    CD32_GPU_CTRL = CD32_GPU_KICK;
}

void cd32_video_wait_vblank(void)
{
    uint32_t f = CD32_GPU_FRAME;
    while (CD32_GPU_FRAME == f) {}
}

void cd32_video_clear(uint32_t color)
{
    for (int i = 0; i < CD32_FB_W * CD32_FB_H; i++)
        FB[i] = color;
}

void cd32_video_putpixel(int x, int y, uint32_t color)
{
    if (x < 0 || x >= CD32_FB_W || y < 0 || y >= CD32_FB_H) return;
    FB[y * CD32_FB_W + x] = color;
}

void cd32_video_rect(int x, int y, int w, int h, uint32_t color)
{
    for (int row = 0; row < h; row++)
        for (int col = 0; col < w; col++)
            cd32_video_putpixel(x + col, y + row, color);
}

/* ── Minimal vsnprintf ───────────────────────────────────────────── */
static int my_vsnprintf(char *buf, int size, const char *fmt, va_list ap)
{
    int pos = 0;
    char tmp[32];
    for (int i = 0; fmt[i] && pos < size - 1; i++) {
        if (fmt[i] != '%') { buf[pos++] = fmt[i]; continue; }
        i++;
        int pad_zero = 0, width = 0;
        if (fmt[i] == '0') { pad_zero = 1; i++; }
        while (fmt[i] >= '0' && fmt[i] <= '9') width = width*10 + (fmt[i++]-'0');
        switch (fmt[i]) {
        case 'd': {
            int val = va_arg(ap, int);
            int neg = 0, tpos = 0;
            if (val < 0) { neg = 1; val = -val; }
            if (val == 0) tmp[tpos++] = '0';
            while (val > 0) { tmp[tpos++] = '0' + (val % 10); val /= 10; }
            int len = tpos + neg;
            for (int p = 0; pos < size-1 && p < (width > len ? width - len : 0); p++)
                buf[pos++] = pad_zero ? '0' : ' ';
            if (neg) buf[pos++] = '-';
            for (int p = tpos-1; p >= 0 && pos < size-1; p--) buf[pos++] = tmp[p];
            break;
        }
        case 'u': {
            unsigned int val = va_arg(ap, unsigned int);
            int tpos = 0;
            if (val == 0) tmp[tpos++] = '0';
            while (val > 0) { tmp[tpos++] = '0' + (val % 10); val /= 10; }
            for (int p = tpos-1; p >= 0 && pos < size-1; p--) buf[pos++] = tmp[p];
            break;
        }
        case 'x': case 'X': {
            unsigned int val = va_arg(ap, unsigned int);
            int tpos = 0;
            if (val == 0) tmp[tpos++] = '0';
            while (val > 0) { int d = val % 16; tmp[tpos++] = d < 10 ? '0'+d : 'a'+d-10; val /= 16; }
            for (int p = tpos-1; p >= 0 && pos < size-1; p--) buf[pos++] = tmp[p];
            break;
        }
        case 's': {
            const char *s = va_arg(ap, const char*);
            if (!s) s = "(null)";
            while (*s && pos < size-1) buf[pos++] = *s++;
            break;
        }
        case 'c': buf[pos++] = (char)va_arg(ap, int); break;
        default: buf[pos++] = '%'; buf[pos++] = fmt[i]; break;
        }
    }
    buf[pos] = '\0';
    return pos;
}

/* ── Font 8x16 bitmap (inline data, sem .rodata) ────────────────── */
/* Gerada como array literal para evitar variavel global. */
#define FONT_GLYPH(c, b0,b1,b2,b3,b4,b5,b6,b7,b8,b9,b10) \
    case c: glyph[0]=b0;glyph[1]=b1;glyph[2]=b2;glyph[3]=b3; \
            glyph[4]=b4;glyph[5]=b5;glyph[6]=b6;glyph[7]=b7; \
            glyph[8]=b8;glyph[9]=b9;glyph[10]=b10; break;

/* Cursor position stored in Chip RAM (evita variaveis globais) */
#define CX (*(volatile int*)0x01001000)
#define CY (*(volatile int*)0x01001004)

void cd32_printf(const char *fmt, ...)
{
    if (CX < 0) { CX = 0; CY = 0; }

    char buf[256];
    va_list args;
    va_start(args, fmt);
    my_vsnprintf(buf, sizeof(buf), fmt, args);
    va_end(args);

    for (char *p = buf; *p; p++) {
        char c = *p;
        if (c == '\n') { CX = 0; CY += FONT_H; continue; }
        if (c == '\r') { CX = 0; continue; }
        if (c == '\t') { CX = (CX + 32) & ~31; continue; }

        uint8_t glyph[16] = {0};
        switch ((unsigned char)c) {
            case 'A':case 'B':case 'C':case 'D':case 'E':case 'F':case 'G':
            case 'H':case 'I':case 'J':case 'K':case 'L':case 'M':case 'N':
            case 'O':case 'P':case 'Q':case 'R':case 'S':case 'T':case 'U':
            case 'V':case 'W':case 'X':case 'Y':case 'Z':
            case '0':case '1':case '2':case '3':case '4':
            case '5':case '6':case '7':case '8':case '9':
            case ' ':case '.':case ',':case '!':case ':':case '-':
            {
                const uint8_t f[128][11] = {
                    ['A']={0x18,0x24,0x42,0x42,0x42,0x7E,0x42,0x42,0x42,0x42,0x42},
                    ['B']={0x7C,0x42,0x42,0x42,0x7C,0x42,0x42,0x42,0x42,0x42,0x7C},
                    ['C']={0x3C,0x42,0x40,0x40,0x40,0x40,0x40,0x40,0x40,0x42,0x3C},
                    ['D']={0x78,0x44,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x44,0x78},
                    ['E']={0x7E,0x40,0x40,0x40,0x7C,0x40,0x40,0x40,0x40,0x40,0x7E},
                    ['F']={0x7E,0x40,0x40,0x40,0x7C,0x40,0x40,0x40,0x40,0x40,0x40},
                    ['G']={0x3C,0x42,0x40,0x40,0x40,0x4E,0x42,0x42,0x42,0x42,0x3C},
                    ['H']={0x42,0x42,0x42,0x42,0x7E,0x42,0x42,0x42,0x42,0x42,0x42},
                    ['I']={0x7E,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x7E},
                    ['J']={0x06,0x06,0x06,0x06,0x06,0x06,0x06,0x06,0x46,0x26,0x1C},
                    ['K']={0x42,0x44,0x48,0x50,0x60,0x60,0x50,0x48,0x44,0x42,0x42},
                    ['L']={0x40,0x40,0x40,0x40,0x40,0x40,0x40,0x40,0x40,0x40,0x7E},
                    ['M']={0x42,0x66,0x5A,0x5A,0x42,0x42,0x42,0x42,0x42,0x42,0x42},
                    ['N']={0x42,0x62,0x52,0x52,0x4A,0x4A,0x46,0x46,0x42,0x42,0x42},
                    ['O']={0x3C,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x3C},
                    ['P']={0x7C,0x42,0x42,0x42,0x7C,0x40,0x40,0x40,0x40,0x40,0x40},
                    ['Q']={0x3C,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x52,0x4A,0x3C},
                    ['R']={0x7C,0x42,0x42,0x42,0x7C,0x48,0x44,0x42,0x42,0x42,0x42},
                    ['S']={0x3C,0x42,0x40,0x40,0x3C,0x02,0x02,0x02,0x02,0x42,0x3C},
                    ['T']={0x7E,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x18},
                    ['U']={0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x3C},
                    ['V']={0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x42,0x24,0x24,0x18},
                    ['W']={0x42,0x42,0x42,0x42,0x42,0x42,0x5A,0x5A,0x66,0x66,0x42},
                    ['X']={0x42,0x42,0x24,0x24,0x18,0x18,0x18,0x24,0x24,0x42,0x42},
                    ['Y']={0x42,0x42,0x24,0x24,0x18,0x18,0x18,0x18,0x18,0x18,0x18},
                    ['Z']={0x7E,0x02,0x04,0x08,0x10,0x10,0x20,0x40,0x40,0x40,0x7E},
                    ['0']={0x3C,0x42,0x46,0x4A,0x52,0x62,0x42,0x42,0x42,0x42,0x3C},
                    ['1']={0x18,0x38,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x18,0x7E},
                    ['2']={0x3C,0x42,0x02,0x02,0x04,0x08,0x10,0x20,0x40,0x40,0x7E},
                    ['3']={0x3C,0x42,0x02,0x02,0x1C,0x02,0x02,0x02,0x02,0x42,0x3C},
                    ['4']={0x04,0x0C,0x14,0x24,0x44,0x44,0x7E,0x04,0x04,0x04,0x04},
                    ['5']={0x7E,0x40,0x40,0x40,0x7C,0x02,0x02,0x02,0x02,0x42,0x3C},
                    ['6']={0x1C,0x20,0x40,0x40,0x7C,0x42,0x42,0x42,0x42,0x42,0x3C},
                    ['7']={0x7E,0x02,0x02,0x04,0x08,0x10,0x20,0x20,0x20,0x20,0x20},
                    ['8']={0x3C,0x42,0x42,0x42,0x3C,0x42,0x42,0x42,0x42,0x42,0x3C},
                    ['9']={0x3C,0x42,0x42,0x42,0x42,0x3E,0x02,0x02,0x02,0x04,0x38},
                    [' ']={0},
                    ['.']={0,0,0,0,0,0,0,0,0x18,0x18,0},
                    [',']={0,0,0,0,0,0,0,0,0x18,0x18,0x08},
                    ['!']={0x18,0x18,0x18,0x18,0x18,0x18,0x18,0,0x18,0x18,0},
                    [':']={0,0,0,0,0x18,0x18,0,0,0x18,0x18,0},
                    ['-']={0,0,0,0,0,0x7E,0,0,0,0,0},
                };
                const uint8_t *g = f[(int)c];
                if (CY + FONT_H > CD32_FB_H) { CY = 0; cd32_video_clear(0); }
                for (int row = 0; row < 11; row++) {
                    uint8_t bits = g[row];
                    for (int col = 0; col < 8; col++)
                        cd32_video_putpixel(CX + col, CY + row,
                            (bits & (0x80 >> col)) ? 0xFFFFFFFF : 0);
                }
                CX += FONT_W;
                if (CX + FONT_W > CD32_FB_W) { CX = 0; CY += FONT_H; }
                break;
            }
        }
    }
    cd32_video_kick();
}

void cd32_gpu_clear(uint32_t color)
{
    cd32_video_clear(color);
}

void cd32_gpu_triangle(int x0, int y0, int x1, int y1, int x2, int y2, uint32_t color)
{
    color = 0xFF000000 | (color & 0xFFFFFF);
    int minX = x0, maxX = x0;
    int minY = y0, maxY = y0;
    if (x1 < minX) minX = x1; if (x1 > maxX) maxX = x1;
    if (x2 < minX) minX = x2; if (x2 > maxX) maxX = x2;
    if (y1 < minY) minY = y1; if (y1 > maxY) maxY = y1;
    if (y2 < minY) minY = y2; if (y2 > maxY) maxY = y2;
    if (minX < 0) minX = 0; if (maxX >= CD32_FB_W) maxX = CD32_FB_W - 1;
    if (minY < 0) minY = 0; if (maxY >= CD32_FB_H) maxY = CD32_FB_H - 1;
    int sa = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    for (int y = minY; y <= maxY; y++) {
        for (int x = minX; x <= maxX; x++) {
            int w0 = (x1 - x0) * (y - y0) - (y1 - y0) * (x - x0);
            int w1 = (x2 - x1) * (y - y1) - (y2 - y1) * (x - x1);
            int w2 = (x0 - x2) * (y - y2) - (y0 - y2) * (x - x2);
            if (sa >= 0) { if (w0 >= 0 && w1 >= 0 && w2 >= 0) cd32_video_putpixel(x, y, color); }
            else         { if (w0 <= 0 && w1 <= 0 && w2 <= 0) cd32_video_putpixel(x, y, color); }
        }
    }
}

void cd32_gpu_line(int x0, int y0, int x1, int y1, uint32_t color)
{
    color = 0xFF000000 | (color & 0xFFFFFF);
    int dx = x1 > x0 ? x1 - x0 : x0 - x1;
    int dy = y1 > y0 ? y1 - y0 : y0 - y1;
    int sx = x0 < x1 ? 1 : -1;
    int sy = y0 < y1 ? 1 : -1;
    int err = dx - dy;
    while (1) {
        cd32_video_putpixel(x0, y0, color);
        if (x0 == x1 && y0 == y1) break;
        int e2 = err * 2;
        if (e2 > -dy) { err -= dy; x0 += sx; }
        if (e2 < dx)  { err += dx; y0 += sy; }
    }
}

void cd32_gpu_present(void)
{
    cd32_video_kick();
    cd32_video_wait_vblank();
}
