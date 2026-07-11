/* MonteLauro CD3² — Linker Script para jogos */
OUTPUT_FORMAT(elf32-powerpc)
OUTPUT_ARCH(powerpc:common)
ENTRY(game_main)

MEMORY
{
    sysram (rwx) : ORIGIN = 0x00010000, LENGTH = 16M - 64K
}

SECTIONS
{
    .text : { *(.text*) } > sysram
    .rodata : { *(.rodata*) } > sysram
    .data : { *(.data*) } > sysram
    .bss : { *(.bss*) } > sysram
}
