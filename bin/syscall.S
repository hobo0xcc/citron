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

.globl create_window
create_window:
    mv a6, a5
    mv a5, a4
    mv a4, a3
    mv a3, a2
    mv a2, a1
    mv a1, a0
    li a0, 1000
    ecall
    ret

.globl map_window
map_window:
    mv a2, a1
    mv a1, a0
    li a0, 1001
    ecall
    ret

.globl sync_window
sync_window:
    mv a1, a0
    li a0, 1002
    ecall
    ret

.globl fork
fork:
    li a0, 57
    ecall
    ret

.globl wait_exit
wait_exit:
    li a0, 56
    ecall
    ret

.globl read
read:
    mv a3, a2
    mv a2, a1
    mv a1, a0
    li a0, 0
    ecall
    ret

.globl seek
seek:
    mv a3, a2
    mv a2, a1
    mv a1, a0
    li a0, 2
    ecall
    ret

.globl open
open:
    mv a1, a0
    li a0, 3
    ecall
    ret
