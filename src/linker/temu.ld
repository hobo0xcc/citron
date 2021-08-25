OUTPUT_ARCH( "riscv" )
ENTRY( _entry )

MEMORY
{
  RAM(wxa) : ORIGIN = 0x80000000, LENGTH = 128M
}

SECTIONS
{
  . = ORIGIN(RAM);

  .text : {
    PROVIDE(_text_start = .);
    *(.text .text.*)
    . = ALIGN(0x1000);
    PROVIDE(_text_end = .);
    _trampoline = .;
    *(trampsec)
    . = ALIGN(0x1000);
    ASSERT(. - _trampoline == 0x1000, "error: trampoline larger than one page");
    *(killmesec)
    . = ALIGN(0x1000);
  }

  .rodata : {
    PROVIDE(_rodata_start = .);
    . = ALIGN(16);
    *(.srodata .srodata.*)
    . = ALIGN(16);
    *(.rodata .rodata.*)
    . = ALIGN(0x1000);
    PROVIDE(_rodata_end = .);
  }

  .data : {
    PROVIDE(_data_start = .);
    . = ALIGN(16);
    *(.sdata .sdata.*)
    . = ALIGN(16);
    *(.data .data.*)
    . = ALIGN(0x1000);
    PROVIDE(_data_end = .);
  }

  .bss : {
    PROVIDE(_bss_start = .);
    . = ALIGN(16);
    *(.sbss .sbss.*)
    . = ALIGN(16);
    *(.bss .bss.*)
    . = ALIGN(0x1000);
    PROVIDE(_bss_end = .);
  }

  PROVIDE(_stack_start = .);
  . = . + 0x40000;
  PROVIDE(_stack_end = .);

  PROVIDE(_heap_start = .);
  PROVIDE(_heap_end = ORIGIN(RAM) + LENGTH(RAM));

  /* from  tinyemu/riscv_machine.c */

  /* Peripherals */
  . = 0x02000000;
  PROVIDE(_clint_start = .);
  . = . + 0x000c0000;
  PROVIDE(_clint_end = .);

  . = 0x40100000;
  PROVIDE(_plic_start = .);
  . = . + 0x00400000;
  PROVIDE(_plic_end = .);

  . = 0x10000000;
  PROVIDE(_uart0_start = .);
  . = . + 0x100;
  PROVIDE(_uart0_end = .);

  . = 0x40010000;
  PROVIDE(_virtio_start = .);
  . = . + 0x1000;
  PROVIDE(_virtio_end = .);
}

