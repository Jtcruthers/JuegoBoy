use std::fmt;

use log::*;

use crate::mmu::{Memory, INTERRUPT_MASK};
use crate::registers::Registers;
use crate::registers::{Register, Register::*, Register16, Register16::*};

const IF_FLAG: u16 = 0xFF0F;
const IE_FLAG: u16 = 0xFFFF;

pub struct CPU {
    registers: Registers,
    sp: u16,
    pc: u16,
    ime: bool,
    halted: bool,
    halt_bug: bool,
    ei_called: bool,
}

impl fmt::Display for CPU {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let r = &self.registers;
        write!(
            f,
            "A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X}\n\
             SP: {:04X} PC: {:04X} Flags: {}{}{}{} IME: {} Halted: {}",
            r.a,
            r.f,
            r.b,
            r.c,
            r.d,
            r.e,
            r.h,
            r.l,
            self.sp,
            self.pc,
            if r.zero_flag() { 'Z' } else { '-' },
            if r.subtract_flag() { 'N' } else { '-' },
            if r.half_carry_flag() { 'H' } else { '-' },
            if r.carry_flag() { 'C' } else { '-' },
            self.ime,
            self.halted,
        )
    }
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            registers: Registers::new(),
            sp: 0x0,
            pc: 0x0,
            ime: false,
            halted: false,
            halt_bug: false,
            ei_called: false,
        }
    }

    fn fetch(&mut self, mmu: &mut impl Memory) -> u8 {
        let opcode = self.read_byte(mmu, self.pc);

        if self.halt_bug {
            // halt bug doesnt increment pc
            self.halt_bug = false;
        } else {
            self.pc += 1;
        }

        opcode
    }

    pub fn step(&mut self, mmu: &mut impl Memory) {
        trace!("\n\n");
        let interrupts_pending = self.get_pending_interrupt(mmu);

        if self.halted {
            debug!("Halted");
            if interrupts_pending == 0 {
                mmu.tick();
                debug!("No interrupts pending, staying halted");
                return;
            }

            // pending interrupt ends halt state
            // But ime determines if we handle it
            self.halted = false;
            if self.ime {
                debug!("Interrupts pending, ime true, not halted and handling interrupt");
                self.interrupt_service_routine(mmu);
            } else {
                debug!("Interrupts pending, ime false, not halted and returning");
                mmu.tick();
                debug!("ticked");
                // TODO - not sure if we tick when ime is off but pending interrupt ends the halt
                // Assuming yes since it had to do something to check what's going on, but didn't
                // run the ISR
            }

            return;
        }

        if interrupts_pending > 0 && self.ime {
            debug!("Handling Interrupts: 0b{:08b}", interrupts_pending);
            self.interrupt_service_routine(mmu);
        }

        let opcode = self.fetch(mmu);
        trace!("PC: {:04X} OP: {:02X}", self.pc - 1, opcode);
        trace!(
            "BEFORE A: {:02X} B: {:02X} HL: {:04X}",
            self.registers[A],
            self.registers[B],
            self.registers.hl()
        );
        self.execute(opcode, mmu);
        trace!(
            "AFTER A: {:02X} B: {:02X} HL: {:04X}",
            self.registers[A],
            self.registers[B],
            self.registers.hl()
        );

        // Enable IME flag after instruction following EI
        if self.ei_called && opcode != 0xFB {
            self.ime = true;
            self.ei_called = false;
        }
    }

    fn read_byte(&mut self, mmu: &mut impl Memory, address: u16) -> u8 {
        let value = mmu.read_byte(address);
        mmu.tick();
        value
    }

    fn write_byte(&mut self, mmu: &mut impl Memory, address: u16, value: u8) {
        mmu.write_byte(address, value);
        mmu.tick();
    }

    /// Helper that takes a u16 and splits it into two individual u8 bytes
    ///
    /// Does NOT call tick, the caller is responsible for calling it if necessary
    fn u16_to_u8s(&self, value: u16) -> (u8, u8) {
        let upper = (value >> 8) as u8;
        let lower = (value & 0x00FF) as u8;

        (upper, lower)
    }

    fn rlc(&mut self, value: u8) -> u8 {
        let carry_bit = (value & 0b10000000) >> 7;
        let new_value = (value << 1) + carry_bit;
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn rlc_reg(&mut self, reg: Register) {
        self.registers[reg] = self.rlc(self.registers[reg]);
    }

    fn rlc_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.rlc(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn rrc(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let new_value = (value >> 1) + (carry_bit << 7);
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn rrc_reg(&mut self, reg: Register) {
        self.registers[reg] = self.rrc(self.registers[reg]);
    }

    fn rrc_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.rrc(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn rl(&mut self, value: u8) -> u8 {
        let carry_bit = value >> 7;
        let new_value = (value << 1) + (self.registers.carry_flag() as u8);
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn rl_reg(&mut self, reg: Register) {
        self.registers[reg] = self.rl(self.registers[reg]);
    }

    fn rl_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.rl(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn rr(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let new_value = (value >> 1) + ((self.registers.carry_flag() as u8) << 7);
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn rr_reg(&mut self, reg: Register) {
        self.registers[reg] = self.rr(self.registers[reg]);
    }

    fn rr_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.rr(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn sla(&mut self, value: u8) -> u8 {
        let carry_bit = (value & 0b10000000) >> 7;
        let new_value = value << 1;
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn sla_reg(&mut self, reg: Register) {
        self.registers[reg] = self.sla(self.registers[reg]);
    }

    fn sla_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.sla(value);

        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn sra(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let msb = value & 0b10000000;
        let new_value = (value >> 1) + msb;
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn sra_reg(&mut self, reg: Register) {
        self.registers[reg] = self.sra(self.registers[reg]);
    }

    fn sra_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.sra(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn srl(&mut self, value: u8) -> u8 {
        let carry_bit = value & 1;
        let new_value = value >> 1;
        self.registers
            .set_flags(new_value == 0, false, false, carry_bit > 0);

        new_value
    }

    fn srl_reg(&mut self, reg: Register) {
        self.registers[reg] = self.srl(self.registers[reg]);
    }

    fn srl_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.srl(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn bit(&mut self, bit: u8, value: u8) {
        let bit_is_set = value & (1 << bit) > 0;
        self.registers
            .set_flags(!bit_is_set, false, true, self.registers.carry_flag());
    }

    fn bit_reg(&mut self, bit: u8, reg: Register) {
        self.bit(bit, self.registers[reg]);
    }

    fn bit_hl(&mut self, bit: u8, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        self.bit(bit, value);
    }

    fn res(&mut self, bit: u8, value: u8) -> u8 {
        let mask = !(1 << bit);
        value & mask
    }

    fn res_reg(&mut self, bit: u8, reg: Register) {
        self.registers[reg] = self.res(bit, self.registers[reg]);
    }

    fn res_hl(&mut self, bit: u8, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.res(bit, value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn set(&mut self, bit: u8, value: u8) -> u8 {
        let mask = 1 << bit;
        value | mask
    }

    fn set_reg(&mut self, bit: u8, reg: Register) {
        self.registers[reg] = self.set(bit, self.registers[reg]);
    }

    fn set_hl(&mut self, bit: u8, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.set(bit, value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    fn swap(&mut self, value: u8) -> u8 {
        let new_value = ((value & 0x0F) << 4) + ((value & 0xF0) >> 4);
        self.registers
            .set_flags(new_value == 0, false, false, false);

        new_value
    }

    fn swap_reg(&mut self, reg: Register) {
        self.registers[reg] = self.swap(self.registers[reg]);
    }

    fn swap_hl(&mut self, mmu: &mut impl Memory) {
        let value = self.read_byte(mmu, self.registers.hl());
        let new_value = self.swap(value);
        self.write_byte(mmu, self.registers.hl(), new_value);
    }

    /// Read a u16 from address and address+1, and combines them into a single u16
    /// Takes 2 M-cycles
    fn read_u16(&mut self, address: u16, mmu: &mut impl Memory) -> u16 {
        let upper = self.read_byte(mmu, address + 1) as u16;
        let lower = self.read_byte(mmu, address) as u16;

        (upper << 8) + lower
    }

    /// Helper to take a u8, increments it, and sets flags according to INC r8
    fn inc_u8_and_set_flags(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);

        let z = result == 0;
        let n = false;
        let h = (value & 0x0F) + 1 > 0x0F;
        let c = self.registers.carry_flag();
        self.registers.set_flags(z, n, h, c);

        result
    }

    /// Helper to take a u8, decrement it, and sets flags according to DEC r8
    fn dec_u8_and_set_flags(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);

        let z = result == 0;
        let n = true;
        let h = (value & 0x0F) < (result & 0x0F);
        let c = self.registers.carry_flag();
        self.registers.set_flags(z, n, h, c);

        result
    }

    /// Returns the u16 value from the stack and increment sp
    ///
    /// Takes 2 M-Cycles
    fn pop(&mut self, mmu: &mut impl Memory) -> (u8, u8) {
        let value = self.read_u16(self.sp, mmu);
        self.sp += 2;
        self.u16_to_u8s(value)
    }

    /// Pushes the value onto the stack and decrement sp twice
    ///
    /// Takes 3 M-Cycles
    fn push(&mut self, value: u16, mmu: &mut impl Memory) {
        let (upper, lower) = self.u16_to_u8s(value);
        mmu.tick();
        self.sp -= 1;
        self.write_byte(mmu, self.sp, upper);
        self.sp -= 1;
        self.write_byte(mmu, self.sp, lower);
    }

    /// Pushes address on stack, if the condition is met
    /// Used for CALL n16 and CALL cc,n16, since they take the same amount of
    /// cycles when cc is true
    ///
    /// Takes 5 M-cycles if condition is met, else 2
    fn call(&mut self, condition: bool, mmu: &mut impl Memory) {
        let address = self.read_u16(self.pc, mmu);
        self.pc += 2;
        if condition {
            trace!("calling, condition met");
            self.push(self.pc, mmu);
            self.pc = address;
        } else {
            trace!("not calling, condition not met");
        }
    }

    /// Relative jump, if condition is met
    /// Also used for JR n16, since it takes the same amount of
    /// cycles as JR cc,n16 when condition is met
    ///
    /// Takes 2 M-Cycles if condition is met, else 1
    fn jump_relative(&mut self, condition: bool, mmu: &mut impl Memory) {
        let offset = self.read_byte(mmu, self.pc) as i8;
        self.pc += 1;
        if condition {
            trace!("jump relative offset: {:X}", offset);
            self.pc = ((self.pc as u32 as i32) + (offset as i32)) as u16;
            mmu.tick(); // Have to tick one for the jump
        } else {
            trace!("not jumping relative");
        }
    }

    /// Absolute jump to address, if condition is met
    /// Also used for JP n16, since it takes the same amount of
    /// cycles as JP cc,n16 when condition is true
    ///
    /// Takes 3 M-cycles if condition is met, else 2
    fn jump_abs(&mut self, condition: bool, mmu: &mut impl Memory) {
        let address = self.read_u16(self.pc, mmu);
        self.pc += 2;
        if condition {
            trace!("jumping abs to address: {:X}", address);
            self.pc = address;
            mmu.tick(); // Have to tick one for the jump
        } else {
            trace!("not jumping absolute");
        }
    }

    /// Adds a u16 value to HL.
    ///
    /// Takes 1 M-Cycle.
    fn add_u16_to_hl(&mut self, mmu: &mut impl Memory, value: u16) {
        let old_hl = self.registers.hl();
        let new_hl = self.registers.hl().wrapping_add(value);
        let (upper, lower) = self.u16_to_u8s(new_hl);
        mmu.tick();
        let half_carry = (old_hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF;
        let carry = value as u32 + old_hl as u32 > 0xFFFF;
        self.registers
            .set_flags(self.registers.zero_flag(), false, half_carry, carry);
        self.registers.h = upper;
        self.registers.l = lower;
    }

    fn halt(&mut self, mmu: &mut impl Memory) {
        self.halted = true;
        if !self.ime && self.get_pending_interrupt(mmu) != 0 {
            // This is the halt bug
            // The next opcode ran will not increment pc
            // and it immediately exits halt
            self.halted = false;
            self.halt_bug = true;
        }
    }

    fn stop(&mut self) {
        todo!("stop not implemented")
    }

    fn add_a(&mut self, value: u8) {
        let old_a = self.registers.a;
        let new_a = self.registers.a.wrapping_add(value);
        let half_carry = (old_a & 0x0F) + (value & 0x0F) > 0x0F;
        self.registers
            .set_flags(new_a == 0, false, half_carry, new_a < old_a);
        self.registers.a = new_a;
    }

    fn add_carry_a(&mut self, value: u8) {
        let old_a = self.registers.a;
        let new_a = self
            .registers
            .a
            .wrapping_add(value)
            .wrapping_add(self.registers.carry_flag() as u8);
        let half_carry = (old_a & 0x0F) + (value & 0x0F) + self.registers.carry_flag() as u8 > 0x0F;
        let carry = (old_a as u16) + (value as u16) + self.registers.carry_flag() as u16 > 0xFF;
        self.registers
            .set_flags(new_a == 0, false, half_carry, carry);
        self.registers.a = new_a;
    }

    fn sub_a(&mut self, value: u8) {
        let old_a = self.registers.a;
        let new_a = self.registers.a.wrapping_sub(value);
        let half_carry = (old_a & 0x0F) < (value & 0x0F);
        self.registers
            .set_flags(new_a == 0, true, half_carry, new_a > old_a);
        self.registers.a = new_a;
    }

    fn sub_carry_a(&mut self, value: u8) {
        let old_a = self.registers.a;
        let new_a = self
            .registers
            .a
            .wrapping_sub(value)
            .wrapping_sub(self.registers.carry_flag() as u8);
        let half_carry = (old_a & 0x0F) < (value & 0x0F) + self.registers.carry_flag() as u8;
        let carry = value as u16 + self.registers.carry_flag() as u16 > old_a as u16;
        self.registers
            .set_flags(new_a == 0, true, half_carry, carry);
        self.registers.a = new_a;
    }

    fn and_a(&mut self, value: u8) {
        self.registers.a &= value;
        self.registers
            .set_flags(self.registers.a == 0, false, true, false);
    }

    fn xor_a(&mut self, value: u8) {
        self.registers.a ^= value;
        self.registers
            .set_flags(self.registers.a == 0, false, false, false);
    }

    fn or_a(&mut self, value: u8) {
        self.registers.a |= value;
        self.registers
            .set_flags(self.registers.a == 0, false, false, false);
    }

    fn cp_a(&mut self, value: u8) {
        let h = (self.registers.a & 0x0F) < (value & 0x0F);
        self.registers
            .set_flags(self.registers.a == value, true, h, self.registers.a < value);
    }

    /// Non-conditional return. Always takes 3 M-cycles
    fn ret(&mut self, mmu: &mut impl Memory) {
        let (upper, lower) = self.pop(mmu);

        let new_pc = ((upper as u16) << 8) as u16 + lower as u16;
        mmu.tick(); // Assuming the third cycle is before setting pc?

        self.pc = new_pc;
    }

    /// Conditional return.
    ///
    /// If true, takes 4 M-cycles, else 1
    fn ret_cc(&mut self, condition: bool, mmu: &mut impl Memory) {
        // I assume the actual hardware would do one tick to calculate the condition, pop takes 2, then one to set the u16
        // So I assume I should spread out the ticks to be more accurate?

        mmu.tick(); // tick for condition

        if condition {
            let (upper, lower) = self.pop(mmu);

            let new_pc = ((upper as u16) << 8) as u16 + lower as u16;

            self.pc = new_pc;
            mmu.tick(); // Not sure where to call the last tick
        }
    }

    fn get_pending_interrupt(&mut self, mmu: &mut impl Memory) -> u8 {
        let if_reg = mmu.read_byte(IF_FLAG);
        let ie_reg = mmu.read_byte(IE_FLAG);

        if_reg & ie_reg & INTERRUPT_MASK
    }

    /// Restart pushed pc, and sets pc to address
    ///
    /// Takes 3 M-cycles
    fn restart(&mut self, address: u16, mmu: &mut impl Memory) {
        self.push(self.pc, mmu);
        self.pc = address;
    }

    // Supposed to take exactly 5 M-cycles
    fn interrupt_service_routine(&mut self, mmu: &mut impl Memory) {
        // Two M-cycles in the beginning since we will handle the interrupt here
        mmu.tick();
        mmu.tick();

        self.ime = false;

        // push pc onto stack taking 2 M-cycles
        let (upper, lower) = self.u16_to_u8s(self.pc);
        self.sp -= 1;
        self.write_byte(mmu, self.sp, upper);
        self.sp -= 1;
        self.write_byte(mmu, self.sp, lower);

        let interrupt = self.get_pending_interrupt(mmu);
        trace!(
            "interrupt service routine called with interrupt: {:X}",
            interrupt
        );

        let if_reg = mmu.read_byte(IF_FLAG);
        // 1 M-cycle to set pc to correct address
        if interrupt & 0b1 != 0 {
            trace!("vblank interrupt");
            self.write_byte(mmu, IF_FLAG, if_reg & 0b11111110);
            self.pc = 0x40;
            return;
        }

        if interrupt & 0b10 != 0 {
            self.write_byte(mmu, IF_FLAG, if_reg & 0b11111101);
            self.pc = 0x48;
            return;
        }

        if interrupt & 0b100 != 0 {
            self.write_byte(mmu, IF_FLAG, if_reg & 0b11111011);
            self.pc = 0x50;
            return;
        }

        if interrupt & 0b1000 != 0 {
            self.write_byte(mmu, IF_FLAG, if_reg & 0b11110111);
            self.pc = 0x58;
            return;
        }

        if interrupt & 0b10000 != 0 {
            self.write_byte(mmu, IF_FLAG, if_reg & 0b11101111);
            self.pc = 0x60;
            return;
        }

        mmu.tick(); // Manually tick since we didn't hit a write_byte to do it for us
        return;
    }

    fn load_reg_from_reg(&mut self, to: Register, from: Register) {
        self.registers[to] = self.registers[from];
    }

    fn load_addr_from_reg(&mut self, addr: u16, reg: Register, mmu: &mut impl Memory) {
        self.write_byte(mmu, addr, self.registers[reg]);
    }

    fn load_reg16_from_pc(&mut self, reg: Register16, mmu: &mut impl Memory) {
        let value = self.read_u16(self.pc, mmu);
        self.pc += 2;
        match reg {
            BC => self.registers.set_16(BC, value),
            DE => self.registers.set_16(DE, value),
            HL => self.registers.set_16(HL, value),
            SP => self.sp = value,
            _ => panic!("Invalid 16-bit register"),
        }
    }

    fn inc_reg(&mut self, reg: Register) {
        self.registers[reg] = self.inc_u8_and_set_flags(self.registers[reg]);
    }

    /// Increments a 16-bit register and ticks the MMU once. Does not set flags.
    fn inc_reg16(&mut self, mmu: &mut impl Memory, reg: Register16) {
        let value = match reg {
            BC => self.registers.bc(),
            DE => self.registers.de(),
            HL => self.registers.hl(),
            SP => self.sp,
            _ => panic!("Invalid 16-bit register"),
        };
        let new_value = value.wrapping_add(1);
        match reg {
            BC => self.registers.set_16(BC, new_value),
            DE => self.registers.set_16(DE, new_value),
            HL => self.registers.set_16(HL, new_value),
            SP => self.sp = new_value,
            _ => panic!("Invalid 16-bit register"),
        }
        mmu.tick();
    }

    fn dec_reg(&mut self, reg: Register) {
        self.registers[reg] = self.dec_u8_and_set_flags(self.registers[reg]);
    }

    /// Decrements a 16-bit register and ticks the MMU once. Does not set flags.
    fn dec_reg16(&mut self, mmu: &mut impl Memory, reg: Register16) {
        let value = match reg {
            BC => self.registers.bc(),
            DE => self.registers.de(),
            HL => self.registers.hl(),
            SP => self.sp,
            _ => panic!("Invalid 16-bit register"),
        };
        let new_value = value.wrapping_sub(1);
        match reg {
            BC => self.registers.set_16(BC, new_value),
            DE => self.registers.set_16(DE, new_value),
            HL => self.registers.set_16(HL, new_value),
            SP => self.sp = new_value,
            _ => panic!("Invalid 16-bit register"),
        }
        mmu.tick();
    }

    fn daa(&mut self) {
        let mut adjustment = 0;
        if self.registers.subtract_flag() {
            if self.registers.half_carry_flag() {
                adjustment += 0x06;
            }
            if self.registers.carry_flag() {
                adjustment += 0x60;
            }
            self.registers[A] = self.registers[A].wrapping_sub(adjustment)
        } else {
            if self.registers.half_carry_flag() || (self.registers[A] & 0xF) > 0x09 {
                adjustment += 0x06;
            }
            if self.registers.carry_flag() || self.registers[A] > 0x99 {
                adjustment += 0x60;
            }
            self.registers[A] = self.registers[A].wrapping_add(adjustment)
        }

        let carry = self.registers.carry_flag() || adjustment >= 0x60;
        self.registers.set_flags(
            self.registers[A] == 0,
            self.registers.subtract_flag(),
            false,
            carry,
        );
    }

    pub fn execute(&mut self, opcode: u8, mmu: &mut impl Memory) {
        match opcode {
            0x00 => {}
            0x01 => self.load_reg16_from_pc(BC, mmu),
            0x02 => self.write_byte(mmu, self.registers.bc(), self.registers.a),
            0x03 => self.inc_reg16(mmu, BC),
            0x04 => self.inc_reg(B),
            0x05 => self.dec_reg(B),
            0x06 => {
                self.registers[B] = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x07 => {
                self.rlc_reg(A);
                // always set z to 0, unlike typical rlc
                self.registers
                    .set_flags(false, false, false, self.registers.carry_flag());
            }
            0x08 => {
                let addr = self.read_u16(self.pc, mmu);
                self.pc += 2;

                let upper = (self.sp >> 8) as u8;
                let lower = (self.sp & 0x00FF) as u8;
                self.write_byte(mmu, addr + 1, upper); // little endian
                self.write_byte(mmu, addr, lower);
            }
            0x09 => self.add_u16_to_hl(mmu, self.registers.bc()),
            0x0A => self.registers.a = self.read_byte(mmu, self.registers.bc()),
            0x0B => self.dec_reg16(mmu, BC),
            0x0C => self.inc_reg(C),
            0x0D => self.dec_reg(C),
            0x0E => {
                self.registers.c = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x0F => {
                self.rrc_reg(A);
                // always set z to 0, unlike typical rrc
                self.registers
                    .set_flags(false, false, false, self.registers.carry_flag());
            }
            0x10 => self.stop(), // Should be STOP
            0x11 => self.load_reg16_from_pc(DE, mmu),
            0x12 => self.write_byte(mmu, self.registers.de(), self.registers.a),
            0x13 => self.inc_reg16(mmu, DE),
            0x14 => self.inc_reg(D),
            0x15 => self.dec_reg(D),
            0x16 => {
                self.registers.d = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x17 => {
                let old_carry = if self.registers.carry_flag() { 1 } else { 0 };
                let new_carry = self.registers.a & 0b10000000 > 0;
                self.registers.a = (self.registers.a << 1) + old_carry;
                self.registers.set_flags(false, false, false, new_carry);
            }

            0x18 => self.jump_relative(true, mmu),
            0x19 => self.add_u16_to_hl(mmu, self.registers.de()),
            0x1A => self.registers.a = self.read_byte(mmu, self.registers.de()),
            0x1B => self.dec_reg16(mmu, DE),
            0x1C => self.inc_reg(E),
            0x1D => self.dec_reg(E),
            0x1E => {
                self.registers.e = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x1F => {
                let old_carry = if self.registers.carry_flag() { 1 } else { 0 };
                let new_carry = self.registers.a & 1 > 0;
                self.registers.a = (self.registers.a >> 1) + (old_carry << 7);
                self.registers.set_flags(false, false, false, new_carry);
            }

            0x20 => self.jump_relative(!self.registers.zero_flag(), mmu),
            0x21 => self.load_reg16_from_pc(HL, mmu),
            0x22 => {
                self.write_byte(mmu, self.registers.hl(), self.registers.a);
                let new_hl = self.registers.hl().wrapping_add(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
            }
            0x23 => self.inc_reg16(mmu, HL),
            0x24 => self.inc_reg(H),
            0x25 => self.dec_reg(H),
            0x26 => {
                self.registers.h = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x27 => self.daa(),

            0x28 => self.jump_relative(self.registers.zero_flag(), mmu),
            0x29 => self.add_u16_to_hl(mmu, self.registers.hl()),
            0x2A => {
                self.registers.a = self.read_byte(mmu, self.registers.hl());
                let new_hl = self.registers.hl().wrapping_add(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
            }
            0x2B => self.dec_reg16(mmu, HL),
            0x2C => self.inc_reg(L),
            0x2D => self.dec_reg(L),
            0x2E => {
                self.registers.l = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x2F => {
                self.registers.a = !self.registers.a;
                self.registers.set_flags(
                    self.registers.zero_flag(),
                    true,
                    true,
                    self.registers.carry_flag(),
                );
            }

            0x30 => self.jump_relative(!self.registers.carry_flag(), mmu),
            0x31 => self.load_reg16_from_pc(SP, mmu),
            0x32 => {
                self.write_byte(mmu, self.registers.hl(), self.registers.a);
                let new_hl = self.registers.hl().wrapping_sub(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
            }
            0x33 => self.inc_reg16(mmu, SP),
            0x34 => {
                let value = self.read_byte(mmu, self.registers.hl());
                let new_value = self.inc_u8_and_set_flags(value);
                self.write_byte(mmu, self.registers.hl(), new_value);
            }
            0x35 => {
                let value = self.read_byte(mmu, self.registers.hl());
                let new_value = self.dec_u8_and_set_flags(value);
                self.write_byte(mmu, self.registers.hl(), new_value);
            }
            0x36 => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.write_byte(mmu, self.registers.hl(), value);
            }
            0x37 => {
                self.registers
                    .set_flags(self.registers.zero_flag(), false, false, true);
            }

            0x38 => self.jump_relative(self.registers.carry_flag(), mmu),
            0x39 => self.add_u16_to_hl(mmu, self.sp),
            0x3A => {
                self.registers.a = self.read_byte(mmu, self.registers.hl());
                let new_hl = self.registers.hl().wrapping_sub(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
            }
            0x3B => self.dec_reg16(mmu, SP),
            0x3C => self.inc_reg(A),
            0x3D => self.dec_reg(A),
            0x3E => {
                self.registers.a = self.read_byte(mmu, self.pc);
                self.pc += 1;
            }
            0x3F => self.registers.set_flags(
                self.registers.zero_flag(),
                false,
                false,
                !self.registers.carry_flag(),
            ),

            0x40 => self.load_reg_from_reg(B, B),
            0x41 => self.load_reg_from_reg(B, C),
            0x42 => self.load_reg_from_reg(B, D),
            0x43 => self.load_reg_from_reg(B, E),
            0x44 => self.load_reg_from_reg(B, H),
            0x45 => self.load_reg_from_reg(B, L),
            0x46 => self.registers[B] = self.read_byte(mmu, self.registers.hl()),
            0x47 => self.load_reg_from_reg(B, A),

            0x48 => self.load_reg_from_reg(C, B),
            0x49 => self.load_reg_from_reg(C, C),
            0x4A => self.load_reg_from_reg(C, D),
            0x4B => self.load_reg_from_reg(C, E),
            0x4C => self.load_reg_from_reg(C, H),
            0x4D => self.load_reg_from_reg(C, L),
            0x4E => self.registers[C] = self.read_byte(mmu, self.registers.hl()),
            0x4F => self.load_reg_from_reg(C, A),

            0x50 => self.load_reg_from_reg(D, B),
            0x51 => self.load_reg_from_reg(D, C),
            0x52 => self.load_reg_from_reg(D, D),
            0x53 => self.load_reg_from_reg(D, E),
            0x54 => self.load_reg_from_reg(D, H),
            0x55 => self.load_reg_from_reg(D, L),
            0x56 => self.registers[D] = self.read_byte(mmu, self.registers.hl()),
            0x57 => self.load_reg_from_reg(D, A),

            0x58 => self.load_reg_from_reg(E, B),
            0x59 => self.load_reg_from_reg(E, C),
            0x5A => self.load_reg_from_reg(E, D),
            0x5B => self.load_reg_from_reg(E, E),
            0x5C => self.load_reg_from_reg(E, H),
            0x5D => self.load_reg_from_reg(E, L),
            0x5E => self.registers[E] = self.read_byte(mmu, self.registers.hl()),
            0x5F => self.load_reg_from_reg(E, A),

            0x60 => self.load_reg_from_reg(H, B),
            0x61 => self.load_reg_from_reg(H, C),
            0x62 => self.load_reg_from_reg(H, D),
            0x63 => self.load_reg_from_reg(H, E),
            0x64 => self.load_reg_from_reg(H, H),
            0x65 => self.load_reg_from_reg(H, L),
            0x66 => self.registers[H] = self.read_byte(mmu, self.registers.hl()),
            0x67 => self.load_reg_from_reg(H, A),

            0x68 => self.load_reg_from_reg(L, B),
            0x69 => self.load_reg_from_reg(L, C),
            0x6A => self.load_reg_from_reg(L, D),
            0x6B => self.load_reg_from_reg(L, E),
            0x6C => self.load_reg_from_reg(L, H),
            0x6D => self.load_reg_from_reg(L, L),
            0x6E => self.registers[L] = self.read_byte(mmu, self.registers.hl()),
            0x6F => self.load_reg_from_reg(L, A),

            0x70 => self.load_addr_from_reg(self.registers.hl(), B, mmu),
            0x71 => self.load_addr_from_reg(self.registers.hl(), C, mmu),
            0x72 => self.load_addr_from_reg(self.registers.hl(), D, mmu),
            0x73 => self.load_addr_from_reg(self.registers.hl(), E, mmu),
            0x74 => self.load_addr_from_reg(self.registers.hl(), H, mmu),
            0x75 => self.load_addr_from_reg(self.registers.hl(), L, mmu),
            0x76 => self.halt(mmu),
            0x77 => self.load_addr_from_reg(self.registers.hl(), A, mmu),

            0x78 => self.load_reg_from_reg(A, B),
            0x79 => self.load_reg_from_reg(A, C),
            0x7A => self.load_reg_from_reg(A, D),
            0x7B => self.load_reg_from_reg(A, E),
            0x7C => self.load_reg_from_reg(A, H),
            0x7D => self.load_reg_from_reg(A, L),
            0x7E => self.registers[A] = self.read_byte(mmu, self.registers.hl()),
            0x7F => self.load_reg_from_reg(A, A),

            0x80 => self.add_a(self.registers.b),
            0x81 => self.add_a(self.registers.c),
            0x82 => self.add_a(self.registers.d),
            0x83 => self.add_a(self.registers.e),
            0x84 => self.add_a(self.registers.h),
            0x85 => self.add_a(self.registers.l),
            0x86 => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.add_a(value);
            }
            0x87 => self.add_a(self.registers.a),

            0x88 => self.add_carry_a(self.registers.b),
            0x89 => self.add_carry_a(self.registers.c),
            0x8A => self.add_carry_a(self.registers.d),
            0x8B => self.add_carry_a(self.registers.e),
            0x8C => self.add_carry_a(self.registers.h),
            0x8D => self.add_carry_a(self.registers.l),
            0x8E => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.add_carry_a(value);
            }
            0x8F => self.add_carry_a(self.registers.a),

            0x90 => self.sub_a(self.registers.b),
            0x91 => self.sub_a(self.registers.c),
            0x92 => self.sub_a(self.registers.d),
            0x93 => self.sub_a(self.registers.e),
            0x94 => self.sub_a(self.registers.h),
            0x95 => self.sub_a(self.registers.l),
            0x96 => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.sub_a(value);
            }
            0x97 => self.sub_a(self.registers.a),

            0x98 => self.sub_carry_a(self.registers.b),
            0x99 => self.sub_carry_a(self.registers.c),
            0x9A => self.sub_carry_a(self.registers.d),
            0x9B => self.sub_carry_a(self.registers.e),
            0x9C => self.sub_carry_a(self.registers.h),
            0x9D => self.sub_carry_a(self.registers.l),
            0x9E => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.sub_carry_a(value);
            }
            0x9F => self.sub_carry_a(self.registers.a),

            0xA0 => self.and_a(self.registers.b),
            0xA1 => self.and_a(self.registers.c),
            0xA2 => self.and_a(self.registers.d),
            0xA3 => self.and_a(self.registers.e),
            0xA4 => self.and_a(self.registers.h),
            0xA5 => self.and_a(self.registers.l),
            0xA6 => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.and_a(value);
            }
            0xA7 => self.and_a(self.registers.a),

            0xA8 => self.xor_a(self.registers.b),
            0xA9 => self.xor_a(self.registers.c),
            0xAA => self.xor_a(self.registers.d),
            0xAB => self.xor_a(self.registers.e),
            0xAC => self.xor_a(self.registers.h),
            0xAD => self.xor_a(self.registers.l),
            0xAE => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.xor_a(value);
            }
            0xAF => self.xor_a(self.registers.a),

            0xB0 => self.or_a(self.registers.b),
            0xB1 => self.or_a(self.registers.c),
            0xB2 => self.or_a(self.registers.d),
            0xB3 => self.or_a(self.registers.e),
            0xB4 => self.or_a(self.registers.h),
            0xB5 => self.or_a(self.registers.l),
            0xB6 => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.or_a(value);
            }
            0xB7 => self.or_a(self.registers.a),

            0xB8 => self.cp_a(self.registers.b),
            0xB9 => self.cp_a(self.registers.c),
            0xBA => self.cp_a(self.registers.d),
            0xBB => self.cp_a(self.registers.e),
            0xBC => self.cp_a(self.registers.h),
            0xBD => self.cp_a(self.registers.l),
            0xBE => {
                let value = self.read_byte(mmu, self.registers.hl());
                self.cp_a(value);
            }
            0xBF => self.cp_a(self.registers.a),

            0xC0 => self.ret_cc(!self.registers.zero_flag(), mmu),
            0xC1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.b = upper;
                self.registers.c = lower;
            }
            0xC2 => self.jump_abs(!self.registers.zero_flag(), mmu),
            0xC3 => self.jump_abs(true, mmu),
            0xC4 => self.call(!self.registers.zero_flag(), mmu),
            0xC5 => self.push(self.registers.bc(), mmu),
            0xC6 => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.add_a(value);
            }
            0xC7 => self.restart(0, mmu),

            0xC8 => self.ret_cc(self.registers.zero_flag(), mmu),
            0xC9 => self.ret(mmu),
            0xCA => self.jump_abs(self.registers.zero_flag(), mmu),
            0xCB => self.execute_cb_opcodes(mmu), // Other opcode sheet
            0xCC => self.call(self.registers.zero_flag(), mmu),
            0xCD => self.call(true, mmu),
            0xCE => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.add_carry_a(value);
            }
            0xCF => self.restart(0x08, mmu),

            0xD0 => self.ret_cc(!self.registers.carry_flag(), mmu),
            0xD1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.d = upper;
                self.registers.e = lower;
            }
            0xD2 => self.jump_abs(!self.registers.carry_flag(), mmu),
            0xD3 => {
                panic!("Unsupported OPCODE 0xD3")
            }
            0xD4 => self.call(!self.registers.carry_flag(), mmu),
            0xD5 => self.push(self.registers.de(), mmu),
            0xD6 => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.sub_a(value);
            }
            0xD7 => self.restart(0x10, mmu),

            0xD8 => self.ret_cc(self.registers.carry_flag(), mmu),
            0xD9 => {
                self.ret(mmu);
                self.ime = true; // Set IME after the RET
                                 // Not sure if it should be on next CPU step though
            }
            0xDA => self.jump_abs(self.registers.carry_flag(), mmu),
            0xDB => {
                panic!("Unsupported OPCODE 0xDB")
            }
            0xDC => self.call(self.registers.carry_flag(), mmu),
            0xDD => {
                panic!("Unsupported OPCODE 0xDD")
            }
            0xDE => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.sub_carry_a(value);
            }
            0xDF => self.restart(0x18, mmu),

            0xE0 => {
                let address = 0xFF00 + self.read_byte(mmu, self.pc) as u16;
                self.pc += 1;
                trace!("Writing {} to 0x{:4X}", self.registers.a, address);
                self.write_byte(mmu, address, self.registers.a);
            }
            0xE1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.h = upper;
                self.registers.l = lower;
            }
            0xE2 => {
                let address = 0xFF00 + self.registers.c as u16;
                self.write_byte(mmu, address, self.registers.a);
            }
            0xE3 => {
                panic!("Unsupported OPCODE 0xE3")
            }
            0xE4 => {
                panic!("Unsupported OPCODE 0xE4")
            }
            0xE5 => self.push(self.registers.hl(), mmu),
            0xE6 => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.and_a(value);
            }
            0xE7 => self.restart(0x20, mmu),

            0xE8 => {
                // Offset is signed 8-bit. Read it as signed, then expand it to 16 bit
                let offset = self.read_byte(mmu, self.pc) as i8 as i16;
                let old_sp = self.sp;
                self.pc += 1;
                mmu.tick();

                self.sp = self.sp.wrapping_add(offset as u16);
                // TODO - where to put ticks? One after incrementing pc,
                // and one after updating sp?
                mmu.tick();

                let half_carry = (old_sp & 0x0F) + (offset as u16 & 0x0F) > 0x0F;
                let carry = (old_sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF;
                self.registers.set_flags(false, false, half_carry, carry);
            }
            0xE9 => self.pc = self.registers.hl(),
            0xEA => {
                let address = self.read_u16(self.pc, mmu);
                self.pc += 2;
                self.write_byte(mmu, address, self.registers.a);
            }
            0xEB => {
                panic!("Unsupported OPCODE 0xEB")
            }
            0xEC => {
                panic!("Unsupported OPCODE 0xEC")
            }
            0xED => {
                panic!("Unsupported OPCODE 0xED")
            }
            0xEE => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.xor_a(value);
            }
            0xEF => self.restart(0x28, mmu),

            0xF0 => {
                let address = 0xFF00 + self.read_byte(mmu, self.pc) as u16;
                trace!("Reading from 0x{:4X}", address);
                self.pc += 1;
                self.registers.a = self.read_byte(mmu, address);
            }
            0xF1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.a = upper;
                self.registers.f = lower & 0b11110000;
            }
            0xF2 => {
                let address = 0xFF00 + self.registers.c as u16;
                self.registers.a = self.read_byte(mmu, address);
            }
            0xF3 => {
                self.ime = false;
                // pandocs says ei followed by di doesn't allow interrupts
                // But that means we have to cancel the EI totally?
                // Nothing says it cancels the pending, but maybe thats an implementation detail
                // EI waits until after the next instruction, but since DI supposedly cancels out EI
                // I will cancel the pend here
                self.ei_called = false;
            }
            0xF4 => {
                panic!("Unsupported OPCODE 0xF4")
            }
            0xF5 => self.push(self.registers.af(), mmu),
            0xF6 => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                self.or_a(value);
            }
            0xF7 => self.restart(0x30, mmu),

            0xF8 => {
                // Offset is signed 8-bit. Read it as signed, then expand it to 16 bit
                let offset = self.read_byte(mmu, self.pc) as i8 as i16;
                let old_sp = self.sp;
                self.pc += 1;
                let value = self.sp.wrapping_add(offset as u16);
                // TODO - where to put tick? After incrememnting sp or pc?
                mmu.tick();

                let (upper, lower) = self.u16_to_u8s(value);
                let half_carry = (old_sp & 0x0F) + (offset as u16 & 0x0F) > 0x0F;
                let carry = (old_sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF;
                self.registers.h = upper;
                self.registers.l = lower;
                self.registers.set_flags(false, false, half_carry, carry);
            }
            0xF9 => {
                let hl = self.registers.hl();
                mmu.tick();
                self.sp = hl;
            }
            0xFA => {
                let address = self.read_u16(self.pc, mmu);
                self.pc += 2;
                self.registers.a = self.read_byte(mmu, address);
            }
            0xFB => self.ei_called = true,
            0xFC => {
                panic!("Unsupported OPCODE 0xFC")
            }
            0xFD => {
                panic!("Unsupported OPCODE 0xFD")
            }
            0xFE => {
                let value = self.read_byte(mmu, self.pc);
                self.pc += 1;
                trace!("Comparing A {} to {}", self.registers.a, value);
                self.cp_a(value);
            }
            0xFF => self.restart(0x38, mmu),
        }
    }

    fn execute_cb_opcodes(&mut self, mmu: &mut impl Memory) {
        let opcode = self.read_byte(mmu, self.pc);
        self.pc += 1;

        match opcode {
            0x00 => self.rlc_reg(B),
            0x01 => self.rlc_reg(C),
            0x02 => self.rlc_reg(D),
            0x03 => self.rlc_reg(E),
            0x04 => self.rlc_reg(H),
            0x05 => self.rlc_reg(L),
            0x06 => self.rlc_hl(mmu),
            0x07 => self.rlc_reg(A),

            0x08 => self.rrc_reg(B),
            0x09 => self.rrc_reg(C),
            0x0A => self.rrc_reg(D),
            0x0B => self.rrc_reg(E),
            0x0C => self.rrc_reg(H),
            0x0D => self.rrc_reg(L),
            0x0E => self.rrc_hl(mmu),
            0x0F => self.rrc_reg(A),

            0x10 => self.rl_reg(B),
            0x11 => self.rl_reg(C),
            0x12 => self.rl_reg(D),
            0x13 => self.rl_reg(E),
            0x14 => self.rl_reg(H),
            0x15 => self.rl_reg(L),
            0x16 => self.rl_hl(mmu),
            0x17 => self.rl_reg(A),

            0x18 => self.rr_reg(B),
            0x19 => self.rr_reg(C),
            0x1A => self.rr_reg(D),
            0x1B => self.rr_reg(E),
            0x1C => self.rr_reg(H),
            0x1D => self.rr_reg(L),
            0x1E => self.rr_hl(mmu),
            0x1F => self.rr_reg(A),

            0x20 => self.sla_reg(B),
            0x21 => self.sla_reg(C),
            0x22 => self.sla_reg(D),
            0x23 => self.sla_reg(E),
            0x24 => self.sla_reg(H),
            0x25 => self.sla_reg(L),
            0x26 => self.sla_hl(mmu),
            0x27 => self.sla_reg(A),

            0x28 => self.sra_reg(B),
            0x29 => self.sra_reg(C),
            0x2A => self.sra_reg(D),
            0x2B => self.sra_reg(E),
            0x2C => self.sra_reg(H),
            0x2D => self.sra_reg(L),
            0x2E => self.sra_hl(mmu),
            0x2F => self.sra_reg(A),

            0x30 => self.swap_reg(B),
            0x31 => self.swap_reg(C),
            0x32 => self.swap_reg(D),
            0x33 => self.swap_reg(E),
            0x34 => self.swap_reg(H),
            0x35 => self.swap_reg(L),
            0x36 => self.swap_hl(mmu),
            0x37 => self.swap_reg(A),

            0x38 => self.srl_reg(B),
            0x39 => self.srl_reg(C),
            0x3A => self.srl_reg(D),
            0x3B => self.srl_reg(E),
            0x3C => self.srl_reg(H),
            0x3D => self.srl_reg(L),
            0x3E => self.srl_hl(mmu),
            0x3F => self.srl_reg(A),

            0x40 => self.bit_reg(0, B),
            0x41 => self.bit_reg(0, C),
            0x42 => self.bit_reg(0, D),
            0x43 => self.bit_reg(0, E),
            0x44 => self.bit_reg(0, H),
            0x45 => self.bit_reg(0, L),
            0x46 => self.bit_hl(0, mmu),
            0x47 => self.bit_reg(0, A),

            0x48 => self.bit_reg(1, B),
            0x49 => self.bit_reg(1, C),
            0x4A => self.bit_reg(1, D),
            0x4B => self.bit_reg(1, E),
            0x4C => self.bit_reg(1, H),
            0x4D => self.bit_reg(1, L),
            0x4E => self.bit_hl(1, mmu),
            0x4F => self.bit_reg(1, A),

            0x50 => self.bit_reg(2, B),
            0x51 => self.bit_reg(2, C),
            0x52 => self.bit_reg(2, D),
            0x53 => self.bit_reg(2, E),
            0x54 => self.bit_reg(2, H),
            0x55 => self.bit_reg(2, L),
            0x56 => self.bit_hl(2, mmu),
            0x57 => self.bit_reg(2, A),

            0x58 => self.bit_reg(3, B),
            0x59 => self.bit_reg(3, C),
            0x5A => self.bit_reg(3, D),
            0x5B => self.bit_reg(3, E),
            0x5C => self.bit_reg(3, H),
            0x5D => self.bit_reg(3, L),
            0x5E => self.bit_hl(3, mmu),
            0x5F => self.bit_reg(3, A),

            0x60 => self.bit_reg(4, B),
            0x61 => self.bit_reg(4, C),
            0x62 => self.bit_reg(4, D),
            0x63 => self.bit_reg(4, E),
            0x64 => self.bit_reg(4, H),
            0x65 => self.bit_reg(4, L),
            0x66 => self.bit_hl(4, mmu),
            0x67 => self.bit_reg(4, A),

            0x68 => self.bit_reg(5, B),
            0x69 => self.bit_reg(5, C),
            0x6A => self.bit_reg(5, D),
            0x6B => self.bit_reg(5, E),
            0x6C => self.bit_reg(5, H),
            0x6D => self.bit_reg(5, L),
            0x6E => self.bit_hl(5, mmu),
            0x6F => self.bit_reg(5, A),

            0x70 => self.bit_reg(6, B),
            0x71 => self.bit_reg(6, C),
            0x72 => self.bit_reg(6, D),
            0x73 => self.bit_reg(6, E),
            0x74 => self.bit_reg(6, H),
            0x75 => self.bit_reg(6, L),
            0x76 => self.bit_hl(6, mmu),
            0x77 => self.bit_reg(6, A),

            0x78 => self.bit_reg(7, B),
            0x79 => self.bit_reg(7, C),
            0x7A => self.bit_reg(7, D),
            0x7B => self.bit_reg(7, E),
            0x7C => self.bit_reg(7, H),
            0x7D => self.bit_reg(7, L),
            0x7E => self.bit_hl(7, mmu),
            0x7F => self.bit_reg(7, A),

            0x80 => self.res_reg(0, B),
            0x81 => self.res_reg(0, C),
            0x82 => self.res_reg(0, D),
            0x83 => self.res_reg(0, E),
            0x84 => self.res_reg(0, H),
            0x85 => self.res_reg(0, L),
            0x86 => self.res_hl(0, mmu),
            0x87 => self.res_reg(0, A),

            0x88 => self.res_reg(1, B),
            0x89 => self.res_reg(1, C),
            0x8A => self.res_reg(1, D),
            0x8B => self.res_reg(1, E),
            0x8C => self.res_reg(1, H),
            0x8D => self.res_reg(1, L),
            0x8E => self.res_hl(1, mmu),
            0x8F => self.res_reg(1, A),

            0x90 => self.res_reg(2, B),
            0x91 => self.res_reg(2, C),
            0x92 => self.res_reg(2, D),
            0x93 => self.res_reg(2, E),
            0x94 => self.res_reg(2, H),
            0x95 => self.res_reg(2, L),
            0x96 => self.res_hl(2, mmu),
            0x97 => self.res_reg(2, A),

            0x98 => self.res_reg(3, B),
            0x99 => self.res_reg(3, C),
            0x9A => self.res_reg(3, D),
            0x9B => self.res_reg(3, E),
            0x9C => self.res_reg(3, H),
            0x9D => self.res_reg(3, L),
            0x9E => self.res_hl(3, mmu),
            0x9F => self.res_reg(3, A),

            0xA0 => self.res_reg(4, B),
            0xA1 => self.res_reg(4, C),
            0xA2 => self.res_reg(4, D),
            0xA3 => self.res_reg(4, E),
            0xA4 => self.res_reg(4, H),
            0xA5 => self.res_reg(4, L),
            0xA6 => self.res_hl(4, mmu),
            0xA7 => self.res_reg(4, A),

            0xA8 => self.res_reg(5, B),
            0xA9 => self.res_reg(5, C),
            0xAA => self.res_reg(5, D),
            0xAB => self.res_reg(5, E),
            0xAC => self.res_reg(5, H),
            0xAD => self.res_reg(5, L),
            0xAE => self.res_hl(5, mmu),
            0xAF => self.res_reg(5, A),

            0xB0 => self.res_reg(6, B),
            0xB1 => self.res_reg(6, C),
            0xB2 => self.res_reg(6, D),
            0xB3 => self.res_reg(6, E),
            0xB4 => self.res_reg(6, H),
            0xB5 => self.res_reg(6, L),
            0xB6 => self.res_hl(6, mmu),
            0xB7 => self.res_reg(6, A),

            0xB8 => self.res_reg(7, B),
            0xB9 => self.res_reg(7, C),
            0xBA => self.res_reg(7, D),
            0xBB => self.res_reg(7, E),
            0xBC => self.res_reg(7, H),
            0xBD => self.res_reg(7, L),
            0xBE => self.res_hl(7, mmu),
            0xBF => self.res_reg(7, A),

            0xC0 => self.set_reg(0, B),
            0xC1 => self.set_reg(0, C),
            0xC2 => self.set_reg(0, D),
            0xC3 => self.set_reg(0, E),
            0xC4 => self.set_reg(0, H),
            0xC5 => self.set_reg(0, L),
            0xC6 => self.set_hl(0, mmu),
            0xC7 => self.set_reg(0, A),

            0xC8 => self.set_reg(1, B),
            0xC9 => self.set_reg(1, C),
            0xCA => self.set_reg(1, D),
            0xCB => self.set_reg(1, E),
            0xCC => self.set_reg(1, H),
            0xCD => self.set_reg(1, L),
            0xCE => self.set_hl(1, mmu),
            0xCF => self.set_reg(1, A),

            0xD0 => self.set_reg(2, B),
            0xD1 => self.set_reg(2, C),
            0xD2 => self.set_reg(2, D),
            0xD3 => self.set_reg(2, E),
            0xD4 => self.set_reg(2, H),
            0xD5 => self.set_reg(2, L),
            0xD6 => self.set_hl(2, mmu),
            0xD7 => self.set_reg(2, A),

            0xD8 => self.set_reg(3, B),
            0xD9 => self.set_reg(3, C),
            0xDA => self.set_reg(3, D),
            0xDB => self.set_reg(3, E),
            0xDC => self.set_reg(3, H),
            0xDD => self.set_reg(3, L),
            0xDE => self.set_hl(3, mmu),
            0xDF => self.set_reg(3, A),

            0xE0 => self.set_reg(4, B),
            0xE1 => self.set_reg(4, C),
            0xE2 => self.set_reg(4, D),
            0xE3 => self.set_reg(4, E),
            0xE4 => self.set_reg(4, H),
            0xE5 => self.set_reg(4, L),
            0xE6 => self.set_hl(4, mmu),
            0xE7 => self.set_reg(4, A),

            0xE8 => self.set_reg(5, B),
            0xE9 => self.set_reg(5, C),
            0xEA => self.set_reg(5, D),
            0xEB => self.set_reg(5, E),
            0xEC => self.set_reg(5, H),
            0xED => self.set_reg(5, L),
            0xEE => self.set_hl(5, mmu),
            0xEF => self.set_reg(5, A),

            0xF0 => self.set_reg(6, B),
            0xF1 => self.set_reg(6, C),
            0xF2 => self.set_reg(6, D),
            0xF3 => self.set_reg(6, E),
            0xF4 => self.set_reg(6, H),
            0xF5 => self.set_reg(6, L),
            0xF6 => self.set_hl(6, mmu),
            0xF7 => self.set_reg(6, A),

            0xF8 => self.set_reg(7, B),
            0xF9 => self.set_reg(7, C),
            0xFA => self.set_reg(7, D),
            0xFB => self.set_reg(7, E),
            0xFC => self.set_reg(7, H),
            0xFD => self.set_reg(7, L),
            0xFE => self.set_hl(7, mmu),
            0xFF => self.set_reg(7, A),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::TestMMU;

    fn get_cpu_and_mmu() -> (CPU, TestMMU) {
        (CPU::new(), TestMMU::new())
    }

    #[test]
    fn test_swap() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0xAB;
        cpu.swap_reg(A);
        assert_eq!(cpu.registers[A], 0xBA);
    }

    #[test]
    fn test_set() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b00000000;
        cpu.set_reg(3, A);
        assert_eq!(cpu.registers[A], 0b00001000);

        cpu.registers[A] = 0b00000000;
        cpu.set_reg(7, A);
        assert_eq!(cpu.registers[A], 0b10000000);
    }

    #[test]
    fn test_res() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b11111111;
        cpu.res_reg(3, A);
        assert_eq!(cpu.registers[A], 0b11110111);

        cpu.registers[A] = 0b11111111;
        cpu.res_reg(7, A);
        assert_eq!(cpu.registers[A], 0b01111111);
    }

    #[test]
    fn test_srl() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b10001010;
        cpu.srl_reg(A);
        assert_eq!(cpu.registers[A], 0b01000101);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.sra_reg(A);
        assert_eq!(cpu.registers[A], 0b00100010);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_sra() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b10001010;
        cpu.sra_reg(A);
        assert_eq!(cpu.registers[A], 0b11000101);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.sra_reg(A);
        assert_eq!(cpu.registers[A], 0b11100010);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_rr() {
        let mut cpu = CPU::new();

        // old carry goes into bit 7, bit 0 goes into carry
        cpu.registers.set_flags(false, false, false, true);
        cpu.registers[B] = 0b00000010;
        cpu.rr_reg(B);
        assert_eq!(cpu.registers[B], 0b10000001);
        assert_eq!(cpu.registers.carry_flag(), false);
        assert_eq!(cpu.registers.zero_flag(), false);

        cpu.rr_reg(B);
        assert_eq!(cpu.registers[B], 0b01000000);
        assert_eq!(cpu.registers.carry_flag(), true);

        // zero result sets Z, even when a bit is shifted out into carry
        cpu.registers.set_flags(false, false, false, false);
        cpu.registers[B] = 0b00000001;
        cpu.rr_reg(B);
        assert_eq!(cpu.registers[B], 0);
        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_cb_0x1f_rr_rotate_a_right_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        mmu.memory[0x0] = 0x1F;
        cpu.registers.f = 0b00010000;
        cpu.registers.a = 0b00000000;

        cpu.execute(0xCB, &mut mmu);

        assert_eq!(cpu.registers.a, 0b10000000);
        assert_flags(&cpu, false, false, false, false);
    }

    #[test]
    fn test_cb_0x1e_rr_rotate_hl_ptr_right_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        mmu.memory[0x0] = 0x1E;
        mmu.memory[0xC000] = 0b00000001;
        cpu.registers.h = 0xC0;
        cpu.registers.l = 0x00;
        cpu.registers.f = 0b00000000;

        cpu.execute(0xCB, &mut mmu);

        assert_eq!(mmu.memory[0xC000], 0b00000000);
        assert_flags(&cpu, true, false, false, true);
    }

    #[test]
    fn test_sla() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b10001010;
        cpu.sla_reg(A);
        assert_eq!(cpu.registers[A], 0b00010100);
        assert_eq!(cpu.registers.carry_flag(), true);

        cpu.sla_reg(A);
        assert_eq!(cpu.registers[A], 0b00101000);
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_call() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0x0;
        mmu.memory[0x0] = 0xCD;
        mmu.memory[0x1] = 0xAB;
        cpu.sp = 0x102;
        mmu.memory[0x100] = 0x00;
        mmu.memory[0x101] = 0x00;

        cpu.call(true, &mut mmu);

        assert_eq!(cpu.pc, 0xABCD);
        assert_eq!(cpu.sp, 0x100);
        assert_eq!(mmu.memory[0x100], 0x02); // pc increments 2, and is pushed
        assert_eq!(mmu.memory[0x101], 0x00);

        cpu.pc = 0x0;
        mmu.memory[0x0] = 0xCD;
        mmu.memory[0x1] = 0xAB;
        cpu.sp = 0x102;
        mmu.memory[0x100] = 0x00;
        mmu.memory[0x101] = 0x00;

        cpu.call(false, &mut mmu);

        assert_eq!(cpu.pc, 2);
        assert_eq!(cpu.sp, 0x102);
        assert_eq!(mmu.memory[0x100], 0x00);
        assert_eq!(mmu.memory[0x101], 0x00);
    }

    #[test]
    fn test_pop() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.sp = 0;
        mmu.memory[0x0000] = 0xCD;
        mmu.memory[0x0001] = 0xAB;

        let popped_u8s = cpu.pop(&mut mmu);

        assert_eq!(popped_u8s, (0xAB, 0xCD));
        assert_eq!(cpu.sp, 2);
    }

    #[test]
    fn test_push() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.sp = 2;
        mmu.memory[0x0000] = 0x00;
        mmu.memory[0x0001] = 0x00;
        cpu.push(0xABCD, &mut mmu);

        assert_eq!(mmu.memory[0x0000], 0xCD);
        assert_eq!(mmu.memory[0x0001], 0xAB);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn test_restart() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0x1234;
        cpu.sp = 2;
        mmu.memory[0] = 0x00;
        mmu.memory[1] = 0x00;
        cpu.restart(0, &mut mmu);
        assert_eq!(cpu.pc, 0);
        assert_eq!(cpu.sp, 0);
        assert_eq!(mmu.memory[1], 0x12);
        assert_eq!(mmu.memory[0], 0x34);

        cpu.pc = 0x1234;
        cpu.sp = 2;
        mmu.memory[0] = 0x00;
        mmu.memory[1] = 0x00;
        cpu.restart(0x40, &mut mmu);
        assert_eq!(cpu.pc, 0x40);
        assert_eq!(mmu.memory[1], 0x12);
        assert_eq!(mmu.memory[0], 0x34);
    }

    #[test]
    fn test_ret() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        cpu.sp = 0;
        mmu.memory[0x0000] = 0xCD;
        mmu.memory[0x0001] = 0xAB;
        cpu.ret(&mut mmu);
        assert_eq!(cpu.pc, 0xABCD);
    }

    #[test]
    fn test_add_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 1;
        cpu.add_a(2);
        assert_eq!(cpu.registers.a, 3);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x0F;
        cpu.add_a(0x01);
        assert_eq!(cpu.registers.a, 0x10);
        assert_eq!(cpu.registers.f, 0b00100000);

        cpu.registers.a = 0xF0;
        cpu.add_a(0x10);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, 0b10010000);

        cpu.registers.a = 0xF0;
        cpu.add_a(0x11);
        assert_eq!(cpu.registers.a, 0x01);
        assert_eq!(cpu.registers.f, 0b00010000);
    }

    #[test]
    fn test_sub_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 1;
        cpu.sub_a(2);
        assert_eq!(cpu.registers.a, 0xFF);
        assert_eq!(cpu.registers.f, 0b01110000);

        cpu.registers.a = 0x10;
        cpu.sub_a(0x01);
        assert_eq!(cpu.registers.a, 0x0F);
        assert_eq!(cpu.registers.f, 0b01100000);

        cpu.registers.a = 0x10;
        cpu.sub_a(0x10);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, 0b11000000);

        cpu.registers.a = 0xF0;
        cpu.sub_a(0x10);
        assert_eq!(cpu.registers.a, 0xE0);
        assert_eq!(cpu.registers.f, 0b01000000);
    }

    #[test]
    fn test_xor_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 0b0111;
        cpu.xor_a(0b1101);
        assert_eq!(cpu.registers.a, 0b1010);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x10;
        cpu.xor_a(0x10);
        assert_eq!(cpu.registers.a, 0);
        assert_eq!(cpu.registers.f, 0b10000000);
    }

    #[test]
    fn test_or_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 0b0111;
        cpu.or_a(0b1101);
        assert_eq!(cpu.registers.a, 0b1111);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x00;
        cpu.or_a(0x00);
        assert_eq!(cpu.registers.a, 0);
        assert_eq!(cpu.registers.f, 0b10000000);
    }

    #[test]
    fn test_daa_add() {
        let mut cpu = CPU::new();

        // no carry, zero -> zero
        cpu.registers[A] = 0x00;
        cpu.registers.set_flags(false, false, false, false);

        cpu.daa();

        assert_eq!(cpu.registers[A], 0x00);
        assert_flags(&cpu, true, false, false, false);

        // no carry -> same result
        cpu.registers[A] = 0x77;
        cpu.registers.set_flags(false, false, false, false);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0x77);
        assert_flags(&cpu, false, false, false, false);

        // no carry, lower bit too high
        cpu.registers[A] = 0x0A;
        cpu.registers.set_flags(false, false, false, false);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0x10);
        assert_flags(&cpu, false, false, false, false);

        // no carry, both too high
        cpu.registers[A] = 0x9A;
        cpu.registers.set_flags(false, false, false, false);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0x00);
        assert_flags(&cpu, true, false, false, true);

        // half carry
        cpu.registers[A] = 0x02;
        cpu.registers.set_flags(true, false, true, false);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0x08);
        assert_flags(&cpu, false, false, false, false);

        // carry
        cpu.registers[A] = 0x90;
        cpu.registers.set_flags(false, false, false, true);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0xF0);
        assert_flags(&cpu, false, false, false, true);

        // carry, valid BCD
        cpu.registers[A] = 0xF0;
        cpu.registers.set_flags(false, false, false, true);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0x50);
        assert_flags(&cpu, false, false, false, true);

        // carry and half carry
        cpu.registers[A] = 0xFA;
        cpu.registers.set_flags(true, false, true, true);
        cpu.daa();

        assert_eq!(cpu.registers[A], 0x60);
        assert_flags(&cpu, false, false, false, true);
    }

    #[test]
    fn test_daa_sub() {
        let mut cpu = CPU::new();

        // no carry -> same result
        cpu.registers[A] = 0x77;
        cpu.registers.set_flags(false, true, false, false);

        cpu.daa();

        assert_eq!(cpu.registers[A], 0x77);
        assert_flags(&cpu, false, true, false, false);

        // no carry, zero -> zero
        cpu.registers[A] = 0x00;
        cpu.registers.set_flags(true, true, false, false);

        cpu.daa();

        assert_eq!(cpu.registers[A], 0x00);
        assert_flags(&cpu, true, true, false, false);

        // half carry
        cpu.registers[A] = 0x05;
        cpu.registers.set_flags(true, true, true, false);

        cpu.daa();

        assert_eq!(cpu.registers[A], 0xFF);
        assert_flags(&cpu, false, true, false, false);

        // carry
        cpu.registers[A] = 0x95;
        cpu.registers.set_flags(true, true, false, true);

        cpu.daa();

        assert_eq!(cpu.registers[A], 0x35);
        assert_flags(&cpu, false, true, false, true);

        // half carry and carry
        cpu.registers[A] = 0x70;
        cpu.registers.set_flags(true, true, true, true);

        cpu.daa();

        assert_eq!(cpu.registers[A], 0x0A);
        assert_flags(&cpu, false, true, false, true);
    }

    #[test]
    fn test_cp_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 5;
        cpu.cp_a(5);
        assert_eq!(cpu.registers.f, 0b11000000);

        cpu.registers.a = 0x03;
        cpu.cp_a(0x01);
        assert_eq!(cpu.registers.f, 0b01000000);

        cpu.registers.a = 0x10;
        cpu.cp_a(0x03);
        assert_eq!(cpu.registers.f, 0b01100000);

        cpu.registers.a = 0x00;
        cpu.cp_a(0x33);
        assert_eq!(cpu.registers.f, 0b01110000);
    }

    #[test]
    fn test_0x47_load_a_into_b() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.a = 0x12;
        cpu.registers.b = 0x34;

        cpu.execute(0x47, &mut mmu);

        assert_eq!(cpu.registers.a, 0x12);
        assert_eq!(cpu.registers.b, 0x12);
    }

    #[test]
    fn test_0xe9_load_a_into_b() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;

        cpu.execute(0xe9, &mut mmu);

        assert_eq!(cpu.pc, 0x1234);
        assert_eq!(cpu.registers.hl(), 0x1234);
    }

    #[test]
    fn test_0x3a_load_hl_ptr_into_a_dec() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.a = 0;
        mmu.memory[0x1234] = 0xAA;

        cpu.execute(0x3A, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1233);
        assert_eq!(cpu.registers.a, 0xAA);
    }

    #[test]
    fn test_0x38_jump_relative_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0 | 0b00010000;
        cpu.pc = 0;
        mmu.memory[0] = 0x10;

        cpu.execute(0x38, &mut mmu);

        assert_eq!(cpu.pc, 0x11);
    }

    #[test]
    fn test_0x37_jump_relative_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0;

        cpu.execute(0x37, &mut mmu);

        assert_eq!(cpu.registers.f, 0b00010000);
    }

    #[test]
    fn test_0x36_load_u8_into_hl_ptr() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        mmu.memory[cpu.pc as usize] = 0xAA;
        mmu.memory[cpu.registers.hl() as usize] = 0;

        cpu.execute(0x36, &mut mmu);

        assert_eq!(mmu.memory[cpu.registers.hl() as usize], 0xAA);
    }

    #[test]
    fn test_0x31_load_u16_into_sp() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.sp = 0x0000;
        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x34; // little endian
        mmu.memory[cpu.pc as usize + 1] = 0x12;

        cpu.execute(0x31, &mut mmu);

        assert_eq!(cpu.sp, 0x1234);
        assert_eq!(cpu.pc, 0x0002);
    }

    #[test]
    fn test_0x30_jump_relative_not_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        cpu.execute(0x30, &mut mmu);

        assert_eq!(cpu.pc, 0x11);
    }

    #[test]
    fn test_0x2f_compliment() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.a = 0b10101010;
        cpu.registers.f = 0b10010000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.registers.a, 0b01010101);
        assert_flags(&cpu, true, true, true, true);

        cpu.registers.a = 0b11110000;
        cpu.registers.f = 0b10010000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.registers.a, 0b00001111);
        assert_flags(&cpu, true, true, true, true);

        cpu.registers.a = 0b00001111;
        cpu.registers.f = 0b00000000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.registers.a, 0b11110000);
        assert_flags(&cpu, false, true, true, false);
    }

    #[test]
    fn test_0x2a_load_hl_ptr_into_a() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.a = 0;
        mmu.memory[0x1234] = 0xAA;

        cpu.execute(0x2A, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1235);
        assert_eq!(cpu.registers.a, 0xAA);
    }

    #[test]
    fn test_0x29_add_hl_hl() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x2468);
        assert_flags(&cpu, true, false, false, false);

        cpu.registers.h = 0x3F;
        cpu.registers.l = 0xFF;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x7FFE);
        assert_flags(&cpu, true, false, true, false);

        cpu.registers.h = 0x80;
        cpu.registers.l = 0x00;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x0000);
        assert_flags(&cpu, false, false, false, true);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0xFFFE);
        assert_flags(&cpu, true, false, true, true);
    }

    #[test]
    fn test_0x28_jump_relative_if_z() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0b10000000; // zero, jump_relative
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;
        cpu.execute(0x28, &mut mmu);
        assert_eq!(cpu.pc, 0x11);
    }

    #[test]
    fn test_0x23_inc_hl() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x1235);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x0);
    }

    #[test]
    fn test_0x22_load_a_into_hl_ptr() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.a = 0xAA;
        mmu.memory[0x1234] = 0x00;

        cpu.execute(0x22, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1235);
        assert_eq!(mmu.memory[0x1234], 0xAA);
    }

    #[test]
    fn test_0x21_load_u16_into_hl() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x00;
        cpu.registers.l = 0x00;
        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x34; // little endian
        mmu.memory[cpu.pc as usize + 1] = 0x12;

        cpu.execute(0x21, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(cpu.pc, 0x0002);
    }

    #[test]
    fn test_0x20_jump_relative_not_zero() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0b00000000;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        cpu.execute(0x20, &mut mmu);

        assert_eq!(cpu.pc, 0x11);
    }

    #[test]
    fn test_0x1a_load_de_addr_to_a() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0x00;
        cpu.registers.d = 0x56;
        cpu.registers.e = 0x78;
        mmu.memory[0x5678] = 0x12;

        cpu.execute(0x1A, &mut mmu);

        assert_eq!(cpu.registers.a, 0x12);
    }

    #[test]
    fn test_0x1f_rra_rotate_a_right_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b00000001;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b00000000);
        assert_flags(&cpu, false, false, false, true);

        cpu.registers.f = 0b00010000;
        cpu.registers.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111111);
        assert_flags(&cpu, false, false, false, false);

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b01111111);
        assert_flags(&cpu, false, false, false, false);
    }

    #[test]
    fn test_0x19_add_hl_de() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        // half carry, not carry
        cpu.registers.h = 0b0011_1111;
        cpu.registers.l = 0xFF;
        cpu.registers.d = 0x00;
        cpu.registers.e = 0x01;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.de(), 0x0001);
        assert_eq!(cpu.registers.hl(), 0b01000000_00000000);
        assert_flags(&cpu, false, false, true, false);

        // carry and half carry
        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.registers.d = 0x00;
        cpu.registers.e = 0x01;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.de(), 0x0001);
        assert_eq!(cpu.registers.hl(), 0x00);
        assert_flags(&cpu, false, false, true, true);

        // cary, not half carry
        cpu.registers.h = 0xF0;
        cpu.registers.l = 0xFF;
        cpu.registers.d = 0xF0;
        cpu.registers.e = 0x00;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.de(), 0xF000);
        assert_eq!(cpu.registers.hl(), 0b1110_0000_1111_1111);
        assert_flags(&cpu, false, false, false, true);

        // not cary, not half carry
        cpu.registers.h = 0x01;
        cpu.registers.l = 0x00;
        cpu.registers.d = 0x00;
        cpu.registers.e = 0x00;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.de(), 0x0000);
        assert_eq!(cpu.registers.hl(), 0x0100);
        assert_flags(&cpu, false, false, false, false);
    }

    #[test]
    fn test_0x18_jr_i8() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x50;

        cpu.execute(0x18, &mut mmu);

        assert_eq!(cpu.pc, 0x51); // pc increments to 1, then adds 50
    }

    #[test]
    fn test_0x17_rla_rotate_a_left_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b10000000;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.registers.a, 0b00000000);
        assert_flags(&cpu, false, false, false, true);

        cpu.registers.f = 0b00010000;
        cpu.registers.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111111);
        assert_flags(&cpu, false, false, false, false);

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111110);
        assert_eq!(cpu.registers.zero_flag(), false); // Reset
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
        assert_eq!(cpu.registers.half_carry_flag(), false); // Reset
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_load_u8_into_register() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x16, &mut mmu);

        assert_eq!(cpu.registers.d, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.registers.e = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x1E, &mut mmu);

        assert_eq!(cpu.registers.e, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.registers.l = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x2E, &mut mmu);

        assert_eq!(cpu.registers.l, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.registers.h = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x26, &mut mmu);

        assert_eq!(cpu.registers.l, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_dec_u8_and_set_flagss() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.execute(0x15, &mut mmu);
        assert_eq!(cpu.registers.d, 0x11);

        cpu.registers.d = 0x00;
        cpu.execute(0x15, &mut mmu);
        assert_eq!(cpu.registers.d, 0xFF);

        cpu.registers.e = 0x12;
        cpu.execute(0x1D, &mut mmu);
        assert_eq!(cpu.registers.e, 0x11);

        cpu.registers.e = 0x00;
        cpu.execute(0x1D, &mut mmu);
        assert_eq!(cpu.registers.e, 0xFF);

        cpu.registers.h = 0x12;
        cpu.execute(0x25, &mut mmu);
        assert_eq!(cpu.registers.h, 0x11);

        cpu.registers.h = 0x00;
        cpu.execute(0x25, &mut mmu);
        assert_eq!(cpu.registers.h, 0xFF);

        cpu.registers.l = 0x12;
        cpu.execute(0x2D, &mut mmu);
        assert_eq!(cpu.registers.l, 0x11);

        cpu.registers.l = 0x00;
        cpu.execute(0x2D, &mut mmu);
        assert_eq!(cpu.registers.l, 0xFF);
    }

    #[test]
    fn test_inc_u8_and_set_flagss() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.execute(0x14, &mut mmu);
        assert_eq!(cpu.registers.d, 0x13);

        cpu.registers.d = 0xFF;
        cpu.execute(0x14, &mut mmu);
        assert_eq!(cpu.registers.d, 0x00);

        cpu.registers.e = 0x12;
        cpu.execute(0x1C, &mut mmu);
        assert_eq!(cpu.registers.e, 0x13);

        cpu.registers.e = 0xFF;
        cpu.execute(0x1C, &mut mmu);
        assert_eq!(cpu.registers.e, 0x00);

        cpu.registers.h = 0x12;
        cpu.execute(0x24, &mut mmu);
        assert_eq!(cpu.registers.h, 0x13);

        cpu.registers.h = 0xFF;
        cpu.execute(0x24, &mut mmu);
        assert_eq!(cpu.registers.h, 0x00);

        cpu.registers.l = 0x12;
        cpu.execute(0x2C, &mut mmu);
        assert_eq!(cpu.registers.l, 0x13);

        cpu.registers.l = 0xFF;
        cpu.execute(0x2C, &mut mmu);
        assert_eq!(cpu.registers.l, 0x00);
    }

    #[test]
    fn test_dec_16_bit_registers() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;
        cpu.execute(0x1B, &mut mmu);
        assert_eq!(cpu.registers.de(), 0x1233);

        cpu.registers.d = 0x00;
        cpu.registers.e = 0x00;
        cpu.execute(0x1B, &mut mmu);
        assert_eq!(cpu.registers.de(), 0xFFFF);

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.execute(0x2B, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x1233);

        cpu.registers.h = 0x00;
        cpu.registers.l = 0x00;
        cpu.execute(0x2B, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0xFFFF);

        cpu.sp = 0x1234;
        cpu.execute(0x3B, &mut mmu);
        assert_eq!(cpu.sp, 0x1233);

        cpu.sp = 0x0;
        cpu.execute(0x3B, &mut mmu);
        assert_eq!(cpu.sp, 0xFFFF);
    }

    #[test]
    fn test_inc_16_bit_registers() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;
        cpu.execute(0x13, &mut mmu);
        assert_eq!(cpu.registers.de(), 0x1235);

        cpu.registers.d = 0xFF;
        cpu.registers.e = 0xFF;
        cpu.execute(0x13, &mut mmu);
        assert_eq!(cpu.registers.de(), 0);

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x1235);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0);

        cpu.sp = 0x1234;
        cpu.execute(0x33, &mut mmu);
        assert_eq!(cpu.sp, 0x1235);

        cpu.sp = 0xFFFF;
        cpu.execute(0x33, &mut mmu);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn test_0x12_load_a_into_de_ptr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0xAB;
        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;
        mmu.memory[0x1234] = 0x00;

        cpu.execute(0x12, &mut mmu);

        assert_eq!(mmu.memory[0x1234], 0xAB);
        assert_eq!(cpu.registers.de(), 0x1234);
    }

    #[test]
    fn test_0x11_load_u16_into_de() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x0005;
        mmu.memory[0x0005] = 0x34;
        mmu.memory[0x0006] = 0x12;

        cpu.execute(0x11, &mut mmu);

        assert_eq!(cpu.registers.de(), 0x1234);
        assert_eq!(cpu.pc, 0x0007);
    }

    #[test]
    fn test_0x0f_rrca_right_rotate_a_circular() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0b11111110;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b01111111);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.registers.a = 0b00000001;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b10000000);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_0x0e_load_u8_into_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.c = 0x0;
        cpu.pc = 0x1234;
        mmu.memory[0x1234] = 0xAB;

        cpu.execute(0x0E, &mut mmu);

        assert_eq!(cpu.registers.c, 0xAB);
        assert_eq!(cpu.pc, 0x1235);
    }

    #[test]
    fn test_0x0d_decrement_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.c = 0x01;
        cpu.registers.f = 0b00110000; // Set carry flag

        cpu.execute(0x0D, &mut mmu);

        assert_eq!(cpu.registers.c, 0x00);
        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), true);
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified

        cpu.registers.c = 0x00;
        cpu.registers.f = 0b00010000; // Set carry flag
        cpu.execute(0x0D, &mut mmu);

        assert_eq!(cpu.registers.c, 0xFF);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.subtract_flag(), true);
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified
    }

    #[test]
    fn test_0x0c_increment_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.c = 0x12;
        cpu.registers.f = 0b11110000; // Set carry flag
        cpu.execute(0x0C, &mut mmu);

        assert_eq!(cpu.registers.c, 0x13);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.subtract_flag(), false); // Resets
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified

        cpu.registers.c = 0xFF;
        cpu.registers.f = 0b01010000; // Set carry flag
        cpu.execute(0x0C, &mut mmu);

        assert_eq!(cpu.registers.c, 0x00);
        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), false); // Resets
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified
    }

    #[test]
    fn test_0x0b_decrement_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0x12;
        cpu.registers.c = 0x34;

        cpu.execute(0x0B, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x1233);
    }

    #[test]
    fn test_0x0a_load_bc_addr_to_a() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0x00;
        cpu.registers.b = 0x56;
        cpu.registers.c = 0x78;
        mmu.memory[0x5678] = 0x12;

        cpu.execute(0x0A, &mut mmu);

        assert_eq!(cpu.registers.a, 0x12);
    }

    #[test]
    fn test_0x09_add_hl_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        // half carry, not carry
        cpu.registers.h = 0b0011_1111;
        cpu.registers.l = 0xFF;
        cpu.registers.b = 0x00;
        cpu.registers.c = 0x01;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x09, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x0001);
        assert_eq!(cpu.registers.hl(), 0b01000000_00000000);
        assert_flags(&cpu, false, false, true, false);

        // carry and half carry
        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.registers.b = 0x00;
        cpu.registers.c = 0x01;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x09, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x0001);
        assert_eq!(cpu.registers.hl(), 0x00);
        assert_flags(&cpu, false, false, true, true);

        // cary, not half carry
        cpu.registers.h = 0xF0;
        cpu.registers.l = 0xFF;
        cpu.registers.b = 0xF0;
        cpu.registers.c = 0x00;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x09, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0xF000);
        assert_eq!(cpu.registers.hl(), 0b1110_0000_1111_1111);
        assert_flags(&cpu, false, false, false, true);

        // not cary, not half carry
        cpu.registers.h = 0x01;
        cpu.registers.l = 0x00;
        cpu.registers.b = 0x00;
        cpu.registers.c = 0x00;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x09, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x0000);
        assert_eq!(cpu.registers.hl(), 0x0100);
        assert_flags(&cpu, false, false, false, false);
    }

    #[test]
    fn test_0x08_load_sp_into_addr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x100;
        mmu.memory[cpu.pc as usize] = 0x34;
        mmu.memory[cpu.pc as usize + 1] = 0x12;
        cpu.sp = 0xDEAD;

        cpu.execute(0x08, &mut mmu);

        assert_eq!(cpu.sp, 0xDEAD);
        assert_eq!(mmu.memory[0x1234], 0xAD);
        assert_eq!(mmu.memory[0x1234 + 1], 0xDE);
        assert_eq!(cpu.pc, 0x102);
    }

    #[test]
    fn test_0x07_rlca_rotate_a_left_circular() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0b10000000;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.registers.a, 0b00000001);
        assert_eq!(cpu.registers.zero_flag(), false); // Reset
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
        assert_eq!(cpu.registers.half_carry_flag(), false); // Reset
        assert_eq!(cpu.registers.carry_flag(), true);

        cpu.registers.a = 0b01111111;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111110);
        assert_eq!(cpu.registers.zero_flag(), false); // Reset
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
        assert_eq!(cpu.registers.half_carry_flag(), false); // Reset
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_0x06_load_u8_into_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x06, &mut mmu);

        assert_eq!(cpu.registers.b, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_0x05_decrement_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xFF;
        cpu.registers.f = 0b11110000;

        cpu.execute(0x05, &mut mmu);

        assert_eq!(cpu.registers.b, 0xFE);
        assert_eq!(cpu.registers.zero_flag(), false); // Not zero
        assert_eq!(cpu.registers.subtract_flag(), true); // Always true
                                                         //TODO - assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unaffected

        cpu.registers.b = 0x01;
        cpu.registers.f = 0b11100000;

        cpu.execute(0x06, &mut mmu);

        assert_eq!(cpu.registers.b, 0x00);
        assert_eq!(cpu.registers.zero_flag(), true); // Not zero
        assert_eq!(cpu.registers.subtract_flag(), true); // Always true
                                                         //TODO - assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false); // Unaffected
    }

    #[test]
    fn test_0x04_increment_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xEE;
        cpu.registers.f = 0b11110000;

        cpu.execute(0x04, &mut mmu);

        assert_eq!(cpu.registers.b, 0xEF);
        assert_eq!(cpu.registers.zero_flag(), false); // Not zero
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
                                                          //TODO - assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unaffected
    }

    #[test]
    fn test_0x03_increment_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xEE;
        cpu.registers.c = 0xE0;

        cpu.execute(0x03, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0xEEE1);
    }

    #[test]
    fn test_0x02_load_a_into_bc_addr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0xFF;
        cpu.registers.b = 0x12;
        cpu.registers.c = 0x34;
        mmu.write_byte(0x1234, 0);

        cpu.execute(0x02, &mut mmu);

        assert_eq!(mmu.read_byte(0x1234), 0xFF);
    }

    #[test]
    fn test_0xce_adc_a_n8() {
        let mut cpu = CPU::new();

        cpu.registers.a = 1;
        cpu.registers.set_flags(true, true, true, true);
        cpu.add_carry_a(2);
        assert_eq!(cpu.registers.a, 4);
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.registers.a = 1;
        cpu.registers.set_flags(true, true, true, false);
        cpu.add_carry_a(2);
        assert_eq!(cpu.registers.a, 3);
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.registers.a = 1;
        cpu.registers.set_flags(true, true, true, true);
        cpu.add_carry_a(0b0000_1111);
        assert_eq!(cpu.registers.a, 0b0001_0001);
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.registers.a = 0;
        cpu.registers.set_flags(true, true, true, true);
        cpu.add_carry_a(0b1111_1111);
        assert_eq!(cpu.registers.a, 0b0000_0000);
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_0x01_load_bc_u16() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x100;
        mmu.memory[0x100] = 0x34;
        mmu.memory[0x101] = 0x12;

        cpu.execute(0x01, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x1234);
        assert_eq!(cpu.pc, 0x102);
    }

    #[test]
    fn test_jump_abs() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        cpu.jump_abs(false, &mut mmu);
        assert_eq!(cpu.pc, 2);

        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0xCD;
        mmu.memory[1 + cpu.pc as usize] = 0xAB;
        cpu.jump_abs(true, &mut mmu);
        assert_eq!(cpu.pc, 0xABCD);

        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0xFF;
        mmu.memory[1 + cpu.pc as usize] = 0xFF;
        cpu.jump_abs(true, &mut mmu);
        assert_eq!(cpu.pc, 0xFFFF);
    }

    #[test]
    fn test_jump_relative() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        cpu.jump_relative(false, &mut mmu);
        assert_eq!(cpu.pc, 1);

        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;
        cpu.jump_relative(true, &mut mmu);
        assert_eq!(cpu.pc, 0x11);

        cpu.pc = 0x10;
        mmu.memory[cpu.pc as usize] = (-0x10 as i8) as u8;
        cpu.jump_relative(true, &mut mmu);
        assert_eq!(cpu.pc, 1);
    }

    #[track_caller]
    fn assert_flags(cpu: &CPU, zero: bool, subtract: bool, half_carry: bool, carry: bool) {
        assert_eq!(cpu.registers.zero_flag(), zero);
        assert_eq!(cpu.registers.subtract_flag(), subtract);
        assert_eq!(cpu.registers.half_carry_flag(), half_carry);
        assert_eq!(cpu.registers.carry_flag(), carry);
    }
}
