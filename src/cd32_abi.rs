// Auto-generated. DO NOT EDIT.

pub const CD32_PLATFORM_MAGIC: u32 = 0xCD32_0001;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct CD32Platform {
    pub magic: u32,
    pub total_ram: u32,
    pub chip_ram_base: u32,
    pub chip_ram_size: u32,
    pub sys_ram_base: u32,
    pub sys_ram_size: u32,
    pub vram_base: u32,
    pub vram_size: u32,
    pub boot_rom_base: u32,
    pub boot_rom_size: u32,
    pub cf_mailbox: u32,
    pub gpu_base: u32,
    pub dsp_base: u32,
    pub dma_base: u32,
    pub cdrom_base: u32,
    pub gpio_base: u32,
    pub coldfire_base: u32,
}

#[test]
fn test_platform_layout() {
    assert_eq!(std::mem::size_of::<CD32Platform>(), 68);
}
