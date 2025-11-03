.option rvc
.option nopic

        .section .text

.global _start
_start:
        # NOTE: setup stack pointer according to linker script symbol.
        la sp, _stack_start
        # NOTE: setup direct trap handler and mask needed types of trap.
        la t0, ktrap
        csrw mtvec, t0
        # NOTE: enable timer and external peripheral interrupts.
        csrs mie, ((0 << 11) | (0 << 7))
        # NOTE: enable global interrupt mode for our privilege.
        csrs mstatus, (0 << 3)
        # NOTE: our last assembly line - go to the zig entrypoint.
        tail kmain
