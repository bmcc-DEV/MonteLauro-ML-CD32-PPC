#ifndef CD32_PAD_H
#define CD32_PAD_H

#include <stdint.h>

typedef struct {
    uint16_t current;
    uint16_t previous;
    uint16_t pressed;
    uint16_t released;
} cd32_pad_state_t;

void cd32_pad_init(void);
void cd32_pad_update(void);
const cd32_pad_state_t *cd32_pad_get(int port);

#define CD32_BTN_HELD(pad, btn)     (((pad)->current & (btn)) != 0)
#define CD32_BTN_PRESSED(pad, btn)  (((pad)->pressed & (btn)) != 0)
#define CD32_BTN_RELEASED(pad, btn) (((pad)->released & (btn)) != 0)

#endif
