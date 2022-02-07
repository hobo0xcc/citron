$BIN = $Args[1]
$DISK = "disk.img"

qemu-system-riscv64 -machine virt -bios none -kernel $BIN -m "256M" -smp 1 -global virtio-mmio.force-legacy=false -serial stdio -drive file=$DISK,format=raw,id=hd0 -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.0 -device virtio-gpu-device,bus=virtio-mmio-bus.1 -device virtio-mouse-device,bus=virtio-mmio-bus.2 -device virtio-keyboard-device,bus=virtio-mmio-bus.3