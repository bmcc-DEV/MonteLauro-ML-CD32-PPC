# CDG² — Makefile de Desenvolvimento

CARGO    ?= cargo
ROM      ?= rom/game_cd32.rom
CYCLES   ?= 50000000
DOCKER   ?= docker

.PHONY: all build rom-hello rom-game test-hello test-game \
        trace-hello sdl-hello sdl-game \
        docker-build docker-kernel \
        headers check-abi ci clean

all: build headers check-abi

# ── Build ─────────────────────────────────────────────────────────────

build:
	cargo build --release

build-sdl:
	cargo build --release --features sdl-frontend

# ── ROMs ──────────────────────────────────────────────────────────────

rom-hello:
	mkdir -p rom
	cargo run --bin gen-rom --release -- --target hello --output rom/hello_cd32.rom

rom-game:
	mkdir -p rom
	cargo run --bin gen-rom --release -- --target game \
		--kernel kernel/demo.bin --output rom/game_cd32.rom

# ── Headers + ABI ─────────────────────────────────────────────────────

headers:
	cargo run --bin gen-headers --release

check-abi:
	cargo run --bin check-abi --release

# ── Testes ────────────────────────────────────────────────────────────

test-hello: build rom-hello
	cargo run --release --bin cd32-rs -- --bios rom/hello_cd32.rom --cycles 5000000

test-game: build rom-game
	cargo run --release --bin cd32-rs -- --bios rom/game_cd32.rom --cycles $(CYCLES)

trace-hello: build rom-hello
	cargo run --release --bin cd32-rs -- --bios rom/hello_cd32.rom --cycles 50000 --trace

# ── SDL ───────────────────────────────────────────────────────────────

sdl-hello: build-sdl rom-hello
	cargo run --release --features sdl-frontend --bin cd32-rs -- \
		--bios rom/hello_cd32.rom --sdl

sdl-game: build-sdl rom-game
	cargo run --release --features sdl-frontend --bin cd32-rs -- \
		--bios rom/game_cd32.rom --sdl

# ── Docker Toolchain ──────────────────────────────────────────────────

docker-build:
	$(DOCKER) build -t cdg2-toolchain docker/

docker-kernel: docker-build
	$(DOCKER) run --rm -v $(PWD):/build cdg2-toolchain sh -c \
		"cd /build/kernel && make clean && make demo CC=powerpc-linux-gnu-gcc"
	cargo run --bin gen-rom --release -- --target game \
		--kernel kernel/demo.bin --output rom/game_cd32.rom

# ── CI ────────────────────────────────────────────────────────────────

ci: build headers check-abi rom-hello test-hello
	@echo "=== CI PASS ==="

# ── Limpeza ───────────────────────────────────────────────────────────

clean:
	cargo clean
	rm -rf rom/
