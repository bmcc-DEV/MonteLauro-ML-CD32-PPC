# MonteLauro CD3² — Game Runtime Makefile
# Uso: make [CC=powerpc-linux-gnu-gcc] [all|lib|clean]

CC      ?= powerpc-linux-gnu-gcc
AR      ?= powerpc-linux-gnu-ar
CFLAGS  ?= -O2 -mcpu=603e -mhard-float -ffreestanding -nostdlib \
           -I../include -I.
LDFLAGS ?= -nostdlib -ffreestanding -Wl,-T,linker.ld

OBJS = kernel.o video.o input.o audio.o cdrom.o dma.o

all: libcd32.a

libcd32.a: $(OBJS)
	$(AR) rcs $@ $^
	@echo "libcd32.a built ($(words $(OBJS)) objects)"

%.o: %.c ../include/cd32.h
	$(CC) $(CFLAGS) -c -o $@ $<

# Exemplo minimo de jogo (opcional)
demo: libcd32.a demo/demo.c
	$(CC) $(CFLAGS) $(LDFLAGS) demo/demo.c -L. -lcd32 -o demo.elf
	@echo "demo.elf built"

clean:
	rm -f *.o *.a *.elf

.PHONY: all lib demo clean
