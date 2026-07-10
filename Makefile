# MonteLauro CD3'² — Makefile de Desenvolvimento
# Uso: make <target> ROM=rom/hello_cd32.rom

CARGO   ?= cargo
ROM     ?= rom/aros_cd32.rom
KERNEL  ?= aros-ppc.bin
CYCLES  ?= 5000000

.PHONY: all build rom-hello rom-aros headers test-hello test-aros trace \
        clean distclean

all: build headers

# ── Build ─────────────────────────────────────────────────────────────

build:
	$(CARGO) build --release

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

# ── Headers ABI ───────────────────────────────────────────────────────

headers:
	$(CARGO) run --bin gen-headers --release 2>/dev/null || \
		rustc tools/gen_headers.rs -o /tmp/gen_headers && /tmp/gen_headers

# ── Testes ────────────────────────────────────────────────────────────

test-hello: rom-hello
	$(CARGO) run --release --bin cd32-rs -- \
		--bios rom/hello_cd32.rom --cycles $(CYCLES)

test-aros: rom-aros
	$(CARGO) run --release --bin cd32-rs -- \
		--bios $(ROM) --cycles $(CYCLES)

trace-hello: rom-hello
	$(CARGO) run --release --bin cd32-rs -- \
		--bios rom/hello_cd32.rom --cycles $(CYCLES) --trace --verbose

trace-aros: rom-aros
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

# ── Limpeza ───────────────────────────────────────────────────────────

clean:
	$(CARGO) clean
	rm -rf rom/ estado.sav hello_cd32.rom aros_cd32.rom
	rm -f /tmp/gen_headers /tmp/test_gen

distclean: clean
	rm -f include/cd32_platform.h src/cd32_abi.rs
