/*
 * DESCRIPTION
 *
 *     * `MEMORY` - names hardware places that exist on board: the lowest piece here.
 *     * `SECTIONS` - names software places, mapped to `MEMORY`, that exist in program.
 *     * `PHDRS` - names segments, mapped to `MEMORY`, specifically for ELF bootloaders.
*/

OUTPUT_ARCH(riscv)
OUTPUT_FORMAT(elf32-littleriscv)

ENTRY(_start)

/*
 * DESCRIPTION
 *
 *     All memory sections have a prefix in their names:
 *
 *         * "I" - instruction path.
 *         * "D" - instruction path.
 *         * "ID"- a unipath, both.
 *
 *     See ESP32-C3 Technical Reference Manual
 *     > Chapter "System and Memory"
 *     > Figure 3.2-1 "System Structure and Address Mapping"
 *     to understand the `ORIGIN`s addresses.
*/
MEMORY
{
  IFLSH (rx) : ORIGIN = 0x42000000, LENGTH = 4M
  DFLSH ( r) : ORIGIN = 0x3C000000, LENGTH = 4M
  ISRAM (rx) : ORIGIN = 0x4037C000, LENGTH = 400K
  DSRAM (rw) : ORIGIN = 0x3FC80000, LENGTH = 384K
  IDRTC (rw) : ORIGIN = 0x50000000, LENGTH = 8K
}

PHDRS
{
  text   PT_LOAD FLAGS(5 /* PF_RX */);
  rodata PT_LOAD FLAGS(4 /* PF_RO */);
  data   PT_LOAD FLAGS(6 /* PF_RW */);
}

PROVIDE(MEDLOW_VMA_MAX_ADDR = 0x80000000);

SECTIONS
{
  .text : ALIGN(16)
    {
      PROVIDE(_text_start = .);
      *(.text .text.*)
      PROVIDE(_text_end = .);
    } >IFLSH AT>IFLSH :text
  ASSERT(ADDR(.text) < MEDLOW_VMA_MAX_ADDR, ".text section out of .medlow code model range")

  .rodata : ALIGN(8)
    {
      PROVIDE(_rodata_start = .);
      *(.rodata .rodata.*)
      PROVIDE(_rodata_end = .);
    } >DFLSH AT>DFLSH :rodata
  ASSERT(ADDR(.rodata) < MEDLOW_VMA_MAX_ADDR, ".rodata section out of .medlow code model range")

  .data : ALIGN(8)
    {
      PROVIDE(_data_start = .);
      *(.data .data.*)
      PROVIDE(_data_end = .);
    } >DSRAM AT>DFLSH :data
  ASSERT(ADDR(.data) < MEDLOW_VMA_MAX_ADDR, ".data section out of .medlow code model range")

  PROVIDE(_stack_size = 4096);
  .stack (NOLOAD): ALIGN(8)
    {
      PROVIDE(_stack_start = .);
      . = ORIGIN(DSRAM) + LENGTH(DSRAM);
      PROVIDE(_stack_end = .);
    } >DSRAM AT>DSRAM
  ASSERT(ADDR(.stack) < MEDLOW_VMA_MAX_ADDR, ".stack section out of .medlow code model range")
  ASSERT(ADDR(.stack) + _stack_size <= ORIGIN(DSRAM) + LENGTH(DSRAM), "DSRAM overflow")

  .rtc (NOLOAD) : ALIGN(8)
    {
      PROVIDE(_rtc_start = .);
      *(.rtc .rtc.*)
      PROVIDE(_rtc_end = .);
    } >IDRTC AT>IDRTC
  ASSERT(ADDR(.rtc) < MEDLOW_VMA_MAX_ADDR, ".rtc section out of .medlow code model range")
}
