//! riscvrogue kernel entry point.
//!
//! This is a *very* small S-mode kernel that boots on top of OpenSBI
//! (the default firmware used by the QEMU `virt` machine). It:
//!
//! 1. Provides its own `_start` that sets up a stack, zeroes `.bss`
//!    and initialises the heap.
//! 2. Uses the SBI legacy console extension for character I/O so we
//!    do not need to program the 16550 UART directly yet.
//! 3. Hands control to the `game` crate which implements the actual
//!    roguelike on top of the tiny `Console` abstraction.
//!
//! The code is deliberately dependency-light and single-hart for now;
//! SMP, traps, timers and paging will be layered on later.

#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate game;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

use linked_list_allocator::LockedHeap;

mod console;
mod gfx_console;
mod sbi;

use console::SbiConsole;
use game::io::Console;

/// Global kernel heap. Backed by the `__heap_start`/`__heap_end`
/// region carved out by `linker.ld`.
#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

// -----------------------------------------------------------------------------
// Boot shim
// -----------------------------------------------------------------------------
//
// OpenSBI jumps here in S-mode with:
//   a0 = hart id
//   a1 = pointer to device tree blob
//
// We set the stack pointer to the top of the reserved boot stack and
// tail-call into `kmain` written in Rust. The assembly lives in its
// own section (`.text.init`) so the linker script can guarantee it is
// placed first.
global_asm!(
    r#"
    .section .text.init, "ax"
    .globl _start
    _start:
        la      sp, __stack_top
        // Clear frame pointer so backtraces terminate cleanly.
        mv      fp, zero
        // a0 = hartid, a1 = dtb – forward both to kmain.
        call    kmain
    1:  wfi
        j       1b
    "#
);

// -----------------------------------------------------------------------------
// Rust entry point
// -----------------------------------------------------------------------------

/// First Rust function executed after the assembly shim.
///
/// # Safety
/// Must only be called once, from `_start`, with the firmware-provided
/// `hartid` / `dtb` arguments.
#[no_mangle]
pub extern "C" fn kmain(hartid: usize, dtb: usize) -> ! {
    unsafe {
        clear_bss();
        init_heap();
    }

    let mut sbi_console = SbiConsole::new();

    cprintln!(&mut sbi_console, "\n[riscvrogue] kernel booted");
    cprintln!(&mut sbi_console, "[riscvrogue]   hartid = {}", hartid);
    cprintln!(&mut sbi_console, "[riscvrogue]   dtb    = {:#x}", dtb);

    let dungeon_seed = boot_seed(hartid, dtb);
    cprintln!(&mut sbi_console, "[riscvrogue]   seed   = {:#x}", dungeon_seed);

    let mut gfx_console = match gfx_console::FramebufferConsole::new() {
        Ok(console) => {
            cprintln!(&mut sbi_console, "[riscvrogue] framebuffer console online");
            Some(console)
        }
        Err(error) => {
            cprintln!(&mut sbi_console, "[riscvrogue] framebuffer init failed: {}", error);
            cprintln!(&mut sbi_console, "[riscvrogue] falling back to SBI serial console");
            None
        }
    };

    cprintln!(&mut sbi_console, "[riscvrogue] starting game...\n");

    // Hand off to the roguelike. It runs forever; when the player
    // chooses to quit we shut the machine down via SBI.
    if let Some(console) = gfx_console.as_mut() {
        let console: &mut dyn Console = console;
        game::run_dungeon_with_seed(console, dungeon_seed);
    } else {
        let console: &mut dyn Console = &mut sbi_console;
        game::run_dungeon_with_seed(console, dungeon_seed);
    }

    cprintln!(&mut sbi_console, "\n[riscvrogue] game exited, shutting down");
    sbi::shutdown();
}

// -----------------------------------------------------------------------------
// Early boot helpers
// -----------------------------------------------------------------------------

unsafe fn clear_bss() {
    extern "C" {
        static mut __bss_start: u8;
        static mut __bss_end: u8;
    }
    let start = &raw mut __bss_start;
    let end = &raw mut __bss_end;
    let len = end.offset_from(start) as usize;
    core::ptr::write_bytes(start, 0, len);
}

unsafe fn init_heap() {
    extern "C" {
        static mut __heap_start: u8;
        static mut __heap_end: u8;
    }
    let start = &raw mut __heap_start;
    let end = &raw mut __heap_end;
    let size = end.offset_from(start) as usize;
    HEAP.lock().init(start, size);
}

fn boot_seed(hartid: usize, dtb: usize) -> u64 {
    let time = read_time_counter();
    mix64(time ^ ((hartid as u64) << 32) ^ (dtb as u64))
}

fn read_time_counter() -> u64 {
    let value: u64;
    unsafe {
        asm!("rdtime {}", out(reg) value);
    }
    value
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// -----------------------------------------------------------------------------
// Panic handler
// -----------------------------------------------------------------------------

#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    let mut console = SbiConsole::new();
    cprintln!(&mut console, "\n[riscvrogue] *** KERNEL PANIC ***");
    if let Some(loc) = info.location() {
        cprintln!(
            &mut console,
            "  at {}:{}:{}",
            loc.file(),
            loc.line(),
            loc.column()
        );
    }
    cprintln!(&mut console, "  {}", info.message());
    sbi::shutdown();
}
