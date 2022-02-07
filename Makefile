BIN=kernel.elf
TEST_BIN=
DISK=disk.img
MOUNT=mnt
BUILD=release
MACHINE=virt
ARCH=$(MACHINE).json
SCRIPT=src/linker/$(MACHINE).ld

ifeq ($(shell uname),Linux)
else
ifeq ($(shell uname),Darwin)
else
SHELL       := pwsh.exe
.SHELLFLAGS := -NoProfile -Command
endif
endif

all: $(BIN)

.PHONY: $(BIN)
$(BIN):
	cp $(ARCH) machine.json
	cp $(SCRIPT) machine.ld
ifeq ($(BUILD),release)
	cargo build --release
else
	cargo build
endif
	cp target/machine/$(BUILD)/citron $@

$(DISK):
	make -C bin
	qemu-img create -f raw $@ 256M
	mkfs.fat -n 'DISK' -F 32 $@
ifeq ($(shell uname),Darwin)
	hdiutil attach -mountpoint $(MOUNT) $(DISK)
	cp -r resources mnt/
	cp -r bin mnt/
else
ifeq ($(shell uname),Linux)
	sudo mount -o loop $(DISK) $(MOUNT)
	sudo cp -r resources mnt/
	sudo cp -r bin mnt/
else
endif
endif
ifeq ($(shell uname),Darwin)
	hdiutil detach $(MOUNT)
else
ifeq ($(shell uname),Linux)
	sudo umount $(MOUNT)
else
endif
endif

qemu-aarch64: $(BIN) $(DISK)
	qemu-system-aarch64 -M raspi3 -m 1G -serial null -serial stdio -kernel $(BIN) 
	-drive file=$(DISK),format=raw,if=sd
	
qemu-aarch64-gdb: $(BIN) $(DISK)
	qemu-system-aarch64 -M raspi3 -m 1G -serial null -serial stdio -kernel $(BIN) -drive file=$(DISK),format=raw,if=sd -gdb tcp::1234 -S

ifeq ($(TEST_BIN),)
qemu-riscv64: $(BIN) $(DISK)
	qemu-system-riscv64 -machine virt -bios none -kernel $< -m 256M -smp 4 -global virtio-mmio.force-legacy=false -drive file=$(DISK),format=raw,id=hd0 -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 -device virtio-gpu-device,bus=virtio-mmio-bus.1 -device virtio-mouse-device,bus=virtio-mmio-bus.2 -device virtio-keyboard-device,bus=virtio-mmio-bus.3 -monitor none -serial stdio
else
qemu-riscv64: $(DISK)
	qemu-system-riscv64 -machine virt -bios none -kernel $(TEST_BIN) -m 256M -smp 1 -global virtio-mmio.force-legacy=false -global riscv.sifive.test=true -drive file=$(DISK),format=raw,id=hd0 -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 -device virtio-gpu-device,bus=virtio-mmio-bus.1 -device virtio-mouse-device,bus=virtio-mmio-bus.2 -device virtio-keyboard-device,bus=virtio-mmio-bus.3 -monitor none -serial stdio
endif

ifeq ($(TEST_BIN),)
qemu-riscv64-gdb: $(BIN) $(DISK)
	qemu-system-riscv64 -machine virt -bios none -kernel $< -m 256M -smp 1 -global virtio-mmio.force-legacy=false -serial stdio -drive file=$(DISK),format=raw,id=hd0 -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 -device virtio-gpu-device,bus=virtio-mmio-bus.1 -device virtio-mouse-device,bus=virtio-mmio-bus.2 -device virtio-keyboard-device,bus=virtio-mmio-bus.3 -gdb tcp::1234 -S
else
qemu-riscv64-gdb: $(DISK)
	qemu-system-riscv64 -machine virt -bios none -kernel $(TEST_BIN) -m 256M -smp 1 -global virtio-mmio.force-legacy=false -serial stdio -drive file=$(DISK),format=raw,id=hd0 -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 -device virtio-gpu-device,bus=virtio-mmio-bus.1 -device virtio-mouse-device,bus=virtio-mmio-bus.2 -device virtio-keyboard-device,bus=virtio-mmio-bus.3 -gdb tcp::1234 -S
endif

test:
	cp $(ARCH) machine.json
	cp $(SCRIPT) machine.ld
	cargo test

disk: $(DISK)

clean:
	make -C bin clean
	cargo clean
ifeq ($(shell uname),Darwin)
	rm -rf $(BIN) $(DISK)
else
ifeq ($(shell uname),Linux)
	rm -rf $(BIN) $(DISK)
else
	new-item -Force -Type File _tmp.txt
	-rm -Recurse -Force $(addsuffix $(comma),$(BIN) $(DISK)) _tmp.txt
endif
endif