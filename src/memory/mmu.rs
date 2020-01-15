use crate::util::binary;
use crate::Cartridge;
use crate::Gpu;

const EXT_RAM_SIZE: usize = 8192;
const W_RAM_SIZE: usize = 8192;
const ECHO_RAM_SIZE: usize = 7679;
const H_RAM_SIZE: usize = 127;
const OAM_SIZE: usize = 159;
const IO_SIZE: usize = 127;

const USER_PROGRAM_AREA_ADDRESS: u16 = 0x100;
const VRAM_ADDRESS: u16 = 0x8000;
const EXT_RAM_ADDRESS: u16 = 0xA000;
const ECHO_RAM_ADDRESS: u16 = 0xE000;
const W_RAM_ADDRESS: u16 = 0xC000;
const OAM_ADDRESS: u16 = 0xFE00;
const IO_ADDRESS: u16 = 0xFF00;
const H_RAM_ADDR: u16 = 0xFF80;
const BG_PAL_ADDR: u16 = 0xFF47;
pub const INTERRUPT_ENABLE_ADDRESS: u16 = 0xFFFF;
pub const INTERRUPT_FLAGS_ADDRESS: u16 = 0xFF0F;

pub enum Opcode {
    Regular(u8),
    CB(u8),
}

pub struct Mmu<'a> {
    cartridge: &'a Cartridge,
    pub gpu: &'a mut Gpu<'a>,
    bios: Option<&'a Cartridge>,
    ext_ram: [u8; EXT_RAM_SIZE],
    w_ram: [u8; W_RAM_SIZE],
    echo_ram: [u8; ECHO_RAM_SIZE],
    h_ram: [u8; H_RAM_SIZE],
    //Remove this when io handling is implemented
    io: [u8; IO_SIZE],
    interrupts_enabled: u8,
    interrupt_flags: u8,
    is_booted: bool,
    keypad: u8,
}

impl<'a> Mmu<'a> {
    pub fn new(
        cartridge: &'a Cartridge,
        gpu: &'a mut Gpu<'a>,
        bios: Option<&'a Cartridge>,
    ) -> Mmu<'a> {
        Mmu {
            cartridge,
            gpu,
            bios,
            ext_ram: [0; EXT_RAM_SIZE],
            w_ram: [0; W_RAM_SIZE],
            echo_ram: [0; ECHO_RAM_SIZE],
            h_ram: [0; H_RAM_SIZE],
            //Remove this when io handling is implemented
            io: [0; IO_SIZE],
            interrupts_enabled: 0,
            interrupt_flags: 0,
            is_booted: false,
            keypad: 0xFF,
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {


            0xFF50 => self.is_booted = true,
            INTERRUPT_FLAGS_ADDRESS => self.interrupt_flags = value,
            VRAM_ADDRESS..=0x9FFF => self.gpu.write_vram(address, value),
            EXT_RAM_ADDRESS..=0xBFFF => self.ext_ram[(address - EXT_RAM_ADDRESS) as usize] = value,
            W_RAM_ADDRESS..=0xDFFF => {
                self.w_ram[(address - W_RAM_ADDRESS) as usize] = value
            }
            ,
            //TODO: What is 0xFDFE??
            ECHO_RAM_ADDRESS..=0xFDFE => {
                self.echo_ram[(address - ECHO_RAM_ADDRESS) as usize] = value
            }
            //TODO: What is 0xFE9E??
            //TODO: Do GPU stuff here
            OAM_ADDRESS..=0xFE9E => self.gpu.write_oam(address, value),
            //TODO: What is 0xFF7E
            IO_ADDRESS..=0xFF7E => {
                if address == 0xFF00 {
                    //Implement Keypad
                    self.keypad = value;
                }

                if address == 0xFF40 {
                    self.gpu.write_lcdc(value);
                }

                if address == 0xFF42 {
                    self.gpu.scroll_y = value;
                }

                if address == 0xFF43 {
                    self.gpu.scroll_x = value;
                }

                if address == BG_PAL_ADDR {
                    self.gpu.set_bgpal(value);
                }

                if address == 0xFF46 {
                    self.dma_transfer(value);
                }

                self.io[(address - IO_ADDRESS) as usize] = value;
            }
            H_RAM_ADDR..=0xFFFD => self.h_ram[(address - H_RAM_ADDR) as usize] = value,
            INTERRUPT_ENABLE_ADDRESS => self.interrupts_enabled = value,
            _ => {}
        };
    }

    fn dma_transfer(&mut self, source_address: u8) {
        //DMA Transfer starts to OAM
        //Start address = value * 0x100 (value << 8)
        //Destination = OAM
        //Write everything from start for OAM length
        //OAM Length = 0xA0 (160)
        let start_address: u16 = (source_address as u16) << 8;

        for offset in 0..160 {
            self.gpu
                .write_oam(OAM_ADDRESS + offset, self.read(start_address + offset))
        }
        //TODO: Cycles are missing here
        //The transfer takes 160 machine cycles
    }

    pub fn write_word(&mut self, address: u16, value: u16) {
        self.write(address, (value >> 8) as u8);
        self.write(address + 0x01, value as u8);
    }

    pub fn read(&self, address: u16) -> u8 {
        match address {
            INTERRUPT_FLAGS_ADDRESS => self.interrupt_flags,
            0..=0xFF => {
                if !self.is_booted {
                    match self.bios {
                        Some(bios_cartridge) => bios_cartridge.read(address),
                        None => {
                            panic!("BIOS has to be present if system is not booted up");
                        }
                    }
                } else {
                    self.cartridge.read(address)
                }
            }
            USER_PROGRAM_AREA_ADDRESS..=0x7FFF => self.cartridge.read(address),
            VRAM_ADDRESS..=0x9FFF => self.gpu.read_vram(address),
            EXT_RAM_ADDRESS..=0xBFFF => self.ext_ram[(address - EXT_RAM_ADDRESS) as usize],
            W_RAM_ADDRESS..=0xDFFF => self.w_ram[(address - W_RAM_ADDRESS) as usize],
            //TODO: What is 0xFDFE??
            ECHO_RAM_ADDRESS..=0xFDFE => self.echo_ram[(address - ECHO_RAM_ADDRESS) as usize],
            //TODO: What is 0xFE9E??
            //TODO: Do GPU stuff here
            OAM_ADDRESS..=0xFE9E => self.gpu.read_oam(address),
            //TODO: What is 0xFF7F
            //Unusable memory. Return 0
            0xFEA0..=0xFEFE => 0,
            IO_ADDRESS..=0xFF7E => {
                if address == 0xFF00 {
                    //Implement Keypad
                    return 0xFF;
                }

                if address == 0xFF40 {
                    return self.gpu.lcdc;
                }

                if address == 0xFF42 {
                    return self.gpu.scroll_y;
                }

                if address == 0xFF43 {
                    return self.gpu.scroll_x;
                }

                if address == 0xFF44 {
                    return self.gpu.current_scanline;
                }

                self.io[(address - IO_ADDRESS) as usize]
            }
            H_RAM_ADDR..=0xFFFD => self.h_ram[(address - H_RAM_ADDR) as usize],
            INTERRUPT_ENABLE_ADDRESS => self.interrupts_enabled,
            _ => 0,
        }
    }

    pub fn read_word(&self, address: u16) -> u16 {
        binary::bytes_to_word(self.read(address), self.read(address + 0x01))
    }

    pub fn read_opcode(&self, pc: u16) -> Opcode {
        let op_code = self.read(pc);

        match op_code {
            0xCB => Opcode::CB(self.read(pc + 1)),
            _ => Opcode::Regular(op_code),
        }
    }
}