#!/bin/sh
# build-aros.sh — Builda o kernel AROS PPC para MonteLauro CD32
#
# Uso: ./tools/build-aros.sh [caminho/para/AROS]
#
# Requer:
#   - Docker instalado
#   - ~8GB de RAM disponivel
#   - ~5GB de disco
#   - Conexao de rede estavel (para baixar binutils/gcc)

set -e

AROS="${1:-/home/bruno/Documentos/AROS}"
IMAGE="montelauro-toolchain"

echo "=== MonteLauro CD32 — AROS PPC Kernel Build ==="

# 1. Verificar Docker
docker info >/dev/null 2>&1 || { echo "ERRO: Docker nao instalado"; exit 1; }

# 2. Clonar AROS se necessario
if [ ! -f "$AROS/configure" ]; then
    echo "==> Clonando AROS em $AROS..."
    git clone --depth=1 https://github.com/aros-development-team/AROS.git "$AROS"
fi

# 3. Buildar imagem Docker (toolchain + depends)
echo "==> Buildando imagem Docker..."
make docker-build

# 4. Configurar AROS
echo "==> Configurando AROS para PPC..."
docker run --rm -v "$AROS:/aros" "$IMAGE" sh -c "
    rm -rf /aros/build/ppc 2>/dev/null
    mkdir -p /aros/build/ppc
    cd /aros/build/ppc
    /aros/configure --target=sam440-ppc --enable-crosstools
" 2>&1 | grep -E "error|complete|required|falhou|AVISO"

# 5. Buildar
echo "==> Buildando AROS (isto leva ~2h)..."
echo "    Para acompanhar: tail -f /tmp/aros-build.log"
echo "    O make e retomavel — pode interromper com Ctrl+C e continuar depois."
echo ""
nohup docker run --rm -v "$AROS:/aros" -m 8g "$IMAGE" sh -c "
    cd /aros/build/ppc && make -j2
" > /tmp/aros-build.log 2>&1 &
BUILD_PID=$!

echo "Build iniciado (PID $BUILD_PID)"
echo "Log: /tmp/aros-build.log"

# 6. Aguardar e verificar resultado
echo ""
echo "Aguardando conclusao..."
wait $BUILD_PID 2>/dev/null || true

if [ -f "$AROS/build/ppc/bin/sam440-ppc/aros-ppc.bin" ]; then
    cp "$AROS/build/ppc/bin/sam440-ppc/aros-ppc.bin" .
    echo "=== KERNEL AROS COMPILADO ==="
    echo "Arquivo: aros-ppc.bin"
    echo "Comando: make rom-aros KERNEL=aros-ppc.bin && make test-aros"
else
    echo "=== KERNEL NAO COMPILADO ==="
    echo "O build do AROS pode ter falhado. Verifique:"
    echo "  tail -50 /tmp/aros-build.log | grep error"
    echo ""
    echo "Causas comuns:"
    echo "  - Rede instavel durante download de binutils/gcc (CTRL+C e tente de novo)"
    echo "  - RAM insuficiente (use -m 8g ou mais)"
    echo "  - Dependencias faltando no container (veja docker/Dockerfile)"
    echo ""
    echo "Se o problema persistir, baixe um aros-ppc.bin pre-compilado de:"
    echo "  https://github.com/aros-development-team/AROS/releases"
fi
