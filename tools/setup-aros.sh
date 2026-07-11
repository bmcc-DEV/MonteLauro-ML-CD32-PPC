#!/bin/sh
# setup-aros.sh — Integra o BSP CDG2 na árvore AROS
# Uso: AROS=/caminho/para/AROS ./tools/setup-aros.sh

set -e

if [ -z "$AROS" ]; then
    echo "ERRO: Defina AROS apontando para a raiz da árvore AROS"
    echo "Uso: AROS=/path/to/AROS $0"
    exit 1
fi

BOARD_DIR="$AROS/rom/boards/cdg2"
BOARD_MAKE="$AROS/boards/cdg2"
BSP_SRC="boards/cdg2"

echo "==> Integrando BSP CDG² em $AROS"

# 1. Copiar BSP
mkdir -p "$BOARD_DIR"
cp -v $BSP_SRC/*.c $BSP_SRC/*.h $BSP_SRC/Makefile "$BOARD_DIR/"
echo "  BSP copiado para $BOARD_DIR"

# 2. Criar board config
mkdir -p "$BOARD_MAKE"
cat > "$BOARD_MAKE/makefile" << 'MAKEEOF'
# CDG2 CDG2 board config para AROS
CPU = ppc
ARCH = sam440
BOARD = cdg2
MAKEEOF
echo "  Board config criado em $BOARD_MAKE/makefile"

# 3. Patch kernel_startup.cpp
KSTARTUP="$AROS/rom/ppc/kernel_startup.cpp"
if [ -f "$KSTARTUP" ]; then
    if ! grep -q "cdg2" "$KSTARTUP" 2>/dev/null; then
        echo "  ATENCAO: Patch manual necessario em $KSTARTUP"
        echo "    Adicione ao inicio de kernel_init():"
        echo '    #include "boards/cdg2/board.h"'
        echo '    extern void InitBoard(const CDG2Platform *);'
        echo '    InitBoard((const CDG2Platform *)r3);'
    else
        echo "  kernel_startup.cpp ja contem referencia ao CDG2"
    fi
else
    echo "  AVISO: $KSTARTUP nao encontrado. Patch manual necessario."
fi

# 4. Instalar dependencias Python
python3 -c "import mako" 2>/dev/null || pip install --break-system-packages mako 2>/dev/null

# 5. Configurar AROS (se ainda nao configurado)
if [ ! -f "$AROS/build/ppc/Makefile" ]; then
    echo "  Configurando AROS para PPC..."
    mkdir -p "$AROS/build/ppc"
    (cd "$AROS/build/ppc" && ../../configure --target=sam440-ppc --with-aros-toolchain="$AROS" --enable-crosstools 2>&1) || {
        echo "  AVISO: configure falhou. Execute manualmente:"
        echo "    mkdir -p $AROS/build/ppc && cd $AROS/build/ppc"
        echo "    $AROS/configure --target=sam440-ppc --with-aros-toolchain=$AROS --enable-crosstools"
    }
fi

# 5. Verificar toolchain
TOOLCHAIN=$(command -v powerpc-elf-gcc || echo "")
if [ -z "$TOOLCHAIN" ]; then
    echo "  AVISO: powerpc-elf-gcc nao encontrado no PATH"
    echo "  Para construir o toolchain AROS PPC:"
    echo "    sudo apt-get install automake autoconf libtool flex bison"
    echo "    make -C $AROS/tools cross-toolchain cpu=ppc"
    echo "  Ou baixe um toolchain pre-compilado de www.aros.org"
fi

echo ""
echo "==> Setup completo!"
echo "Para compilar: make -C $AROS cpu=ppc board=cdg2"
echo "Para testar no emulador: make rom-aros KERNEL=$AROS/bin/ppc/aros-ppc.bin"
