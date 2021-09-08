ENTRY(_entry)

MEMORY
{
  RAM(wxa) : ORIGIN = 0x80000, LENGTH = 128M
}
 
SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = ORIGIN(RAM);
    __start = .;
    __text_start = .;
    .text :
    {
        KEEP(*(.text.boot))
        *(.text)
    }
    . = ALIGN(4096); /* align to page size */
    __text_end = .;

    __rodata_start = .;
    .rodata :
    {
        *(.srodata .srodata.*)
        . = ALIGN(16);
        *(.rodata .rodata.*)
    }
    . = ALIGN(4096);
    __rodata_end = .;
 
    __data_start = .;
    .data :
    {
        *(.data)
    }
    . = ALIGN(4096); /* align to page size */
    __data_end = .;
 
    __bss_start = .;
    .bss :
    {
        bss = .;
        *(.bss)
    }
    . = ALIGN(4096); /* align to page size */
    __bss_end = .;
    __end = .;

    . = . + 0x1000;
    __stack_end = .;

    PROVIDE(_heap_start = .);
    PROVIDE(_heap_end = ORIGIN(RAM) + LENGTH(RAM));
}
__bss_size = (__bss_end - __bss_start)>>3;