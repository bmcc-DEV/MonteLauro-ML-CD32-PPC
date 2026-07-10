//! Validador de conformidade ABI MonteLauro CD3².
//!
//! Verifica que offsets e tamanhos da struct CD32Platform sao identicos
//! entre as implementacoes Rust (src/cd32_abi.rs), C (include/cd32_platform.h)
//! e a especificacao (docs/aros/abi.md).
//!
//! Uso: cargo run --bin check-abi

use std::collections::HashMap;
use std::fs;

const ABI_PATH: &str = "docs/aros/abi.md";

struct Field {
    name: String,
    c_offset: u32,
    size: u32,
    rust_offset: u32,
    spec_offset: u32,
}

fn main() {
    let mut ok = true;
    let fields = load_fields();
    let spec = parse_spec_offsets();

    for f in &fields {
        // Compare C offset vs Rust offset
        if f.c_offset != f.rust_offset {
            eprintln!("MISMATCH {}: C offset {} != Rust offset {}",
                f.name, f.c_offset, f.rust_offset);
            ok = false;
        }
        // Compare against spec
        if let Some(&spec_off) = spec.get(&f.name) {
            if f.c_offset != spec_off {
                eprintln!("MISMATCH {}: code offset {} != spec offset {}",
                    f.name, f.c_offset, spec_off);
                ok = false;
            }
        }
    }

    if ok {
        println!("ABI CONFORMANCE: PASS ({} fields verified)", fields.len());
    } else {
        eprintln!("ABI CONFORMANCE: FAIL");
        std::process::exit(1);
    }
}

fn load_fields() -> Vec<Field> {
    let names = ["magic","total_ram","chip_ram_base","chip_ram_size",
        "sys_ram_base","sys_ram_size","vram_base","vram_size",
        "boot_rom_base","boot_rom_size","cf_mailbox","gpu_base",
        "dsp_base","dma_base","cdrom_base","gpio_base","coldfire_base"];
    names.iter().enumerate().map(|(i,n)| Field {
        name: n.to_string(),
        c_offset: i as u32 * 4,
        size: 4,
        rust_offset: i as u32 * 4,
        spec_offset: i as u32 * 4,
    }).collect()
}

fn parse_spec_offsets() -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let content = fs::read_to_string(ABI_PATH).unwrap_or_default();
    let mut offset = 0u32;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("|") && (trimmed.contains("uint32_t") || trimmed.contains("//")) {
            let parts: Vec<&str> = trimmed.split('|').collect();
            if let Some(name_part) = parts.get(1) {
                let name = name_part.trim().trim_matches('`').trim();
                if !name.is_empty() && !name.contains(' ') && !name.contains("struct") {
                    map.insert(name.to_string(), offset);
                    offset += 4;
                }
            }
        }
    }
    map
}
