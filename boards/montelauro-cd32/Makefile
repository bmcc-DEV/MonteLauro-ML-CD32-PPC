# MonteLauro ML-CD32 Board Support Package
# Makefile para integração com o build system do AROS
#
# Uso (dentro da árvore AROS):
#   make cpu=ppc arch=sam440 board=montelauro-cd32
#
# Ou compile manualmente com:
#   powerpc-elf-gcc -c -O2 -mcpu=603e -mhard-float -I. \
#       kernel_cpu.c console.c input.c cdrom.c

CC      = powerpc-elf-gcc
CFLAGS  = -O2 -mcpu=603e -mhard-float -ffreestanding -I.
AR      = powerpc-elf-ar

OBJS    = kernel_cpu.o console.o input.o cdrom.o

all: libmlcd32.a

libmlcd32.a: $(OBJS)
	$(AR) rcs $@ $^

%.o: %.c board.h
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	rm -f *.o *.a

.PHONY: all clean
