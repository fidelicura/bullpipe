const std = @import("std");

const Gpio = struct {
    const base = 0x6000_4000;

    const ENABLE_W1TS_REG: *volatile u32 = @ptrFromInt(base + 0x0024);
    const OUT_W1TS_REG: *volatile u32 = @ptrFromInt(base + 0x0008);
    const OUT_W1TC_REG: *volatile u32 = @ptrFromInt(base + 0x000C);

    const mask: u32 = 1 << 8;
};

fn delay(seconds: u32) void {
    const total = 160 * 1_000_000 * seconds;
    var counter: u32 = 0;
    while (counter < total) : (counter += 1) {
        asm volatile ("nop");
    }
}

pub fn trap() noreturn {
    while (true) {}
}

pub fn main() void {
    Gpio.ENABLE_W1TS_REG.* = Gpio.mask;

    while (true) {
        Gpio.OUT_W1TS_REG.* = Gpio.mask;
        delay(1);

        Gpio.OUT_W1TC_REG.* = Gpio.mask;
        delay(1);
    }
}
