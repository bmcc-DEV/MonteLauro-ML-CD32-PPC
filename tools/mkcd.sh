#!/bin/sh
# mkcd.sh — MonteLauro CD+G² ISO Mastering Tool
#
# Empacota um jogo (.elf) em imagem ISO9660 jogável no emulador.
#
# Uso: ./tools/mkcd.sh kernel/demo/demo.elf [rom/jogo.iso]

set -e

ELF="${1:-kernel/demo.elf}"
OUT="${2:-rom/jogo.iso}"
TMP=$(mktemp -d)

echo "=== MonteLauro CD+G² — ISO Mastering ==="
echo "Jogo:  $ELF"
echo "ISO:   $OUT"

if [ ! -f "$ELF" ]; then
    echo "ERRO: $ELF nao encontrado. Compile o jogo primeiro:"
    echo "  make -C kernel demo"
    exit 1
fi

# Monta diretorio com GAME.ELF na raiz
mkdir -p "$TMP/cd"
cp "$ELF" "$TMP/cd/GAME.ELF"

# Gera ISO9660 compativel com o emulador
mkisofs -o "$OUT" \
    -V "MONTELAURO" \
    -J \
    -R \
    -sysid "CD32" \
    -volset "MONTELAURO CD32" \
    -publisher "MonteLauro CD+G² Labs" \
    -quiet \
    "$TMP/cd" 2>&1

rm -rf "$TMP"
ls -lh "$OUT"
echo "ISO pronta: $OUT"
echo "Teste: cargo run --release -- --bios rom/aros_cd32.rom --disc $OUT --sdl"
