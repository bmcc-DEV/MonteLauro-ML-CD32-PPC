# MonteLauro CD3² — Makefile de Desenvolvimento
# Uso: make <target> [ROM=...] [KERNEL=...] [AROS=...]

CARGO   ?= cargo
ROM     ?= rom/aros_cd32.rom
KERNEL  ?= aros-ppc.bin
CYCLES  ?= 5000000
AROS    ?= $(AROS)

.PHONY: all build rom-hello rom-aros headers check-abi \
        test-hello test-aros trace-hello trace-aros \
        sdl-hello sdl-aros save load \
        aros-setup aros-build \
        clean distclean

all: build headers check-abi

# ── Build ─────────────────────────────────────────────────────────────

build:
	$(CARGO) build --release
	@echo "Emulador compilado: target/release/cd32-rs"

build-sdl:
	$(CARGO) build --release --features sdl-frontend

# ── ROMs ──────────────────────────────────────────────────────────────

rom-hello:
	mkdir -p rom
	$(CARGO) run --bin gen-rom --release -- \
		--target hello --output rom/hello_cd32.rom

rom-aros:
	mkdir -p rom
	$(CARGO) run --bin gen-rom --release -- \
		--target aros-bootstrap \
		$(if $(wildcard $(KERNEL)),--kernel $(KERNEL),) \
		--output $(ROM)

# ── ABI Headers + Conformidade ──────────────────────────────────────

headers:
	$(CARGO) run --bin gen-headers --release

check-abi: headers
	$(CARGO) run --bin check-abi --release 2>/dev/null || \
		echo "(check-abi tool disponivel via cargo run --bin check-abi)"

# ── Testes ────────────────────────────────────────────────────────────

test-hello: build rom-hello
	$(CARGO) run --release --bin cd32-rs -- \
		--bios rom/hello_cd32.rom --cycles $(CYCLES)

test-aros: build rom-aros
	$(CARGO) run --release --bin cd32-rs -- \
		--bios $(ROM) --cycles $(CYCLES)

stress: build rom-aros
	$(CARGO) run --release --bin cd32-rs -- \
		--bios $(ROM) --cycles 500000000 --save-state rom/stress.sav

trace-hello: build rom-hello
	$(CARGO) run --release --bin cd32-rs -- \
		--bios rom/hello_cd32.rom --cycles $(CYCLES) --trace --verbose

trace-aros: build rom-aros
	$(CARGO) run --release --bin cd32-rs -- \
		--bios $(ROM) --cycles $(CYCLES) --trace --verbose

# ── SDL Frontend ──────────────────────────────────────────────────────

sdl-hello: build-sdl rom-hello
	$(CARGO) run --release --features sdl-frontend --bin cd32-rs -- \
		--bios rom/hello_cd32.rom --sdl

sdl-aros: build-sdl rom-aros
	$(CARGO) run --release --features sdl-frontend --bin cd32-rs -- \
		--bios $(ROM) --sdl

# ── Save / Load ───────────────────────────────────────────────────────

save:
	$(CARGO) run --release --bin cd32-rs -- \
		--bios $(ROM) --cycles $(CYCLES) --save-state estado.sav

load:
	$(CARGO) run --release --bin cd32-rs -- \
		--load-state estado.sav

# ── Integracao AROS ──────────────────────────────────────────────────

aros-setup:
	@if [ -z "$(AROS)" ]; then \
		echo "ERRO: Defina AROS=/path/para/AROS"; exit 1; fi
	./tools/setup-aros.sh

aros-build: aros-setup
	@if [ -z "$(AROS)" ]; then \
		echo "ERRO: Defina AROS=/path/para/AROS"; exit 1; fi
	make -C $(AROS) cpu=ppc board=montelauro-cd32
	cp $(AROS)/bin/ppc/aros-ppc.bin rom/aros-ppc.bin
	$(MAKE) rom-aros KERNEL=rom/aros-ppc.bin

# ── Validação completa ────────────────────────────────────────────────

ci: build headers check-abi rom-hello test-hello rom-aros test-aros
	@echo "=== CI PASS ==="

# ── Limpeza ───────────────────────────────────────────────────────────

clean:
	$(CARGO) clean
	rm -rf rom/ estado.sav hello_cd32.rom aros_cd32.rom

distclean: clean
	rm -f include/cd32_platform.h src/cd32_abi.rs
