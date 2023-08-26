pub struct Cartridge {
    header: [u8; 0x50],
}

impl Cartridge {
    pub fn new() -> Cartridge {
        Cartridge {
            header: [0; 0x50],
        }
    }
}
