ENTRY(_start)

__stack_top = 0x81800000;

SECTIONS
{
  . = 0x80004000;

  .init : ALIGN(4)
  {
    KEEP(*(.init))
    KEEP(*(.init.*))
  }

  .text : ALIGN(4)
  {
    *(.text .text.*)
    *(.gnu.linkonce.t.*)
  }

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*)
    *(.gnu.linkonce.r.*)
    *(.srodata .srodata.*)
    *(.eh_frame_hdr)
    *(.eh_frame)
    *(.gcc_except_table .gcc_except_table.*)
  }

  .data : ALIGN(4)
  {
    *(.data .data.*)
    *(.gnu.linkonce.d.*)
    *(.ctors .ctors.*)
    *(.dtors .dtors.*)
    *(.init_array .init_array.*)
    *(.fini_array .fini_array.*)
    *(.sdata .sdata.*)
    *(.got .got.*)
  }

  .bss (NOLOAD) : ALIGN(8)
  {
    __bss_start = .;
    *(.sbss .sbss.*)
    *(.bss .bss.*)
    *(COMMON)
    __bss_end = .;
  }

  /* Match devkitPPC's Wii linker-script IPC arena. */
  __ipcbufferLo = 0x933e0000;
  __ipcbufferHi = 0x93400000;
}
