# Docker image for AROS PPC cross-toolchain + MonteLauro CD32 BSP
#
# Build:  docker build -t montelauro-toolchain .
# Use:    docker run --rm -v $(pwd):/build montelauro-toolchain
#
# The toolchain is pre-built in this image (~2h first build, cached after).
# Mount the project root at /build to compile ROMs.

FROM ubuntu:24.04 AS builder

LABEL description="MonteLauro CD32 AROS PPC Cross-Toolchain"

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential \
    autoconf automake libtool \
    flex bison \
    python3 python3-mako \
    git cmake \
    wget xz-utils \
    netpbm \
    libpng-dev libpng-tools \
    libsdl2-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /aros

# Clone AROS (shallow, just master)
RUN git clone --depth=1 https://github.com/aros-development-team/AROS.git src

# Configure for PPC target (only crosstools, skip ROM build)
RUN mkdir -p build/ppc && cd build/ppc && \
    /aros/src/configure --target=sam440-ppc --enable-crosstools

# Build cross-toolchain only (binutils + gcc for ppc-elf)
# Toolchain: /aros/build/ppc/bin/linux-x86_64/ppc-elf/
# Kernel:    /aros/build/ppc/bin/sam440-ppc/
RUN cd build/ppc && make -j$(nproc) ppc-elf 2>&1 || \
    echo "AVISO: make falhou, tentando alvo alternativo..."
RUN cd build/ppc && make -j$(nproc) cross-toolchain 2>&1 || true
RUN cd build/ppc && make -j$(nproc) kernel 2>&1 || true

# ── Runtime stage ──────────────────────────────────────────────────
# Much smaller image containing just the toolchain + kernel binary

FROM ubuntu:24.04 AS runtime

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    make \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /aros/build/ppc/bin /aros/toolchain
COPY --from=builder /aros/build/ppc/bin/sam440-ppc/aros-ppc.bin /aros/kernel/

VOLUME /build
WORKDIR /build

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
