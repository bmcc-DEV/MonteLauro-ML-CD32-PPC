# MonteLauro ML-CD32 Board Support Package

Este diretório contém o BSP (Board Support Package) para integrar o
MonteLauro ML-CD32 com o kernel AROS PPC.

## Estrutura

```
boards/
└── montelauro-cd32/
    ├── board.h          Definições canônicas do hardware (ABI v1.0)
    ├── kernel_cpu.c     Inicialização da CPU, MMU, IRQ, timer
    ├── console.c        Driver de framebuffer Lisa II TBDR
    ├── input.c          Driver de joypad via ColdFire mailbox
    ├── cdrom.c          Driver de CD-ROM via DMA + ISO9660
    ├── Makefile         Compila libmlcd32.a
    └── README.md
```

## Integração com a árvore AROS

Para compilar o AROS com suporte a MonteLauro:

1.  Copie `boards/montelauro-cd32/` para a árvore AROS em:
    `AROS/rom/boards/montelauro-cd32/`

2.  Crie um board config em `AROS/boards/montelauro-cd32/`:
    ```makefile
    # boards/montelauro-cd32/makefile
    CPU = ppc
    ARCH = sam440  # ou nova arch montelauro
    BOARD = montelauro-cd32
    ```

3.  Adicione ao kernel startup a chamada:
    ```c
    // rom/ppc/kernel_startup.cpp
    #include "boards/montelauro-cd32/board.h"
    extern void InitBoard(const MLCD32Platform *);
    // Chamar com r3 = ponteiro para CD32Platform
    ```

4.  Compile:
    ```bash
    make cpu=ppc board=montelauro-cd32
    ```

## Compilação standalone (para teste no emulador)

```bash
cd boards/montelauro-cd32/
make
# Produz libmlcd32.a com kernel_cpu.o, console.o, input.o, cdrom.o
```

## ABI

A struct `MLCD32Platform` e os endereços de hardware seguem a especificação
em `docs/aros/abi.md`. Headers C atualizados são gerados via:

```bash
make headers  # no raiz do projeto MonteLauro
```

## Licença

Este BSP é distribuído sob MIT License. Partes que venham a ser
incorporadas ao AROS podem precisar seguir a AROS Public License (APL).
