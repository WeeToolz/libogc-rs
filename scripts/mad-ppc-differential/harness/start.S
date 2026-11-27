.section .text.start,"ax"
.globl _start
_start:
    lis 1,0x8170
    li 0,0
    stwu 0,-32(1)
    bl run
.globl finished
finished:
    b finished
