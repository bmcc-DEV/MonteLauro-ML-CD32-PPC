/*
 * MontêLauro CD+G² — Input (joypad via ColdFire mailbox)
 *
 * Lê GPIO do ColdFire no offset 0x20 via mailbox CF_CMD_IO_READ (0x02).
 * GPIO bits: 0=UP, 1=DN, 2=L, 3=R, 4=A, 5=B, 6=START, 7=SEL
 */

#include "cd32.h"

#define MAILBOX_CMD  (*(volatile uint32_t*)(CD32_MAILBOX + 0x00))
#define MAILBOX_RESP (*(volatile uint32_t*)(CD32_MAILBOX + 0x04))
#define MAILBOX_STAT (*(volatile uint32_t*)(CD32_MAILBOX + 0x08))
#define MAILBOX_ARG  (*(volatile uint32_t*)(CD32_MAILBOX + 0x0C))

#define CF_CMD_IO_READ 0x02

static uint16_t state = 0, prev = 0, changed = 0;

static uint16_t mailbox_io_read(uint32_t offset)
{
    while (MAILBOX_STAT != 0) {}
    MAILBOX_ARG = offset;
    MAILBOX_CMD = CF_CMD_IO_READ;
    MAILBOX_STAT = 1;
    while (MAILBOX_STAT != 0) {}
    return (uint16_t)MAILBOX_RESP;
}

void cd32_input_init(void)
{
    state = prev = changed = 0;
}

void cd32_input_poll(void)
{
    prev = state;
    state = ~mailbox_io_read(0x20);  /* GPIO active-low */
    changed = state ^ prev;
}

uint16_t cd32_joypad_read(void) { return state; }
uint16_t cd32_joypad_pressed(int btn) { return state & btn; }
uint16_t cd32_joypad_just_pressed(int btn) { return state & changed & btn; }
