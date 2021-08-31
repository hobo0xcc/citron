CFLAGS=-ffreestanding -nostdlib -e main
CC=riscv64-unknown-elf-gcc

main: main.c syscall.S
	$(CC) -o $@ $^ $(CFLAGS)

clean:
	rm -rf main