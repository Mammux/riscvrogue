//! A minimal text console backed by the SBI legacy console extension.
//!
//! The console implements [`game::io::Console`]; the game crate
//! provides a blanket `core::fmt::Write` adapter for any `&mut C`
//! where `C: Console`, so `write!` / `writeln!` work both in the
//! kernel and in the game.

use crate::sbi;

/// Console that talks to the firmware via SBI.
pub struct SbiConsole;

impl SbiConsole {
    pub const fn new() -> Self {
        Self
    }
}

impl game::io::Console for SbiConsole {
    fn write_byte(&mut self, byte: u8) {
        if byte == b'\n' {
            sbi::putchar(b'\r');
        }
        sbi::putchar(byte);
    }

    fn read_byte_blocking(&mut self) -> u8 {
        // Busy-poll the SBI legacy console. We *could* use `wfi`
        // here, but without a properly programmed PLIC + trap
        // handler the hart would never be woken up for UART RX
        // interrupts, so we just spin for now. This will be revisited
        // once the kernel grows a real trap/irq subsystem.
        loop {
            if let Some(b) = sbi::getchar() {
                return b;
            }
            core::hint::spin_loop();
        }
    }
}
