CFLAGS=-ffreestanding -nostdlib -e main
CC=riscv64-unknown-elf-gcc

all: main app mandelbrot

main: main.c syscall.S
	$(CC) -o $@ $^ $(CFLAGS)

app: app.c syscall.S
	$(CC) -o $@ $^ $(CFLAGS)

mandelbrot: mandelbrot.c syscall.S
	$(CC) -o $@ $^ $(CFLAGS)

clean:
	rm -rf main