# CDG2 CDG2 Board Support Package

Este diretório contém o BSP (Board Support Package) para integrar o
CDG2 CDG2 com o kernel AROS PPC.

## Estrutura

```
boards/
└── cdg2/
    ├── board.h          Definições canônicas do hardware (ABI v1.0)
    ├── kernel_cpu.c     Inicialização da CPU, MMU, IRQ, timer
    ├── console.c        Driver de framebuffer Lisa II TBDR
    ├── input.c          Driver de joypad via ColdFire mailbox
    ├── cdrom.c          Driver de CD-ROM via DMA + ISO9660
    ├── Makefile         Compila libmlcd32.a
    └── README.md
```

## Integração com a árvore AROS

Para compilar o AROS com suporte a CDG2:

1.  Copie `boards/cdg2/` para a árvore AROS em:
    `AROS/rom/boards/cdg2/`

2.  Crie um board config em `AROS/boards/cdg2/`:
    ```makefile
    # boards/cdg2/makefile
    CPU = ppc
    ARCH = sam440  # ou nova arch cdg2
    BOARD = cdg2
    ```

3.  Adicione ao kernel startup a chamada:
    ```c
    // rom/ppc/kernel_startup.cpp
    #include "boards/cdg2/board.h"
    extern void InitBoard(const CDG2Platform *);
    // Chamar com r3 = ponteiro para CD32Platform
    ```

4.  Compile:
    ```bash
    make cpu=ppc board=cdg2
    ```

## Compilação standalone (para teste no emulador)

```bash
cd boards/cdg2/
make
# Produz libmlcd32.a com kernel_cpu.o, console.o, input.o, cdrom.o
```

## ABI

A struct `CDG2Platform` e os endereços de hardware seguem a especificação
em `docs/aros/abi.md`. Headers C atualizados são gerados via:

```bash
make headers  # no raiz do projeto CDG2
```

## Licença

Este BSP é distribuído sob MIT License. Partes que venham a ser
incorporadas ao AROS podem precisar seguir a AROS Public License (APL).
