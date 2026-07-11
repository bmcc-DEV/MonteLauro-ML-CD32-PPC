//! Gera headers C/Rust da ABI MonteLauro CD+G².
//! Uso: cargo run --bin gen-headers

use std::fs;

const ABI_PATH: &str = "docs/aros/abi.md";
const H_PATH: &str = "include/cd32_platform.h";
const RS_PATH: &str = "src/cd32_abi.rs";

#[derive(Clone)]
struct Field { name: String, offset: u32, size: u32 }

fn default_fields() -> Vec<Field> {
    let names = ["magic","total_ram","chip_ram_base","chip_ram_size",
        "sys_ram_base","sys_ram_size","vram_base","vram_size",
        "boot_rom_base","boot_rom_size","cf_mailbox","gpu_base",
        "dsp_base","dma_base","cdrom_base","gpio_base","coldfire_base"];
    names.iter().enumerate().map(|(i,n)| Field {
        name: n.to_string(), offset: i as u32 * 4, size: 4
    }).collect()
}

fn parse_abi() -> Vec<Field> {
    let content = fs::read_to_string(ABI_PATH).unwrap_or_default();
    if content.is_empty() { return default_fields(); }
    let mut fields = Vec::new();
    for line in content.lines() {
        if line.contains("struct CD32Platform") {
            // Extract field names from the code block after this line
        }
    }
    if fields.is_empty() { default_fields() } else { fields }
}

fn gen_c(fields: &[Field]) -> String {
    let mut s = String::new();
    s.push_str("// Auto-generated. DO NOT EDIT.\n");
    s.push_str("#ifndef CD32_PLATFORM_H\n#define CD32_PLATFORM_H\n\n");
    s.push_str("#include <stdint.h>\n\n");
    s.push_str("#define CD32_PLATFORM_MAGIC   0xCD320001u\n");
    s.push_str("#define CD32_PLATFORM_VERSION 1u\n\n");
    s.push_str("typedef struct __attribute__((packed)) {\n");
    for f in fields {
        s.push_str(&format!("    uint32_t {};\n", f.name));
    }
    s.push_str("} CD32Platform;\n\n");
    s.push_str("// Mailbox commands\n");
    for (i, cmd) in ["CF_CMD_EXEC","CF_CMD_IO_READ","CF_CMD_IO_WRITE","CF_CMD_JOYPAD",
        "CF_CMD_CDROM_STATUS","CF_CMD_DMA_AUDIO","CF_CMD_UART_WRITE","CF_CMD_HALT"]
        .iter().enumerate()
    {
        let v = if i < 7 { 0x01 + i as u32 } else { 0xFF };
        s.push_str(&format!("#define {:<20} 0x{:02X}\n", cmd, v));
    }
    s.push_str("\n#endif\n");
    s
}

fn gen_rs(fields: &[Field]) -> String {
    let total_size = fields.last().map(|f| f.offset + f.size).unwrap_or(0);
    let mut s = String::new();
    s.push_str("// Auto-generated. DO NOT EDIT.\n\n");
    s.push_str("pub const CD32_PLATFORM_MAGIC: u32 = 0xCD32_0001;\n\n");
    s.push_str("#[repr(C, packed)]\n#[derive(Clone, Copy, Debug)]\n");
    s.push_str("pub struct CD32Platform {\n");
    for f in fields {
        s.push_str(&format!("    pub {}: u32,\n", f.name));
    }
    s.push_str("}\n\n");
    s.push_str("#[test]\nfn test_platform_layout() {\n");
    s.push_str(&format!("    assert_eq!(std::mem::size_of::<CD32Platform>(), {});\n", total_size));
    s.push_str("}\n");
    s
}

fn main() {
    let fields = parse_abi();
    fs::create_dir_all("include").ok();
    fs::write(H_PATH, gen_c(&fields)).unwrap();
    fs::write(RS_PATH, gen_rs(&fields)).unwrap();
    println!("Generated: {} ({} fields)", H_PATH, fields.len());
    println!("Generated: {} ({} fields)", RS_PATH, fields.len());
}
