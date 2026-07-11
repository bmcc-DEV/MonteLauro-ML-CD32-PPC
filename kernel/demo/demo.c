/*
 * MonteLauro CD³² — Demo minimo de jogo
 *
 * Compilar: make demo
 * Executar no emulador: --bios rom/game_cd32.rom --disc rom/jogo.iso
 *
 * Mostra gradiente animado + contador de frames + estado do joypad.
 */

#include "cd32.h"

static uint32_t rng_state = 42;
static uint32_t rand(void) {
    rng_state = rng_state * 1103515245 + 12345;
    return rng_state;
}

void game_main(void)
{
    int frame = 0;
    uint32_t colors[] = {0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0xFFFFFF};

    cd32_video_init();
    cd32_input_init();

    while (1) {
        cd32_input_poll();
        uint16_t joy = cd32_joypad_read();

        /* Clear */
        cd32_video_clear(0x00082040);

        /* Draw colored rectangles */
        for (int i = 0; i < 7; i++) {
            int x = ((frame * 3 + i * 50) * 7) % 580 + 10;
            int y = ((frame * 7 + i * 30) * 13) % 420 + 10;
            cd32_video_rect(x, y, 40 + i * 5, 30 + i * 3, colors[i]);
        }

        /* Draw a box around pressed buttons */
        cd32_video_rect(10, 440, 100, 30, 0x000000);
        cd32_printf("Frame: %d\n", frame);

        if (joy & CD32_JOY_A)     cd32_video_rect(150, 440, 80, 20, 0xFF0000);
        if (joy & CD32_JOY_B)     cd32_video_rect(240, 440, 80, 20, 0x00FF00);
        if (joy & CD32_JOY_START) cd32_video_rect(330, 440, 80, 20, 0x0000FF);
        if (joy & CD32_JOY_UP)    cd32_printf(" UP");
        if (joy & CD32_JOY_DOWN)  cd32_printf(" DOWN");
        if (joy & CD32_JOY_LEFT)  cd32_printf(" LEFT");
        if (joy & CD32_JOY_RIGHT) cd32_printf(" RIGHT");

        cd32_video_kick();
        cd32_video_wait_vblank();
        frame++;
        if (frame > 9999) frame = 0;
    }
}
