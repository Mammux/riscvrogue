/*
 * Linker script for the riscvrogue kernel on the QEMU `virt` machine.
 *
 * The QEMU `virt` board loads ELF kernels passed via `-kernel` at
 * 0x8020_0000 when OpenSBI is used as the firmware (OpenSBI itself
 * lives at 0x8000_0000 and jumps to 0x8020_0000 in S-mode).
 *
 * This script is intentionally minimal – once we grow a real MMU /
 * paging layer we will likely split this into multiple regions.
 */

OUTPUT_ARCH("riscv")
ENTRY(_start)

MEMORY
{
    RAM (rwx) : ORIGIN = 0x80200000, LENGTH = 126M
}

PHDRS
{
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
}

SECTIONS
{
    . = ORIGIN(RAM);

    .text : ALIGN(4K) {
        KEEP(*(.text.init))
        *(.text .text.*)
    } >RAM :text

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    } >RAM :rodata

    .data : ALIGN(4K) {
        *(.data .data.*)
        *(.sdata .sdata.*)
    } >RAM :data

    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        . = ALIGN(8);
        __bss_end = .;
    } >RAM :bss

    /*
     * Reserve a chunk of RAM for the boot stack and the kernel heap.
     * These symbols are consumed by `main.rs` and the early boot code.
     */
    . = ALIGN(4K);
    __stack_bottom = .;
    . += 64K;
    __stack_top = .;

    . = ALIGN(4K);
    __heap_start = .;
    . += 4M;
    __heap_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.comment)
        *(.note .note.*)
    }
}
