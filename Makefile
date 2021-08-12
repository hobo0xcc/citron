BIN=kernel.elf

all: $(BIN)

.PHONY: $(BIN)
$(BIN):
	cargo build
	cp target/riscv64-citron/debug/citron $@

qemu: $(BIN)
	qemu-system-riscv64 -machine virt \
	-bios none -kernel $< -m 128M -smp 1 \
	-global virtio-mmio.force-legacy=false \
	-nographic \
	-monitor none \
	-serial stdio

qemu-gdb: $(BIN)
	qemu-system-riscv64 -machine virt \
	-bios none -kernel $< -m 128M -smp 1 \
	-nographic \
	-global virtio-mmio.force-legacy=false \
	-gdb tcp::1234 -S

clean:
	cargo clean
	rm -rf $(BIN)