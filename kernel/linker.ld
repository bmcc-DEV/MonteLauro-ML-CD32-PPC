/* CDG² — Linker Script para jogos */
OUTPUT_FORMAT(elf32-powerpc)
OUTPUT_ARCH(powerpc:common)
ENTRY(game_main)

MEMORY
{
    sysram (rwx) : ORIGIN = 0x00002000, LENGTH = 16M - 8K
}

SECTIONS
{
    .text : { *(.text*) } > sysram
    .rodata : { *(.rodata*) } > sysram
    .data : { *(.data*) } > sysram
    .bss : { *(.bss*) } > sysram
}

/* SDA base para acesso a variaveis globais sem TOC */
_SDA_BASE_ = 0x00004000;
_SDA2_BASE_ = 0x00004000;
