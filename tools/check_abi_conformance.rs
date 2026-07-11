//! Validador de conformidade ABI MontêLauro CD+G².
//!
//! Verifica offsets da struct CD32Platform entre:
//!   - src/cd32_abi.rs (gerado)
//!   - include/cd32_platform.h (gerado)
//!   - docs/aros/abi.md (especificação)
//!
//! Uso: cargo run --bin check-abi

use std::collections::HashMap;
use std::fs;

const ABI_PATH: &str = "docs/aros/abi.md";
const H_PATH: &str = "include/cd32_platform.h";
const RS_PATH: &str = "src/cd32_abi.rs";

const EXPECTED_FIELDS: &[&str] = &[
    "magic",
    "total_ram",
    "chip_ram_base",
    "chip_ram_size",
    "sys_ram_base",
    "sys_ram_size",
    "vram_base",
    "vram_size",
    "boot_rom_base",
    "boot_rom_size",
    "cf_mailbox",
    "gpu_base",
    "dsp_base",
    "dma_base",
    "cdrom_base",
    "gpio_base",
    "coldfire_base",
];

fn parse_c_fields(content: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in content.lines() {
        let t = line.trim();
        if t.contains("typedef struct") || t.contains("struct CD32Platform") || t.contains("struct __attribute__") {
            in_struct = true;
            continue;
        }
        if in_struct {
            if t.starts_with('}') {
                break;
            }
            if t.starts_with("uint32_t") {
                let name = t
                    .trim_start_matches("uint32_t")
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                if !name.is_empty() {
                    fields.push(name.to_string());
                }
            }
        }
    }
    fields
}

fn parse_rs_fields(content: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in content.lines() {
        let t = line.trim();
        if t.contains("struct CD32Platform") {
            in_struct = true;
            continue;
        }
        if in_struct {
            if t == "}" || t.starts_with('}') {
                break;
            }
            if t.starts_with("pub ") {
                // pub magic: u32,
                let rest = t.trim_start_matches("pub ").trim();
                if let Some(name) = rest.split(':').next() {
                    fields.push(name.trim().to_string());
                }
            }
        }
    }
    fields
}

fn parse_spec_fields(content: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    for line in content.lines() {
        let t = line.trim();
        if t.contains("struct CD32Platform") {
            in_struct = true;
            continue;
        }
        if in_struct {
            if t.starts_with('}') {
                break;
            }
            if t.starts_with("uint32_t") {
                // uint32_t  magic; // comment
                let after = t.trim_start_matches("uint32_t").trim();
                let name = after
                    .split(|c: char| c == ';' || c == '/' || c.is_whitespace())
                    .find(|s| !s.is_empty())
                    .unwrap_or("");
                if !name.is_empty() {
                    fields.push(name.to_string());
                }
            }
        }
    }
    fields
}

fn main() {
    let mut ok = true;

    let h = fs::read_to_string(H_PATH).unwrap_or_default();
    let rs = fs::read_to_string(RS_PATH).unwrap_or_default();
    let spec = fs::read_to_string(ABI_PATH).unwrap_or_default();

    if h.is_empty() {
        eprintln!("FAIL: missing {}", H_PATH);
        ok = false;
    }
    if rs.is_empty() {
        eprintln!("FAIL: missing {}", RS_PATH);
        ok = false;
    }

    let c_fields = parse_c_fields(&h);
    let rs_fields = parse_rs_fields(&rs);
    let spec_fields = parse_spec_fields(&spec);

    if c_fields != rs_fields {
        eprintln!("MISMATCH: C fields {:?} != Rust fields {:?}", c_fields, rs_fields);
        ok = false;
    }

    // Spec may list extra trailing fields; require prefix match for core ABI
    if !spec_fields.is_empty() {
        for (i, name) in EXPECTED_FIELDS.iter().enumerate() {
            if c_fields.get(i).map(|s| s.as_str()) != Some(*name) {
                eprintln!(
                    "MISMATCH: field[{}] C={:?} expected={}",
                    i,
                    c_fields.get(i),
                    name
                );
                ok = false;
            }
            if let Some(sf) = spec_fields.get(i) {
                if sf != *name {
                    eprintln!(
                        "MISMATCH: field[{}] spec={} expected={}",
                        i, sf, name
                    );
                    ok = false;
                }
            }
        }
    }

    // Offsets: cada uint32_t → 4 bytes
    let size = c_fields.len() * 4;
    if size != 68 {
        eprintln!("WARN: CD32Platform size {} (expected 68 for 17 fields)", size);
        if c_fields.len() != EXPECTED_FIELDS.len() {
            ok = false;
        }
    }

    // Verifica constantes de memória unificada na spec (best-effort)
    let mut notes = HashMap::new();
    if spec.contains("24MB") || spec.contains("0x0180_0000") || spec.contains("0x01800000") {
        notes.insert("unified_24mb", true);
    }

    if ok {
        println!(
            "ABI CONFORMANCE: PASS ({} fields, size={} bytes){}",
            c_fields.len(),
            size,
            if notes.contains_key("unified_24mb") {
                " [24MB unified noted in spec]"
            } else {
                ""
            }
        );
    } else {
        eprintln!("ABI CONFORMANCE: FAIL");
        std::process::exit(1);
    }
}
