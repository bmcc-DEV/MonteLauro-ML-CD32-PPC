#include "cd32.h"
#include "cd32_pad.h"

static cd32_pad_state_t pad_state;

void cd32_pad_init(void) {
    cd32_input_init();
    pad_state.current = 0;
    pad_state.previous = 0;
    pad_state.pressed = 0;
    pad_state.released = 0;
}

void cd32_pad_update(void) {
    pad_state.previous = pad_state.current;
    pad_state.current = cd32_joypad_read();
    pad_state.pressed = pad_state.current & ~pad_state.previous;
    pad_state.released = pad_state.previous & ~pad_state.current;
}

const cd32_pad_state_t *cd32_pad_get(int port) {
    (void)port;
    return &pad_state;
}
