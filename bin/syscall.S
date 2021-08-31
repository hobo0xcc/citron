.globl write
write:
    mv a3, a2
    mv a2, a1
    mv a1, a0
    li a0, 1
    ecall
    ret

.globl sleep
sleep:
    mv a1, a0
    li a0, 35
    ecall
    ret