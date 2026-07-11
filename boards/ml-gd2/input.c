/*
 * CDG2 CDG2 BSP — input.c
 * Driver de input via ColdFire GPIO (joypad) + mailbox.
 *
 * O estado do joypad é lido do GPIO do ColdFire no offset 0x20.
 * A leitura é feita via mailbox (CF_CMD_IO_READ), que aciona
 * o ColdFire para samplear o GPIO e retornar o valor.
 */

#include <exec/types.h>
#include <exec/interrupts.h>
#include <hardware/intbits.h>

#include "board.h"

/* ── Estado do joypad ────────────────────────────────────────────── */

static uint16_t joypad_state = 0;
static uint16_t joypad_changed = 0;
static uint16_t joypad_prev = 0;

/* ── Inicialização ───────────────────────────────────────────────── */

void ml_input_init(void)
{
    joypad_state = 0;
    joypad_changed = 0;
    joypad_prev = 0;
}

/* ── Poll do joypad via ColdFire mailbox ────────────────────────────
 *
 * Envia comando CF_CMD_IO_READ com offset = 0x20 (GPIO).
 * O ColdFire responde com o valor de 32 bits do GPIO (apenas bits 0-7 usados).
 *
 * Mapeamento de bits:
 *   0=UP, 1=DOWN, 2=LEFT, 3=RIGHT, 4=A, 5=B, 6=START, 7=SELECT
 */

void ml_input_poll(void)
{
    joypad_prev = joypad_state;

    ml_mailbox_send(ML_CF_CMD_IO_READ, 0x20);
    joypad_state = ~ml_mailbox_recv();  /* GPIO é active-low */

    joypad_changed = joypad_state ^ joypad_prev;
}

/* ── Leitura de estado ────────────────────────────────────────────── */

uint16_t ml_joypad_read(void)
{
    return joypad_state;
}

uint16_t ml_joypad_changed(void)
{
    return joypad_changed;
}

int ml_joypad_pressed(int button)
{
    return (joypad_state & button) != 0;
}

int ml_joypad_just_pressed(int button)
{
    return (joypad_state & button) && (joypad_changed & button);
}

/* ── Mapeamento para input.device AROS ─────────────────────────────
 *
 * Integração com o subsistema de input do AROS:
 *   - ml_input_poll() deve ser chamado a cada VBlank
 *   - Os eventos são convertidos para IECLASS_* e enviados
 *     ao input.device
 *
 * Exemplo de conversão (para integrar no task de input):
 *
 *   void ml_input_dispatch(void) {
 *       ml_input_poll();
 *       if (ml_joypad_just_pressed(ML_JOY_A))
 *           // Envia IECLASS_RAWKEY com código de A
 *   }
 */
