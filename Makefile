BIN=kernel.elf
DISK=kernel.elf

all: $(BIN)

.PHONY: $(BIN)
$(BIN):
	cargo build
	cp target/riscv64-citron/debug/citron $@

# $(DISK):
# 	qemu-img create -f raw $@ 256M
# 	mkfs.fat -n 'DISK' -F 32 $@

qemu: $(BIN) $(DISK)
	qemu-system-riscv64 -machine virt \
	-bios none -kernel $< -m 128M -smp 1 \
	-global virtio-mmio.force-legacy=false \
	-drive file=$(DISK),format=raw,id=hd0 \
	-device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 \
	-device virtio-gpu-device,bus=virtio-mmio-bus.1 \
	-monitor none \
	-serial stdio

qemu-gdb: $(BIN) $(DISK)
	qemu-system-riscv64 -machine virt \
	-bios none -kernel $< -m 128M -smp 1 \
	-nographic \
	-global virtio-mmio.force-legacy=false \
	-drive file=$(DISK),format=raw,id=hd0 \
	-device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 \
	-gdb tcp::1234 -S

clean:
	cargo clean
	rm -rf $(BIN) $(DISK)