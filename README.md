# riscvrogue

A minimal, bare-metal operating system for **RISC-V 64** written in **Rust**,
whose one and only job is to host a small **text-mode roguelike** — also
written in Rust. The project's initial target is the QEMU `virt` machine; the
long-term goal is to boot the same kernel on real RISC-V hardware (e.g. a
VisionFive 2, a Milk-V Duo/Mars, or a SiFive HiFive board) with as few
platform-specific changes as possible.

> Status: **early prototype.** The kernel boots, initializes a VirtIO GPU
> framebuffer and VirtIO keyboard device, then runs the roguelike in QEMU's
> graphical window. SBI serial is kept as a debug/fallback console.

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
- A full filesystem or TCP/IP stack.
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
│       ├── console.rs    # SBI-backed Console implementation
│       └── gfx_console.rs# VirtIO GPU+input framebuffer Console
└── game/                 # no_std roguelike library
    ├── Cargo.toml
    └── src/
   ├── lib.rs        # demo game + exports (`prelude`)
   ├── engine.rs     # bracket-like API (`BTerm`, `GameState`, loop)
   ├── input.rs      # key decoding into game actions
        ├── io.rs         # `Console` trait (the kernel/game contract)
        └── map.rs        # demo map
```

## `game` API style

The `game` crate now exposes a tiny **bracket-like** API while staying
`no_std`:

- `game::engine::BTerm` – console context (`cls`, `print`, `put_char`, `key`).
- `game::engine::GameState` – trait with `tick(&mut self, &mut BTerm)`.
- `game::engine::main_loop` – runs a `GameState` until `TickResult::Quit`.
- `game::dungeon::DungeonState` – procedural rooms-and-corridors sample state.
- `game::prelude` – convenient re-exports for new game states.

The current walking demo is implemented through this API, so you can replace
`DemoState` with your own state machine incrementally.

There are now two entrypoints from the `game` crate:

- `game::run` – original fixed demo map.
- `game::run_dungeon` – procedural dungeon sample state.
- `game::run_dungeon_with_seed` – same dungeon mode with explicit RNG seed.

The kernel currently calls `game::run_dungeon_with_seed` by default, using a
boot-time hardware counter mixed with `hartid`/`dtb` for per-boot variation.

## How it boots

1. QEMU loads **OpenSBI** (its default `-bios`) at `0x8000_0000`.
2. OpenSBI sets up M-mode, drops to S-mode, and jumps to our kernel at
   `0x8020_0000` — the address baked into [`kernel/linker.ld`](kernel/linker.ld).
3. The assembly shim in `kernel/src/main.rs` (`_start`) points `sp` at the
   boot stack reserved by the linker script and calls `kmain`.
4. `kmain` zeroes `.bss`, initialises the heap with
   [`linked_list_allocator`](https://crates.io/crates/linked_list_allocator),
   prints a banner on SBI serial, then probes VirtIO-MMIO for GPU + keyboard.
5. If VirtIO init succeeds, `game::run` receives a framebuffer-backed
   `Console`: ANSI bytes are interpreted by a tiny in-kernel parser and drawn
   via `embedded-graphics` + IBM437 glyphs into a `640x400` ARGB framebuffer.
6. Keyboard input is read from `virtio-input` evdev events and translated to
   ASCII for the game loop.
7. If VirtIO init fails, the kernel falls back to the SBI serial console.
8. When the player quits, the kernel asks OpenSBI to shut the machine down
   via the System Reset extension.

## Crates used

| Crate | Why |
| ----- | --- |
| [`sbi-rt`](https://crates.io/crates/sbi-rt) | Talk to OpenSBI (console, shutdown, timers). |
| [`riscv`](https://crates.io/crates/riscv) | Safe wrappers around RISC-V CSRs and instructions (used as traps/timers are added). |
| [`linked_list_allocator`](https://crates.io/crates/linked_list_allocator) | Kernel heap. |
| [`spin`](https://crates.io/crates/spin) | `no_std` mutexes / lazy statics. |
| [`virtio-drivers`](https://crates.io/crates/virtio-drivers) | VirtIO MMIO transports + GPU/input drivers. |
| [`embedded-graphics`](https://crates.io/crates/embedded-graphics) | Rendering text glyphs into the framebuffer. |
| [`ibm437`](https://crates.io/crates/ibm437) | 8x8 CP437 bitmap font used by the text console. |
| [`rand_core`](https://crates.io/crates/rand_core) + [`rand_xoshiro`](https://crates.io/crates/rand_xoshiro) | Deterministic PRNG for dungeon generation. |

More will be added as the kernel grows (`fdt` for device-tree parsing,
`uart_16550` or a custom driver for real UARTs, interrupt/PLIC support,
paging, etc.).

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

You should see:

- A QEMU graphical window that renders the roguelike map.
- SBI debug lines in the launching terminal, similar to:

```
[riscvrogue] kernel booted
[riscvrogue]   hartid = 0
[riscvrogue]   dtb    = 0x...
[riscvrogue] framebuffer console online
[riscvrogue] starting game...
```

Controls:

- Click the QEMU window once so it captures keyboard focus.
- `h` / `j` / `k` / `l` **or arrow keys** — cardinal movement.
- Numeric keypad movement is enabled, including diagonals:
   - `7`/`9` = up-left/up-right, `1`/`3` = down-left/down-right
   - `8`/`2` = up/down, `4`/`6` = left/right
- `O` — open/close in-game options menu.
   - `F` cycles fonts (`IBM437 8x8 regular`, `IBM437 8x8 bold`, `IBM437 9x14 regular`).
   - `G` toggles graphical tiles for walls/corridors/floors.
   - `C` toggles color on/off.
- `q` — quit. The kernel then powers the VM off via SBI.

To exit QEMU at any time use `Ctrl-A` then `x`. To toggle into the QEMU
monitor use `Ctrl-A` then `c` (and the same shortcut to toggle back).

> Input now comes from `virtio-keyboard-device` in the QEMU window, not from
> host terminal stdin. The terminal serial console remains for logs, monitor,
> and panic output.

## Debugging with GDB

```powershell
# Terminal 1: launch QEMU paused, waiting for a debugger on :1234
qemu-system-riscv64 -machine virt -cpu rv64 -smp 1 -m 128M -nographic `
   -bios default -device virtio-gpu-device -device virtio-keyboard-device `
   -serial mon:stdio -s -S `
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
