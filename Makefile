BIN=kernel.elf
DISK=disk.img
MOUNT=mnt
BUILD=release

all: $(BIN)

.PHONY: $(BIN)
$(BIN):
ifeq ($(BUILD),release)
	cargo build --release
else
	cargo build
endif
	cp target/riscv64-citron/$(BUILD)/citron $@

$(DISK):
	make -C bin
	qemu-img create -f raw $@ 256M
	mkfs.fat -n 'DISK' -F 32 $@
ifeq ($(shell uname),Darwin)
	hdiutil attach -mountpoint $(MOUNT) $(DISK)
else
	mount -o loop $(DISK) $(MOUNT)
endif
	cp -r resources mnt/
	cp -r bin mnt/
ifeq ($(shell uname),Darwin)
	hdiutil detach $(MOUNT)
else
	umount $(MOUNT)
endif

qemu: $(BIN) $(DISK)
	qemu-system-riscv64 -machine virt \
	-bios none -kernel $< -m 128M -smp 1 \
	-global virtio-mmio.force-legacy=false \
	-drive file=$(DISK),format=raw,id=hd0 \
	-device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 \
	-device virtio-gpu-device,bus=virtio-mmio-bus.1 \
	-device virtio-mouse-device,bus=virtio-mmio-bus.2 \
	-device virtio-keyboard-device,bus=virtio-mmio-bus.3 \
	-monitor none \
	-serial stdio

qemu-gdb: $(BIN) $(DISK)
	qemu-system-riscv64 -machine virt \
	-bios none -kernel $< -m 128M -smp 1 \
	-global virtio-mmio.force-legacy=false \
	-serial stdio \
	-drive file=$(DISK),format=raw,id=hd0 \
	-device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 \
	-device virtio-gpu-device,bus=virtio-mmio-bus.1 \
	-device virtio-mouse-device,bus=virtio-mmio-bus.2 \
	-device virtio-keyboard-device,bus=virtio-mmio-bus.3 \
	-gdb tcp::1234 -S

clean:
	make -C bin clean
	cargo clean
	rm -rf $(BIN) $(DISK)