use super::virtio::virtio_gpu::*;
use crate::graphics::Painter;

impl Painter for VirtioGpu {
    #[inline(always)]
    fn draw_at(&mut self, x: u32, y: u32, pixel: u32) {
        unsafe {
            (self.framebuffer as *mut u32)
                .add((y * self.width + x) as usize)
                .write(pixel);
        }
    }

    fn copy_buf(&mut self, src: *mut u32, size: usize) {
        unsafe {
            core::ptr::copy(src, self.framebuffer as *mut u32, size);
        }
    }

    fn flush(&mut self) {
        self.update_display();
    }

    fn get_width(&self) -> u32 {
        self.width
    }

    fn get_height(&self) -> u32 {
        self.height
    }
}
