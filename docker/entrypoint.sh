#!/bin/sh
# Entrypoint for montelauro-toolchain container.
# Copies AROS PPC kernel to mounted /build directory.

KERNEL_SRC="/aros/kernel/aros-ppc.bin"
KERNEL_DST="/build/aros-ppc.bin"

if [ -f "$KERNEL_SRC" ]; then
    cp "$KERNEL_SRC" "$KERNEL_DST"
    echo "=== MonteLauro CD32 AROS Toolchain ==="
    echo "Kernel: $KERNEL_DST"
    ls -lh "$KERNEL_DST"
else
    echo "=== MonteLauro CD32 AROS Toolchain ==="
    echo "AVISO: kernel AROS PPC nao encontrado em $KERNEL_SRC"
    echo "O build do AROS pode nao ter completado."
    ls -lh /aros/toolchain/linux-x86_64/ppc-elf/bin/ 2>/dev/null && \
        echo "Toolchain disponivel em /aros/toolchain"
fi
