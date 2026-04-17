# riscvrogue

A minimal, bare-metal operating system for **RISC-V 64** written in **Rust**,
whose one and only job is to host a small **text-mode roguelike** — also
written in Rust. The project's initial target is the QEMU `virt` machine; the
long-term goal is to boot the same kernel on real RISC-V hardware (e.g. a
VisionFive 2, a Milk-V Duo/Mars, or a SiFive HiFive board) with as few
platform-specific changes as possible.

> Status: **early scaffolding.** The kernel boots, prints a banner over the
> SBI legacy console, and drops the player into a tiny walking-around demo
> map. Almost everything else is still to come.

## Goals

- **Just enough OS to host a game.** No general-purpose userspace, no POSIX.
  The kernel gives the game a `Console` (bytes in, bytes out), a heap, a
  deterministic RNG, and a notion of time. That's it.
- **Rust all the way down.** Kernel, drivers, and game share a single Cargo
  workspace and a common set of abstractions.
- **Portable between QEMU and real boards.** Board-specific code is isolated
  behind traits; bringing up a new board should mean writing a new UART
  driver and a new memory map, not rewriting the kernel.
- **Lean on the ecosystem.** Existing, well-maintained crates (`riscv`,
  `sbi-rt`, `linked_list_allocator`, `spin`, `rand_xoshiro`, …) are preferred
  over hand-rolled equivalents unless there is a clear reason not to.

## Non-goals (for now)

- Multi-user / multi-process support.
- Loading ELF binaries from disk. The game is linked into the kernel image.
- A full filesystem, TCP/IP stack, or graphical output.
- Security boundaries between the game and the kernel.

## Repository layout

```
riscvrogue/
├── Cargo.toml            # Cargo workspace root
├── rust-toolchain.toml   # pins nightly + adds the riscv64 target
├── .cargo/config.toml    # default build target, linker args, QEMU runner
├── kernel/               # bare-metal S-mode kernel
│   ├── Cargo.toml
│   ├── linker.ld         # memory map for the QEMU `virt` machine
│   └── src/
│       ├── main.rs       # _start shim + kmain + panic handler
│       ├── sbi.rs        # thin wrappers over OpenSBI
│       └── console.rs    # SBI-backed Console implementation
└── game/                 # no_std roguelike library
    ├── Cargo.toml
    └── src/
        ├── lib.rs        # main loop and rendering
        ├── io.rs         # `Console` trait (the kernel/game contract)
        └── map.rs        # demo map
```

## How it boots

1. QEMU loads **OpenSBI** (its default `-bios`) at `0x8000_0000`.
2. OpenSBI sets up M-mode, drops to S-mode, and jumps to our kernel at
   `0x8020_0000` — the address baked into [`kernel/linker.ld`](kernel/linker.ld).
3. The assembly shim in `kernel/src/main.rs` (`_start`) points `sp` at the
   boot stack reserved by the linker script and calls `kmain`.
