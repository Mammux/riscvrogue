//! Thin wrappers around the Supervisor Binary Interface.
//!
//! We mostly use the `sbi-rt` crate directly, but expose a couple of
//! helpers so that the rest of the kernel does not need to know which
//! extension is being invoked.
//!
//! The legacy console extensions are deprecated in newer SBI specs in
//! favour of the `DBCN` (debug console) extension. We use them here
//! because they are the simplest thing that works on every OpenSBI
//! version QEMU has ever shipped; migrating to `DBCN` is on the
//! roadmap in the README.
#![allow(deprecated)]

/// Print a single byte to the SBI legacy debug console.
#[inline]
pub fn putchar(byte: u8) {
    // `sbi-rt` exposes the legacy console extension behind the
    // `legacy` cargo feature (enabled in `kernel/Cargo.toml`).
    sbi_rt::legacy::console_putchar(byte as usize);
}

/// Try to read a single byte from the SBI legacy debug console.
///
/// Returns `None` if no byte is currently available.
#[inline]
pub fn getchar() -> Option<u8> {
    let ch = sbi_rt::legacy::console_getchar();
    // The legacy extension returns `-1` (usize::MAX on 64-bit) when
    // no character is available.
    if ch == usize::MAX {
        None
    } else {
        Some(ch as u8)
    }
}

/// Ask the firmware to power the machine off.
pub fn shutdown() -> ! {
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    // If the firmware refuses, spin forever.
    loop {
        unsafe { core::arch::asm!("wfi") };
    }
}
