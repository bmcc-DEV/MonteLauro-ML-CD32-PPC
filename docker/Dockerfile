# CDG² PPC Cross-Compiler Toolchain (Docker)
#
# Usa gcc-powerpc-linux-gnu do apt (build ~5min, sem compilar GCC do zero).
# Toolchain: powerpc-linux-gnu-gcc (glibc, suficiente para BSP + kernel).
#
# Build:  docker build -t cdg2-toolchain docker/
# Use:    make docker-kernel  (extrai aros-ppc.bin + gera ROM)
#         make rom-aros KERNEL=aros-ppc.bin

FROM ubuntu:24.04

LABEL description="CDG² PPC Cross-Compiler (gcc-powerpc)"

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential automake autoconf libtool \
    flex bison \
    python3 python3-mako \
    g++ make cmake \
    netpbm libpng-dev \
    curl wget ca-certificates \
    gcc-powerpc-linux-gnu \
    libsdl2-dev mesa-common-dev libgl1-mesa-dev \
    && rm -rf /var/lib/apt/lists/*

VOLUME /build
WORKDIR /build

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
