ENTRY(_start)
PHDRS { text PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
  . = 0x80004000;
  .text : { KEEP(*(.text.start)) *(.text .text.*) } :text
  .rodata : { *(.rodata .rodata.*) *(.srodata .srodata.*) } :text
  . = ALIGN(32);
  .data : { *(.data .data.*) *(.sdata .sdata.*) *(.got .got.*) } :data
  . = ALIGN(32);
  .bss (NOLOAD) : { *(.bss .bss.*) *(.sbss .sbss.*) *(COMMON) } :data
  /DISCARD/ : { *(.eh_frame*) *(.comment) }
}