4. `kmain` zeroes `.bss`, initialises the heap with
   [`linked_list_allocator`](https://crates.io/crates/linked_list_allocator),
   prints a banner, and hands control to `game::run`.
5. `game::run` takes a `&mut dyn Console` and loops forever — reading a key
   via SBI's legacy `console_getchar`, updating state, redrawing the map
   with ANSI escapes.
6. When the player quits, the kernel asks OpenSBI to shut the machine down
   via the System Reset extension.

## Crates used

| Crate | Why |
| ----- | --- |
| [`sbi-rt`](https://crates.io/crates/sbi-rt) | Talk to OpenSBI (console, shutdown, timers). |
| [`riscv`](https://crates.io/crates/riscv) | Safe wrappers around RISC-V CSRs and instructions (used as traps/timers are added). |
| [`linked_list_allocator`](https://crates.io/crates/linked_list_allocator) | Kernel heap. |
| [`spin`](https://crates.io/crates/spin) | `no_std` mutexes / lazy statics. |
| [`rand_core`](https://crates.io/crates/rand_core) + [`rand_xoshiro`](https://crates.io/crates/rand_xoshiro) | Deterministic PRNG for dungeon generation. |

More will be added as the kernel grows (`fdt` for device-tree parsing,
`uart_16550` or a custom driver for real UARTs, `embedded-hal` for board
portability, etc.).

## Prerequisites

- A recent **Rust nightly** (managed automatically by
  [`rust-toolchain.toml`](rust-toolchain.toml)). `rustup` will install it on
  first build.
- **QEMU** with RISC-V support: `qemu-system-riscv64` on your `PATH`.
  - Windows: grab it from <https://www.qemu.org/download/#windows> or install
    via `choco install qemu` / `scoop install qemu`.
  - macOS: `brew install qemu`.
  - Linux: your distro's `qemu-system-misc` / `qemu-system-riscv64` package.

No external cross-compiler is required — everything goes through `rustc` and
LLVM's built-in RISC-V backend.

## Build & run

From the workspace root:

```powershell
# Build kernel + game for riscv64gc-unknown-none-elf
cargo build -p kernel

# Build and launch in QEMU (uses the runner in .cargo/config.toml)
cargo run -p kernel
```

You should see something like:

```
[riscvrogue] kernel booted
[riscvrogue]   hartid = 0
[riscvrogue]   dtb    = 0x...
[riscvrogue] starting game...

riscvrogue -- a roguelike on bare-metal RISC-V

########################################
#@.....................................#
#....######.........................####
...
Move: h/j/k/l   Quit: q
```

Controls:

- `h` / `j` / `k` / `l` — move west / south / north / east (vi keys).
- `q`, `Ctrl-C`, or `Ctrl-D` — quit. The kernel will then power the
  (virtual) machine off.

To exit QEMU at any time use `Ctrl-A` then `x`. To toggle into the QEMU
monitor use `Ctrl-A` then `c` (and the same shortcut to toggle back).

> Note on Windows: the runner in `.cargo/config.toml` opens stdio as an
> explicit muxed chardev with `signal=off`, which is what makes single
> keystrokes (h/j/k/l/q) reach the guest on `cmd.exe` and PowerShell.
> The simpler `-serial mon:stdio` form works on Linux/macOS but swallows
> input on Windows terminals.

## Debugging with GDB

```powershell
# Terminal 1: launch QEMU paused, waiting for a debugger on :1234
qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 128M -nographic `
    -serial mon:stdio -bios default -s -S `
    -kernel target/riscv64gc-unknown-none-elf/debug/kernel

# Terminal 2:
riscv64-unknown-elf-gdb target/riscv64gc-unknown-none-elf/debug/kernel `
    -ex "target remote :1234" -ex "break kmain" -ex "continue"
```

`rust-gdb` works too if you have a RISC-V-aware GDB behind it.

## Roadmap

Rough order of planned work:

1. **Traps & timers.** Install a real trap vector, handle illegal-instruction
   and timer interrupts, and expose a monotonic clock to the game.
2. **Real UART driver.** Replace the SBI legacy console with a direct
   `uart_16550` (or board-specific) driver. Add input interrupts instead of
   polling.
3. **Device tree.** Parse the FDT that OpenSBI hands us in `a1` so the
   kernel can discover memory, UART, and PLIC addresses at runtime.
4. **Paging.** Turn on Sv39, map the kernel into the higher half, and give
   the game its own address space.
5. **Better game.** Procedurally generated dungeons, monsters, items, line
   of sight, save files (once we have any kind of storage).
6. **Real hardware bring-up.** Start with something well-documented like a
   VisionFive 2 or HiFive Unmatched: new memory map, new UART, confirm
   OpenSBI is still our firmware target.

## License

Dual-licensed under **MIT** or **Apache-2.0** at your option.
