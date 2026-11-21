PHDRS {
    code PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = CARS3_BASE;
    .text : { *(.text*) *(.rodata*) } :code
    . = ALIGN(0x10000);
    .data : { *(.data*) *(.bss*) *(COMMON) } :data
    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
