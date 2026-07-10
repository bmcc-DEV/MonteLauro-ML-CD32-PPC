#!/bin/sh
# setup-aros.sh — Integra o BSP MonteLauro na árvore AROS
# Uso: AROS=/caminho/para/AROS ./tools/setup-aros.sh

set -e

if [ -z "$AROS" ]; then
    echo "ERRO: Defina AROS apontando para a raiz da árvore AROS"
    echo "Uso: AROS=/path/to/AROS $0"
    exit 1
fi

BOARD_DIR="$AROS/rom/boards/montelauro-cd32"
BOARD_MAKE="$AROS/boards/montelauro-cd32"
BSP_SRC="boards/montelauro-cd32"

echo "==> Integrando BSP MonteLauro CD32 em $AROS"

# 1. Copiar BSP
mkdir -p "$BOARD_DIR"
cp -v $BSP_SRC/*.c $BSP_SRC/*.h $BSP_SRC/Makefile "$BOARD_DIR/"
echo "  BSP copiado para $BOARD_DIR"

# 2. Criar board config
mkdir -p "$BOARD_MAKE"
cat > "$BOARD_MAKE/makefile" << 'MAKEEOF'
# MonteLauro ML-CD32 board config para AROS
CPU = ppc
ARCH = sam440
BOARD = montelauro-cd32
MAKEEOF
echo "  Board config criado em $BOARD_MAKE/makefile"

# 3. Patch kernel_startup.cpp
KSTARTUP="$AROS/rom/ppc/kernel_startup.cpp"
if [ -f "$KSTARTUP" ]; then
    if ! grep -q "montelauro-cd32" "$KSTARTUP" 2>/dev/null; then
        echo "  ATENCAO: Patch manual necessario em $KSTARTUP"
        echo "    Adicione ao inicio de kernel_init():"
        echo '    #include "boards/montelauro-cd32/board.h"'
        echo '    extern void InitBoard(const MLCD32Platform *);'
        echo '    InitBoard((const MLCD32Platform *)r3);'
    else
        echo "  kernel_startup.cpp ja contem referencia ao MonteLauro"
    fi
else
    echo "  AVISO: $KSTARTUP nao encontrado. Patch manual necessario."
fi

# 4. Verificar toolchain
TOOLCHAIN=$(command -v powerpc-elf-gcc || echo "")
if [ -z "$TOOLCHAIN" ]; then
    echo "  AVISO: powerpc-elf-gcc nao encontrado no PATH"
    echo "  Instale o toolchain AROS PPC primeiro:"
    echo "    make -C $AROS toolchain"
fi

echo ""
echo "==> Setup completo!"
echo "Para compilar: make -C $AROS cpu=ppc board=montelauro-cd32"
echo "Para testar no emulador: make rom-aros KERNEL=$AROS/bin/ppc/aros-ppc.bin"
