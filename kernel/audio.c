/*
 * MontêLauro CD+G² — Áudio (DSP + DMA)
 *
 * 8 canais estéreo, 16-bit, 44.1kHz.
 * Cada canal tem FIFO de 2KB, preenchido via DMA da Chip RAM.
 */

#include "cd32.h"

#define DSP_CTRL (*(volatile uint32_t*)(CD32_DSP_BASE + 0x00))
#define DSP_VOL  (*(volatile uint32_t*)(CD32_DSP_BASE + 0x04))

void cd32_audio_init(void)
{
    DSP_CTRL = 0xFF;       /* Habilita todos os 8 canais */
    DSP_VOL  = 1024;       /* Volume master maximo */
}

void cd32_audio_play(int ch, int16_t *samples, int count, int loop)
{
    if (ch < 0 || ch >= CD32_AUDIO_CHANNELS) return;
    /* Stub: agenda transferencia DMA do buffer para o FIFO do canal */
    /* Na implementacao real: DMA chan 2 (Audio) da Chip RAM pro DSP */
    (void)samples; (void)count; (void)loop;
}

void cd32_audio_stop(int ch)
{
    if (ch < 0 || ch >= CD32_AUDIO_CHANNELS) return;
    DSP_CTRL &= ~(1 << ch);
}

void cd32_audio_volume(int ch, int vol)
{
    if (ch < 0 || ch >= CD32_AUDIO_CHANNELS || vol < 0) return;
    (void)ch; (void)vol;
    /* Volume por canal via registers DSP (a implementar) */
}

void cd32_audio_pan(int ch, int pan)
{
    if (ch < 0 || ch >= CD32_AUDIO_CHANNELS || pan > 255) return;
    (void)ch; (void)pan;
}
