//! CDROM Controller do CD³².
//!
//! Drive: CD-ROM 12x (sem GD-ROM), slot de expansão DVD opcional.
//! Protocolo: interface SPI-like mapeada em 0x0300_0000.
//!
//! Setor de boot segue padrão AmigaCD (ver bios_dump_notes.md).

use std::collections::VecDeque;

const SECTOR_SIZE: usize = 2048;
const PVD_LBA: u32 = 16;

fn read_le32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(buf[off..off+4].try_into().unwrap_or([0;4]))
}

#[derive(Debug, Clone)]
pub struct IsoFile {
    pub name: String,
    pub lba: u32,
    pub size: u32,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdromState {
    NoDisc,
    SpinningUp,
    Ready,
    Reading,
    Paused,
}

pub struct CdromDrive {
    pub state: CdromState,
    pub regs: [u32; 64],
    pub data_buffer: VecDeque<u8>,
    pub disc_inserted: bool,
    pub lba: u32,
    pub sectors_remaining: u32,
    // "CD" virtual: imagem ISO9660 carregada na memória
    pub virtual_disc: Option<Vec<u8>>,
    // Cache ISO9660 structures
    fs_ready: bool,
    root_lba: u32,
    root_size: u32,
}

impl CdromDrive {
    pub fn new() -> Self {
        Self {
            state: CdromState::NoDisc,
            regs: [0u8; 64].map(|_| 0u32),
            data_buffer: VecDeque::with_capacity(SECTOR_SIZE * 4),
            disc_inserted: false,
            lba: 0,
            sectors_remaining: 0,
            virtual_disc: None,
            fs_ready: false,
            root_lba: 0,
            root_size: 0,
        }
    }

    pub fn insert_disc(&mut self, data: Vec<u8>) {
        self.virtual_disc = Some(data);
        self.disc_inserted = true;
        self.state = CdromState::SpinningUp;
        self.regs[0x00] = 0x01; // disc present
        log::info!("CDROM: disc inserted");
    }

    pub fn eject_disc(&mut self) {
        self.virtual_disc = None;
        self.disc_inserted = false;
        self.state = CdromState::NoDisc;
        self.regs[0x00] = 0x00;
    }

    pub fn read_sector(&mut self, lba: u32) -> Option<Vec<u8>> {
        let disc = self.virtual_disc.as_ref()?;
        let offset = (lba as usize) * SECTOR_SIZE;
        if offset + SECTOR_SIZE > disc.len() {
            return None;
        }
        let mut sector = Vec::with_capacity(SECTOR_SIZE);
        sector.extend_from_slice(&disc[offset..offset + SECTOR_SIZE]);
        Some(sector)
    }

    pub fn tick(&mut self, _cycles: u32) {
        match self.state {
            CdromState::SpinningUp => {
                // ~1s simulado de spin-up
                self.state = CdromState::Ready;
                self.regs[0x04] |= 0x02; // ready bit
                log::debug!("CDROM: ready");
            }
            CdromState::Reading => {
                if self.sectors_remaining > 0 {
                    if let Some(sector) = self.read_sector(self.lba) {
                        self.data_buffer.extend(sector);
                        self.sectors_remaining -= 1;
                        self.lba += 1;
                        self.regs[0x08] = self.lba; // current LBA
                        self.regs[0x0C] = self.sectors_remaining;

                        if self.sectors_remaining == 0 {
                            self.state = CdromState::Ready;
                            self.regs[0x04] |= 0x04; // data ready
                        }
                    } else {
                        // Setor inválido → erro
                        self.state = CdromState::Paused;
                        self.regs[0x04] |= 0x08; // error bit
                    }
                } else {
                    self.state = CdromState::Ready;
                }
            }
            _ => {}
        }
    }

    pub fn start_read(&mut self, lba: u32, sectors: u32) {
        if self.state != CdromState::Ready {
            log::warn!("CDROM: cannot read (state={:?})", self.state);
            return;
        }
        self.lba = lba;
        self.sectors_remaining = sectors;
        self.data_buffer.clear();
        self.state = CdromState::Reading;
        self.regs[0x04] &= !0x04; // clear data ready
        self.regs[0x04] &= !0x08; // clear error
        log::info!("CDROM: start read LBA={} sectors={}", lba, sectors);
    }

    // ── Register access ───────────────────────────────────────────────

    pub fn read_byte(&self, addr: u32) -> u8 {
        let idx = ((addr & 0xFFF) >> 2) as usize;
        if idx < 64 { (self.regs[idx] & 0xFF) as u8 } else { 0 }
    }

    pub fn read_half(&self, addr: u32) -> u16 {
        let idx = ((addr & 0xFFF) >> 2) as usize;
        if idx < 64 { (self.regs[idx] & 0xFFFF) as u16 } else { 0 }
    }

    pub fn read_word(&self, addr: u32) -> Option<u32> {
        let idx = ((addr & 0xFFF) >> 2) as usize;
        if idx < 64 { Some(self.regs[idx]) } else { None }
    }

    pub fn write_byte(&mut self, addr: u32, val: u8) {
        let idx = ((addr & 0xFFF) >> 2) as usize;
        if idx < 64 {
            let shift = (addr & 3) * 8;
            self.regs[idx] = (self.regs[idx] & !(0xFFu32 << shift)) | ((val as u32) << shift);
            self.handle_reg_write(idx, self.regs[idx]);
        }
    }

    pub fn write_half(&mut self, addr: u32, val: u16) {
        if addr & 1 != 0 { return; }
        let idx = ((addr & 0xFFF) >> 2) as usize;
        if idx < 64 {
            let shift = (addr & 2) * 8;
            self.regs[idx] = (self.regs[idx] & !(0xFFFFu32 << shift)) | ((val as u32) << shift);
            self.handle_reg_write(idx, self.regs[idx]);
        }
    }

