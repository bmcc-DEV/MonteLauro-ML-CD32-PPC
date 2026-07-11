/*
 * MontêLauro CD+G² — Tech Demo
 *
 * Compilar: make demo DEMO=tech_demo
 * Executa no emulador: cargo run --release -- --bios rom/game_cd32.rom --disc rom/jogo.iso --sdl
 *
 * Demonstra:
 *  - TBDR GPU "Lisa II": triângulos, retângulos, linhas
 *  - Efeitos de cor (HSV ciclo, plasma)
 *  - Objeto 3D wireframe rotacionando (cubo)
 *  - Áudio: 2 canais com tom
 *  - Input: joypad + botões
 *  - Texto via cd32_printf (font 8x16)
 */

#include "cd32.h"
#include <stdint.h>

/* ── Simple LCG PRNG (Park-Miller minimal) ─────────────────────── */
static uint32_t prng_state = 0xDEADBEEF;
static inline uint32_t prng_next(void) {
    prng_state = prng_state * 1664525 + 1013904223;
    return prng_state;
}
#define rand() prng_next()

/* ── Matemática fixa Q16.16 ────────────────────────────────────── */
typedef int32_t fix16_t;
#define FIX16_ONE   0x00010000
#define FIX16_PI    0x0003243F
#define FIX16_HALF_PI 0x0001921F

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

static int32_t sin_table[256];

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }
    return (0xFF << 24) | (r << 16) | (g << 8) | b;
}

/* ── Cubo 3D wireframe ─────────────────────────────────────────── */
static const int16_t cube_verts[8][3] = {
    {-50,-50,-50}, {50,-50,-50}, {50,50,-50}, {-50,50,-50},
    {-50,-50,50},  {50,-50,50},  {50,50,50},  {-50,50,50}
};
static const uint8_t cube_edges[12][2] = {
    {0,1},{1,2},{2,3},{3,0},
    {4,5},{5,6},{6,7},{7,4},
    {0,4},{1,5},{2,6},{3,7}
};

static void rotate_point(int16_t out[3], const int16_t in[3], int32_t ax, int32_t ay, int32_t az) {
    int32_t x = in[0] << 16, y = in[1] << 16, z = in[2] << 16;
    int32_t cx = fix16_cos(ax), sx = fix16_sin(ax);
    int32_t cy = fix16_cos(ay), sy = fix16_sin(ay);
    int32_t cz = fix16_cos(az), sz = fix16_sin(az);

    int32_t y1 = fix16_mul(cx, y) - fix16_mul(sx, z);
    int32_t z1 = fix16_mul(sx, y) + fix16_mul(cx, z);
    int32_t x2 = fix16_mul(cy, x) + fix16_mul(sy, z1);
    int32_t z2 = -fix16_mul(sy, x) + fix16_mul(cy, z1);
    int32_t x3 = fix16_mul(cz, x2) - fix16_mul(sz, y1);
    int32_t y3 = fix16_mul(sz, x2) + fix16_mul(cz, y1);

    out[0] = (int16_t)(x3 >> 16);
    out[1] = (int16_t)(y3 >> 16);
    out[2] = (int16_t)(z2 >> 16);
}

static void project(int16_t* sx, int16_t* sy, const int16_t p[3]) {
    int32_t z = p[2] + 256;
    if (z < 1) z = 1;
    *sx = (int16_t)(((int32_t)p[0] * 256) / z + 320);
    *sy = (int16_t)(((int32_t)p[1] * 256) / z + 240);
}

/* Audio tones */
static void play_note(int ch, int freq) {
    static int16_t tone_buf[256];
    int period = 44100 / freq;
    int half = period / 2;
    for (int i = 0; i < 256; i++) {
        tone_buf[i] = ((i % (period/2)) < (period/4)) ? 10000 : -10000;
    }
    cd32_audio_play(ch, tone_buf, 256, 1);
}

/* HSV frame counter */
static uint16_t frame = 0;
static uint8_t hue = 0;
static int32_t sin_table[256];

static void init_sin_table(void) {
    for (int i = 0; i < 256; i++) {
        int32_t x = (i * 0x6487F) >> 8;
        int32_t x2 = ((int64_t)x * x) >> 16;
        int32_t x3 = ((int64_t)x * x * x) >> 32;
        int32_t s = x - (fix16_mul(x, fix16_mul(x, x)) / 6);
        sin_table[i] = s;
    }
}

static inline int32_t fix16_sin(int32_t x) {
    uint8_t idx = (uint32_t)(x >> 8) & 0xFF;
    return sin_table[idx];
}

static inline int32_t fix16_cos(int32_t x) {
    return fix16_sin(x + 0x0001921F); /* FIX16_HALF_PI */
}

static inline int32_t fix16_mul(int32_t a, int32_t b) {
    return (int32_t)(((int64_t)a * b) >> 16);
}

/* HSV → RGB (0-255 cada) */
static uint32_t hsv2rgb(uint8_t h, uint8_t s, uint8_t v) {
    uint8_t r, g, b;
    uint8_t region = h / 43;
    uint8_t remainder = (h - region * 43) * 6;
    uint8_t p = (v * (255 - s)) >> 8;
    uint8_t q = (v * (255 - ((s * remainder) >> 8))) >> 8;
    uint8_t t = (v * (255 - ((s * (255 - remainder)) >> 8))) >> 8;
    switch (h / 43) {
        case 0: r = v; g = t; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = t; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = t; g = p; b = v; break;
        default: r = v; g = p; b = p; break;
    }