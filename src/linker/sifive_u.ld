OUTPUT_ARCH ( "riscv" )
ENTRY( _entry )

MEMORY
{
    RAM(wxa) : ORIGIN = 0x80000000, LENGTH = 128M
}

SECTIONS
{
  . = ORIGIN(RAM);

  .reset : {
      KEEP(*(.reset.boot));
  }

  .text : {
    PROVIDE(_text_start = .);
    *(.text .text.*)
    . = ALIGN(0x1000);
    PROVIDE(_text_end = .);
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
  . = . + 0x16000;
  PROVIDE(_stack_end = .);

  PROVIDE(_heap_start = .);
  PROVIDE(_heap_end = ORIGIN(RAM) + LENGTH(RAM));

  /* from qemu/hw/riscv/sifive_u.c
  static const struct MemmapEntry {
      hwaddr base;
      hwaddr size;
  } sifive_u_memmap[] = {
      [SIFIVE_U_DEBUG] =    {        0x0,      0x100 },
      [SIFIVE_U_MROM] =     {     0x1000,    0x11000 },
      [SIFIVE_U_CLINT] =    {  0x2000000,    0x10000 },
      [SIFIVE_U_L2LIM] =    {  0x8000000,  0x2000000 },
      [SIFIVE_U_PLIC] =     {  0xc000000,  0x4000000 },
      [SIFIVE_U_PRCI] =     { 0x10000000,     0x1000 },
      [SIFIVE_U_UART0] =    { 0x10010000,     0x1000 },
      [SIFIVE_U_UART1] =    { 0x10011000,     0x1000 },
      [SIFIVE_U_OTP] =      { 0x10070000,     0x1000 },
      [SIFIVE_U_FLASH0] =   { 0x20000000, 0x10000000 },
      [SIFIVE_U_DRAM] =     { 0x80000000,        0x0 },
      [SIFIVE_U_GEM] =      { 0x10090000,     0x2000 },
      [SIFIVE_U_GEM_MGMT] = { 0x100a0000,     0x1000 },
  };
  */

  . = 0x2000000;
  PROVIDE(_clint_start = .);
  . = . + 0x10000;
  PROVIDE(_clint_end = .);

  . = 0xc000000;
  PROVIDE(_plic_start = .);
  . = . + 0x4000000;
  PROVIDE(_plic_end = .);

  . = 0x10010000;
  PROVIDE(_uart0_start = .);
  . = . + 0x1000;
  PROVIDE(_uart0_end = .);

  . = 0xfffff000;
  PROVIDE(_virtio_start = .);
  PROVIDE(_virtio_end = .);
}