    pub fn write_word(&mut self, addr: u32, val: u32) {
        if addr & 3 != 0 { return; }
        let idx = ((addr & 0xFFF) >> 2) as usize;
        if idx < 64 {
            self.regs[idx] = val;
            self.handle_reg_write(idx, val);
        }
    }

    fn handle_reg_write(&mut self, idx: usize, val: u32) {
        match idx {
            0x10 => {
                // Command register
                let cmd = val & 0xFF;
                let lba = self.regs[0x14];
                let sectors = self.regs[0x18];
                match cmd {
                    0x01 => self.start_read(lba, sectors),
                    0x02 => self.state = CdromState::Paused,
                    0x03 => self.state = CdromState::Ready,
                    0x10 => { self.mount(); }
                    0x11 => {
                        // Cmd: find file and read into data buffer
                        // Path string is read from system RAM via DMA
                        // For now, just list root dir as a test
                        let files = self.list_root();
                        self.data_buffer.clear();
                        for f in &files {
                            for b in f.name.bytes() { self.data_buffer.push_back(b); }
                            self.data_buffer.push_back(b'\n');
                        }
                        log::info!("CDROM: listed {} root entries", files.len());
                        self.state = CdromState::Ready;
                    }
                    _ => log::warn!("CDROM: unknown command 0x{:02X}", cmd),
                }
            }
            _ => {}
        }
    }

    /// Lê dados do buffer de dados (pelo DMA).
    pub fn read_data(&mut self) -> Option<u8> {
        self.data_buffer.pop_front()
    }

    // ── ISO9660 Filesystem ────────────────────────────────────────────

    pub fn mount(&mut self) -> bool {
        let pvd = match self.read_sector_raw(PVD_LBA) {
            Some(s) => s,
            None => return false,
        };
        if pvd[0] != 1 { // PVD type
            log::warn!("CDROM: no ISO9660 PVD at sector 16 (type={})", pvd[0]);
            return false;
        }
        let root_rec_off: usize = 156;
        self.root_lba = read_le32(&pvd, root_rec_off + 2);
        self.root_size = read_le32(&pvd, root_rec_off + 10);
        self.fs_ready = true;
        log::info!("CDROM: ISO9660 mounted (root LBA={}, size={})", self.root_lba, self.root_size);
        true
    }

    fn read_sector_raw(&self, lba: u32) -> Option<Vec<u8>> {
        let disc = self.virtual_disc.as_ref()?;
        let offset = (lba as usize) * SECTOR_SIZE;
        if offset + SECTOR_SIZE > disc.len() {
            return None;
        }
        Some(disc[offset..offset + SECTOR_SIZE].to_vec())
    }

    fn read_dir_entries(&self, lba: u32, size: u32) -> Vec<IsoFile> {
        let mut files = Vec::new();
        let mut off = 0;
        let mut buf = Vec::new();
        let num_sectors = (size as usize + SECTOR_SIZE - 1) / SECTOR_SIZE;
        for i in 0..num_sectors {
            if let Some(s) = self.read_sector_raw(lba + i as u32) {
                buf.extend_from_slice(&s);
            }
        }
        while off + 1 < buf.len() && buf[off] != 0 {
            let rec_len = buf[off] as usize;
            if rec_len < 34 || off + rec_len > buf.len() { break; }
            let file_lba = read_le32(&buf, off + 2);
            let file_size = read_le32(&buf, off + 10);
            let flags = buf[off + 25];
            let name_len = buf[off + 32] as usize;
            let is_dir = flags & 0x02 != 0;
            if name_len > 0 {
                // Skip '.' and '..' entries
                if name_len == 1 && (buf[off + 33] == 0 || buf[off + 33] == 1) {
                    off += rec_len;
                    continue;
                }
                let name = String::from_utf8_lossy(&buf[off + 33..off + 33 + name_len])
                    .trim_end_matches(';')
                    .to_string();
                files.push(IsoFile { name, lba: file_lba, size: file_size, is_dir });
            }
            off += rec_len;
        }
        files
    }

    pub fn list_root(&self) -> Vec<IsoFile> {
        if !self.fs_ready { return vec![]; }
        self.read_dir_entries(self.root_lba, self.root_size)
    }

    pub fn find_file(&self, path: &str) -> Option<IsoFile> {
        if !self.fs_ready { return None; }
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            return None;
        }
        let mut cur_lba = self.root_lba;
        let mut cur_size = self.root_size;
        for (i, part) in parts.iter().enumerate() {
            let entries = self.read_dir_entries(cur_lba, cur_size);
            let matched = entries.iter().find(|e| e.name.eq_ignore_ascii_case(part));
            match matched {
                Some(f) => {
                    if i == parts.len() - 1 {
                        return Some(f.clone());
                    }
                    if f.is_dir {
                        cur_lba = f.lba;
                        cur_size = f.size;
                    } else {
                        return None; // not a dir, but more path components remain
                    }
                }
                None => return None,
            }
        }
        None
    }

    pub fn read_file(&self, file: &IsoFile, buf: &mut Vec<u8>) -> bool {
        buf.clear();
        let num_sectors = (file.size as usize + SECTOR_SIZE - 1) / SECTOR_SIZE;
        for i in 0..num_sectors {
            if let Some(sector) = self.read_sector_raw(file.lba + i as u32) {
                let end = (file.size as usize).min(buf.len() + SECTOR_SIZE) - buf.len();
                buf.extend_from_slice(&sector[..end.min(SECTOR_SIZE)]);
            } else {
                return false;
            }
        }
        buf.truncate(file.size as usize);
        true
    }
}
