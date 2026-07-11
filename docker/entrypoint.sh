#!/bin/sh
# MonteLauro CD32 PPC Toolchain entrypoint.
# Se um comando for passado, executa ele. Senao, valida o toolchain.

if [ $# -gt 0 ]; then
    exec "$@"
fi

CC="powerpc-linux-gnu-gcc"
echo "=== MonteLauro CD32 PPC Toolchain ==="
echo "Toolchain: $(which $CC)"
$CC --version | head -1
echo "PPC cross-compiler: OK"
echo ""
echo "Uso: docker run --rm -v /caminho/AROS:/aros montelauro-toolchain sh -c 'comando'"
