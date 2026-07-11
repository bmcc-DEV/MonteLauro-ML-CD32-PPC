#!/bin/sh
# build-aros.sh — Obtem o kernel AROS PPC para MontêLauro CD+G²
#
# Tenta, em ordem:
#   1. Usar kernel ja compilado em /tmp/AROS (build anterior)
#   2. Baixar pre-compilado do AROS Nightly Builds
#   3. Buildar do source (via Docker, ~2h, requer rede estavel)
#
# Uso: ./tools/build-aros.sh

set -e

KERNEL_DST="aros-ppc.bin"

echo "=== MontêLauro CD+G² — AROS PPC Kernel ==="

# ── Opcao 1: Kernel ja compilado local ─────────────────────────────
if [ -f "/tmp/AROS/build/ppc/bin/sam440-ppc/aros-ppc.bin" ]; then
    cp /tmp/AROS/build/ppc/bin/sam440-ppc/aros-ppc.bin "$KERNEL_DST"
    echo "Kernel copiado de /tmp/AROS (build anterior)"
    echo "Comando: make rom-aros KERNEL=$KERNEL_DST"
    exit 0
fi

# ── Opcao 2: Download pre-compilado ────────────────────────────────
echo "Procurando binary pre-compilado..."
for URL in \
    "https://www.aros.org/nightly1/sam440-ppc/aros-ppc.bin" \
    "https://github.com/aros-development-team/AROS/releases/download/nightly/sam440-ppc-aros-ppc.bin"; do
    echo "  Tentando $URL ..."
    if wget -q -O "$KERNEL_DST" "$URL" 2>/dev/null; then
        echo "Kernel baixado de $URL"
        ls -lh "$KERNEL_DST"
        echo "Comando: make rom-aros KERNEL=$KERNEL_DST"
        exit 0
    fi
done

# ── Opcao 3: Build do source ───────────────────────────────────────
echo "Nenhum binary pre-compilado encontrado."
echo "Buildando AROS do source (isto leva ~2h)..."
echo ""

AROS="/home/bruno/Documentos/AROS"
if [ ! -f "$AROS/configure" ]; then
    echo "Clonando AROS..."
    git clone --depth=1 https://github.com/aros-development-team/AROS.git "$AROS"
fi

echo "Buildando imagem Docker..."
make docker-build

echo "Configurando AROS..."
docker run --rm -v "$AROS:/aros" ml-gd2-toolchain sh -c "
    rm -rf /aros/build/ppc 2>/dev/null
    mkdir -p /aros/build/ppc
    cd /aros/build/ppc
    /aros/configure --target=sam440-ppc --enable-crosstools
"

echo "Buildando (make -j2)..."
echo "  Log: tail -f /tmp/aros-build.log"
docker run --rm -v "$AROS:/aros" -m 8g ml-gd2-toolchain \
    sh -c "cd /aros/build/ppc && make -j2" > /tmp/aros-build.log 2>&1 &

echo "Build em background (PID $!)"
echo "Acompanhe com: tail -f /tmp/aros-build.log"
echo ""
echo "Se o build falhar por download de binutils/gcc, tente:"
echo "  1. docker exec -it $(docker ps -lq) sh"
echo "  2. wget http://ftp.gnu.org/gnu/binutils/binutils-2.32.tar.xz -P /aros/build/ppc/bin/Sources/"
echo "E depois: make -j2"
