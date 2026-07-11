/*
 * MonteLauro CD3² — Demo minimo de jogo
 *
 * Compilar: make demo
 * Executar: carregar demo.ELF via cd32_cdrom_load() ou
 *           substituir o kernel.c:_start() pelo entry point direto.
 *
 * Mostra:
 *   - Desenho de retangulos na GPU Lisa II
 *   - Leitura de joypad
 *   - Texto formatado
 */

#include "cd32.h"

/* Simple PRNG for colors */
static uint32_t rng_state = 42;
static uint32_t rng(void) {
    rng_state = rng_state * 1103515245 + 12345;
    return rng_state;
}

void game_main(void)
{
    int frame = 0;

    cd32_video_init();

    while (1) {
        cd32_input_poll();
        uint16_t joy = cd32_joypad_read();

        /* Clear screen with a color based on frame */
        cd32_video_clear((frame * 16) << 16 | (frame * 8) << 8 | frame * 4);

        /* Draw some rectangles based on joypad */
        for (int i = 0; i < 5; i++) {
            int x = (joy * 3 + i * 50 + frame * 2) % 600;
            int y = (joy * 7 + i * 30 + frame * 3) % 440;
            cd32_video_rect(x, y, 30 + i * 5, 20 + i * 3, rng());
        }

        /* Print frame counter and joypad state */
        cd32_video_rect(200, 220, 240, 30, 0x00000000);
        cd32_printf("Frame: %d  Joy: 0x%04X\n", frame++, joy);

        cd32_video_kick();
        cd32_video_wait_vblank();
    }
}
