MEMORY
{
  RAM (!r): ORIGIN = 0x05000000, LENGTH = 64k
  ROM (rx): ORIGIN = 0x07000000, LENGTH = 16M
}

ENTRY(__handle_reset);

PROVIDE(vb_game_pad_handler = default_handler);
PROVIDE(vb_timer_handler = default_handler);
PROVIDE(vb_game_pak_handler = default_handler);
PROVIDE(vb_communication_handler = default_handler);
PROVIDE(vb_vip_handler = default_handler);
PROVIDE(vb_fp_exception_handler = default_handler);
PROVIDE(vb_divide_by_zero_handler = default_handler);
PROVIDE(vb_illegal_opcode_handler = default_handler);
PROVIDE(vb_lo_trap_handler = default_handler);
PROVIDE(vb_hi_trap_handler = default_handler);
PROVIDE(vb_address_trap_handler = default_handler);
PROVIDE(vb_duplexed_exception_handler = default_handler);

SECTIONS
{
  .text ORIGIN(ROM) :
  {
    *(.text)
    *(.text.*)
  } >ROM

  .rodata : {
    *(.rodata)
    *(.rodata.*)
  } >ROM

  _data_lma = .;
  .data ORIGIN(RAM) : AT(_data_lma)
  {
    _data_start = .;
    *(.data)
    *(.data.*)
    *(.sdata)
    _data_end = .;
  } >RAM

  .bss :
  {
    _bss_start = .;
    *(.bss)
    *(.bss.*)
    *(.sbss)
    _bss_end = .;
  } >RAM

  __gp = ORIGIN(RAM) + (LENGTH(RAM) / 2);

  __sections_size = SIZEOF(.text) + SIZEOF(.rodata) + SIZEOF(.data);
  __rom_size = 1 << LOG2CEIL(__sections_size + 0x220);
  __rom_header_start = ORIGIN(ROM) + __rom_size - 0x220;

  .rom_header 0x07FFFDE0 : AT(__rom_header_start) {
    KEEP (*(.rom_header))
  } >ROM

  .handlers 0x07FFFE00 : AT(__rom_header_start + 0x20) {
    KEEP (*(.handlers.vb_game_pad_handler))
    KEEP (*(.handlers.vb_timer_handler))
    KEEP (*(.handlers.vb_game_pak_handler))
    KEEP (*(.handlers.vb_communication_handler))
    KEEP (*(.handlers.vb_vip_handler))
    . = . + 0x110;
    KEEP (*(.handlers.vb_fp_exception_handler))
    . = . + 0x10;
    KEEP (*(.handlers.vb_divide_by_zero_handler))
    KEEP (*(.handlers.vb_illegal_opcode_handler))
    KEEP (*(.handlers.vb_lo_trap_handler))
    KEEP (*(.handlers.vb_hi_trap_handler))
    KEEP (*(.handlers.vb_address_trap_handler))
    KEEP (*(.handlers.vb_duplexed_exception_handler))
    . = . + 0x10;
    KEEP (*(.handlers.reset))
  } >ROM =0x008a008a /* fill unset interrupts with "loop forever" */
}