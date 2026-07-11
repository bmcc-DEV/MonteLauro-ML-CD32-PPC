/*
 * MonteLauro CD3² — Kernel mínimo para jogos
 *
 * Entry point chamado pelo bootstrap PPC com:
 *   r3 = &CD32Platform (struct na Chip RAM)
 *
 * Init hardware → carrega jogo do CD → executa.
 * Se não houver CD, entra em loop com tela azul.
 */

#include "cd32.h"

extern void _init(void);

// Ponteiro para a struct de plataforma (passada pelo bootstrap)
static volatile uint32_t *platform = (uint32_t*)0x0000FC00;

void _start(void)
{
    // ── Setup básico ──────────────────────────────────────────────
    // BATs, stack, MSR já configurados pelo bootstrap PPC

    // ── Init hardware ─────────────────────────────────────────────
    cd32_video_init();

    // Banner de boot
    cd32_video_clear(0x00000000);
    cd32_printf("MonteLauro CD3²  v1.0\n");
    cd32_printf("PPC603e @ 266MHz  |  ColdFire V4e @ 140MHz\n");
    cd32_printf("20MB RAM | 8MB VRAM | GPU Lisa II TBDR\n\n");

    cd32_audio_init();
    cd32_input_init();

    // ── Tentar carregar jogo do CD ───────────────────────────────
    cd32_printf("CD-ROM: ");
    if (cd32_cdrom_init() == 0) {
        cd32_printf("disco presente, procurando jogo...\n");

        void *entry = cd32_cdrom_load("cd0:game.elf");
        if (entry != NULL) {
            cd32_printf("\nIniciando jogo em 0x%08X...\n", (uint32_t)entry);
            cd32_video_clear(0x00000000);

            // Passa controle para o jogo
            void (*game_main)(void) = entry;
            game_main();

            // Se o jogo retornar, halt
            cd32_printf("Jogo encerrado. Halt.\n");
        } else {
            cd32_printf("nenhum jogo encontrado.\n");
        }
    } else {
        cd32_printf("sem disco.\n");
    }

    // ── Loop de idle ─────────────────────────────────────────────
    cd32_printf("\nInsira um CD com jogo e reinicie.\n");
    while (1) {
        cd32_input_poll();
        uint16_t joy = cd32_joypad_read();
        if (joy) {
            // Qualquer botao = reboot via ColdFire watchdog
            // (placeholder — por ora so halt)
        }
    }
}
