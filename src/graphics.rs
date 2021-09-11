use crate::arch::target::virtio::virtio_gpu::*;
use crate::arch::target::virtio::*;
use crate::*;
use alloc::collections::BTreeMap;
use alloc::{
    alloc::alloc_zeroed,
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::alloc::Layout;
use core::any::Any;
use core::cmp::{max, min};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::{ascii::*, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::*;
use embedded_graphics::text::*;
use spin::Mutex;
use tinybmp::Bmp;

#[cfg(target_arch = "riscv64")]
pub static mut LAYER_MANAGER: Option<LayerManager<Mutex<VirtioGpu>>> = None;
pub static mut WINDOW_MANAGER: Option<WindowManager> = None;
pub static mut MOUSE_LAYER_ID: usize = 0;
pub static mut DESKTOP_LAYER_ID: usize = 0;
pub static mut OBJECT_ARENA: Option<ObjectArena> = None;

static WALLPAPER: &[u8] = include_bytes!("../resources/wallpaper.bmp") as &[u8];
static MOUSE_CURSOR: [&str; 23] = [
    "................",
    "................",
    "..#.............",
    "..##............",
    "..#=#...........",
    "..#==#..........",
    "..#===#.........",
    "..#====#........",
    "..#=====#.......",
    "..#======#......",
    "..#=======#.....",
    "..#========#....",
    "..#=========#...",
    "..#==========#..",
    "..#======#####..",
    "..#===#==#......",
    "..#==#.#==#.....",
    "..#=#..#==#.....",
    "..##....#==#....",
    "........#==#....",
    ".........##.....",
    "................",
    "................",
];

pub type ObjectId = usize;
pub type LayerId = usize;

pub struct ObjectArena {
    map: BTreeMap<ObjectId, Box<dyn Object>>,
    curr_id: ObjectId,
}

impl ObjectArena {
    pub fn new() -> Self {
        ObjectArena {
            map: BTreeMap::new(),
            curr_id: 0,
        }
    }

    pub fn alloc(&mut self, obj: Box<dyn Object>) -> ObjectId {
        let id = self.curr_id;
        self.curr_id += 1;
        self.map.insert(id, obj);
        id
    }

    pub fn get(&self, id: ObjectId) -> Option<&Box<dyn Object>> {
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Box<dyn Object>> {
        self.map.get_mut(&id)
    }

    pub fn remove(&mut self, id: ObjectId) -> Option<Box<dyn Object>> {
        let obj = self.map.remove(&id);
        if let Some(obj) = obj {
            Some(obj)
        } else {
            None
        }
    }
}

pub trait Painter {
    fn draw_at(&mut self, x: u32, y: u32, pixel: u32);
    fn copy_buf(&mut self, src: *mut u32, src_offset: usize, dst_offset: usize, size: usize);
    fn flush(&mut self);
    fn get_width(&self) -> u32;
    fn get_height(&self) -> u32;
}

impl<T: Painter> Painter for Mutex<T> {
    fn draw_at(&mut self, x: u32, y: u32, pixel: u32) {
        self.lock().draw_at(x, y, pixel);
    }

    fn copy_buf(&mut self, src: *mut u32, src_offset: usize, dst_offset: usize, size: usize) {
        self.lock().copy_buf(src, src_offset, dst_offset, size);
    }

    fn flush(&mut self) {
        self.lock().flush();
    }

    fn get_width(&self) -> u32 {
        self.lock().get_width()
    }

    fn get_height(&self) -> u32 {
        self.lock().get_height()
    }
}

#[derive(Copy, Clone)]
pub enum ObjectEvent {
    MouseLeftPress(u32, u32),
    MouseLeftRelease(u32, u32),
}

pub struct Mouse {
    transparent: u32,
}

impl Mouse {
    pub fn new(transparent: u32) -> Self {
        Mouse { transparent }
    }
}

impl Object for Mouse {
    fn draw_to(&mut self, buffer: &mut FrameBuffer, x: u32, y: u32) {
        let width = buffer.get_width();
        let height = buffer.get_height();
        for (ty, s) in MOUSE_CURSOR.iter().enumerate() {
            for (tx, ch) in s.chars().enumerate() {
                if x + tx as u32 >= width || y + ty as u32 >= height {
                    continue;
                }
                match ch {
                    '.' => buffer.draw_at(tx as u32, ty as u32, self.transparent),
                    '#' => buffer.draw_at(tx as u32, ty as u32, 0xff000000),
                    '=' => buffer.draw_at(tx as u32, ty as u32, 0xffffffff),
                    _ => {}
                }
            }
        }
    }

    fn on_event(&mut self, _event: ObjectEvent, _layer_id: LayerId) {
        // nothing to do
    }

    fn get_width(&self) -> u32 {
        16
    }

    fn get_height(&self) -> u32 {
        23
    }
}

#[allow(dead_code)]
pub struct WindowFrame {
    buffer: *mut u8,
    width: u32,
    height: u32,
}

pub struct Window {
    frame: FrameBuffer,
    title: String,
    title_bar_height: u32,
}

impl Window {
    pub fn new(width: u32, height: u32) -> Self {
        let title_bar_height = 30;
        let window_frame_width = width;
        let window_frame_height = height;
        Window {
            frame: FrameBuffer::new(window_frame_width, window_frame_height),
            title: String::new(),
            title_bar_height,
        }
    }

    pub fn draw_window(&mut self, buffer: &mut FrameBuffer) {
        let width = buffer.width;
        let height = buffer.height;

        // window background
        let bg_style = PrimitiveStyleBuilder::new()
            .fill_color(Rgb888::new(0xf0, 0xef, 0xef))
            .build();
        Rectangle::new(Point::new(0, 0), Size::new(width, height))
            .into_styled(bg_style)
            .draw(buffer)
            .expect("draw");

        // window title bar
        self.draw_title_bar(buffer);
    }

    pub fn draw_title_bar(&self, buffer: &mut FrameBuffer) {
        let width = buffer.width;
        let _height = buffer.height;

        let color = Rgb888::new(0x80, 0x80, 0x80);

        let title_bar_style = PrimitiveStyleBuilder::new().fill_color(color).build();
        Rectangle::new(Point::new(0, 0), Size::new(width, self.title_bar_height))
            .into_styled(title_bar_style)
            .draw(buffer)
            .expect("draw");

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(Rgb888::WHITE)
            .build();
        Text::new(self.title.as_str(), Point::new(10, 20), text_style)
            .draw(buffer)
            .expect("draw");
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn get_frame(&self) -> FrameBuffer {
        self.frame
    }

    pub fn update_frame(&mut self, buffer: &mut FrameBuffer) {
        for y in 0..self.frame.height {
            for x in 0..self.frame.width {
                let pixel = self.frame.get_pixel(x, y);
                buffer.draw_at(x, y + self.title_bar_height, pixel);
            }
        }
    }
}

impl Object for Window {
    fn draw_to(&mut self, buffer: &mut FrameBuffer, _x: u32, _y: u32) {
        self.draw_window(buffer);
    }

    fn on_event(&mut self, event: ObjectEvent, layer_id: LayerId) {
        match event {
            ObjectEvent::MouseLeftPress(_x, _y) => {
                let lm = unsafe { layer_manager() };
                let mouse_layer_id = unsafe { MOUSE_LAYER_ID };

                if lm.get_layer_height(layer_id) == lm.get_layer_height(mouse_layer_id) - 1 {
                    return;
                }

                lm.move_layer(layer_id, lm.get_layer_height(mouse_layer_id) as i32 - 1);
                lm.update(layer_id);
            }
            ObjectEvent::MouseLeftRelease(x, y) => {
                let lm = unsafe { layer_manager() };
                let prev_x = lm.get_layer_x(layer_id) as i32;
                let prev_y = lm.get_layer_y(layer_id) as i32;
                let next_x = x as i32 - (lm.event_info.pressed_x as i32 - prev_x);
                let next_y = y as i32 - (lm.event_info.pressed_y as i32 - prev_y);
                lm.move_abs(layer_id, next_x, next_y);
                lm.update(layer_id);
            }
        }
    }

    fn get_width(&self) -> u32 {
        self.frame.width
    }

    fn get_height(&self) -> u32 {
        self.frame.height
    }
}

#[allow(dead_code)]
pub struct Desktop {
    bg_color: u32,
    width: u32,
    height: u32,
    buffer: *mut u32,
}

impl Desktop {
    pub fn new(bg_color: u32, width: u32, height: u32) -> Self {
        let size = (width * 4 * height) as usize;
        let layout = Layout::from_size_align(size, 0x1000).unwrap();
        let buffer = unsafe { alloc_zeroed(layout) } as *mut u32;
        for x in 0..width {
            for y in 0..height {
                unsafe {
                    buffer.add((y * width + x) as usize).write(bg_color);
                }
            }
        }
        Desktop {
            bg_color,
            width,
            height,
            buffer,
        }
    }
}

impl Object for Desktop {
    fn draw_to(&mut self, buffer: &mut FrameBuffer, _x: u32, _y: u32) {
        let _size = (self.width * self.height) as usize;
        let bmp = Bmp::<Rgb888>::from_slice(WALLPAPER).unwrap();
        Image::new(&bmp, Point::new(0, 0))
            .draw(buffer)
            .expect("draw");
        // buffer.copy_buf(self.buffer, size);
        // for x in 0..self.width {
        //     for y in 0..self.height {
        //         painter.draw_at(x, y, self.bg_color);
        //     }
        // }
    }

    fn on_event(&mut self, _event: ObjectEvent, _layer_id: LayerId) {}

    fn get_width(&self) -> u32 {
        self.width
    }

    fn get_height(&self) -> u32 {
        self.height
    }
}

pub trait AToAny: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

impl<T: 'static> AToAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait Object: AToAny {
    fn draw_to(&mut self, buffer: &mut FrameBuffer, x: u32, y: u32);
    fn on_event(&mut self, event: ObjectEvent, layer_id: LayerId);
    fn get_width(&self) -> u32;
    fn get_height(&self) -> u32;
}

#[derive(Copy, Clone)]
pub struct FrameBuffer {
    pub buffer: *mut u32,
    pub width: u32,
    pub height: u32,
}

impl Painter for FrameBuffer {
    fn draw_at(&mut self, x: u32, y: u32, pixel: u32) {
        let index = (y * self.width + x) as usize;
        unsafe {
            self.buffer.add(index).write(pixel);
        }
    }

    fn copy_buf(&mut self, src: *mut u32, src_offset: usize, dst_offset: usize, size: usize) {
        unsafe {
            core::ptr::copy(src.add(src_offset), (self.buffer).add(dst_offset), size);
        }
    }

    fn flush(&mut self) {}

    fn get_width(&self) -> u32 {
        self.width
    }

    fn get_height(&self) -> u32 {
        self.height
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb888;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let width = self.width as i32;
        let height = self.height as i32;
        for pixel in pixels.into_iter() {
            let Pixel(point, color) = pixel;
            let (x, y) = (point.x, point.y);
            if x >= width || y >= height || x < 0 || y < 0 {
                continue;
            }
            let mut pixel_word: u32 = 0xff000000;
            for (i, byte) in color.to_le_bytes().iter().enumerate() {
                pixel_word |= ((*byte as u32) << (i * 8)) as u32;
            }
            self.draw_at(x as u32, y as u32, pixel_word);
        }

        Ok(())
    }
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * 4 * height) as usize;
        let layout = Layout::from_size_align(size, 0x1000).unwrap();
        let buffer = unsafe { alloc_zeroed(layout) } as *mut u32;
        FrameBuffer {
            buffer,
            width,
            height,
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        unsafe { self.buffer.add((y * self.width + x) as usize).read() }
    }
}

#[allow(dead_code)]
pub struct Layer {
    id: LayerId,
    object: ObjectId,
    pub buffer: FrameBuffer,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    prev_x: u32,
    prev_y: u32,
    prev_width: u32,
    prev_height: u32,
    display_width: u32,
    display_height: u32,
    transparent: Option<u32>,
}

impl Layer {
    pub fn new(
        id: LayerId,
        object: ObjectId,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        display_width: u32,
        display_height: u32,
    ) -> Self {
        Layer {
            id,
            object,
            buffer: FrameBuffer::new(width, height),
            x,
            y,
            width,
            height,
            prev_x: 0,
            prev_y: 0,
            prev_width: 0,
            prev_height: 0,
            display_width,
            display_height,
            transparent: None,
        }
    }

    pub fn set_transparent_color(&mut self, color: u32) {
        self.transparent = Some(color);
    }

    pub fn move_abs(&mut self, x: i32, y: i32) {
        let mut new_x = x;
        let mut new_y = y;
        let width = self.display_width as i32;
        let height = self.display_height as i32;
        if new_x < 0 {
            new_x = 0;
        } else if new_x > width {
            new_x = width;
        }
        if new_y < 0 {
            new_y = 0;
        } else if new_y > height {
            new_y = height;
        }
        self.x = new_x as u32;
        self.y = new_y as u32;
    }

    pub fn move_rel(&mut self, x: i32, y: i32) {
        let mut new_x = self.x as i32 + x;
        let mut new_y = self.y as i32 + y;
        let width = self.display_width as i32;
        let height = self.display_height as i32;
        if new_x < 0 {
            new_x = 0;
        } else if new_x > width {
            new_x = width;
        }
        if new_y < 0 {
            new_y = 0;
        } else if new_y > height {
            new_y = height;
        }

        self.x = new_x as u32;
        self.y = new_y as u32;
    }

    pub fn update_prev_info(&mut self) {
        self.prev_x = self.x;
        self.prev_y = self.y;
        self.prev_width = self.width;
        self.prev_height = self.height;
    }

    pub fn draw_to_buffer(&mut self) {
        let arena = unsafe { object_arena() };
        arena
            .get_mut(self.object)
            .unwrap()
            .draw_to(&mut self.buffer, self.x, self.y);
    }

    pub fn transfer_buffer_range<T: Painter>(
        &mut self,
        painter: &mut T,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        let x_start = max(x, self.x);
        let x_end = min(x + width, min(self.x + self.width, painter.get_width()));
        let y_start = max(y, self.y);
        let y_end = min(y + height, min(self.y + self.height, painter.get_height()));

        if let Some(transparent) = self.transparent {
            for y in y_start..y_end {
                for x in x_start..x_end {
                    let pixel = self.buffer.get_pixel(x - self.x, y - self.y);
                    if pixel == transparent {
                        continue;
                    }
                    painter.draw_at(x, y, pixel);
                }
            }
        } else {
            // for y in y_start..y_end {
            //     painter.copy_buf(
            //         self.buffer.buffer,
            //         ((y - self.y) * self.width + (x_start - self.x)) as usize,
            //         (y * painter.get_width() + x_start) as usize,
            //         (x_end - x_start) as usize,
            //     );
            // }
            for y in y_start..y_end {
                for x in x_start..x_end {
                    painter.draw_at(x, y, self.buffer.get_pixel(x - self.x, y - self.y));
                }
            }
        }
    }
}

pub struct EventInfo {
    pressed_id: LayerId,
    pressed_x: u32,
    pressed_y: u32,
}

impl EventInfo {
    pub fn new() -> Self {
        EventInfo {
            pressed_id: 0,
            pressed_x: 0,
            pressed_y: 0,
        }
    }
}

pub struct LayerManager<'a, T: Painter> {
    painter: &'a mut T,
    pub layers: BTreeMap<usize, Layer>,
    layer_stack: Vec<usize>,
    curr_id: LayerId,
    event_info: EventInfo,
}

impl<'a, T: Painter> LayerManager<'a, T> {
    pub fn new(painter: &'a mut T) -> Self {
        LayerManager {
            painter,
            layers: BTreeMap::new(),
            layer_stack: Vec::new(),
            curr_id: 0,
            event_info: EventInfo::new(),
        }
    }

    pub fn create_layer<'b>(
        &mut self,
        object: ObjectId,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> LayerId {
        let layer_id = self.curr_id;
        self.curr_id += 1;
        let layer = Layer::new(
            layer_id,
            object,
            x,
            y,
            width,
            height,
            self.painter.get_width(),
            self.painter.get_height(),
        );
        self.layers.insert(layer_id, layer);
        layer_id
    }

    pub fn hide_layer(&mut self, id: LayerId) {
        let mut found = false;
        let mut index = 0;
        for (i, layer_id) in self.layer_stack.iter().enumerate() {
            if *layer_id == id {
                found = true;
                index = i;
                break;
            }
        }

        if found {
            self.layer_stack.remove(index);
        }
    }

    pub fn is_overlapping(x1: u32, x2: u32, x3: u32, x4: u32) -> bool {
        // (x1 <= x3 && x3 <= x2)
        //     || (x1 <= x4 && x4 <= x2)
        //     || (x3 <= x1 && x1 <= x4)
        //     || (x3 <= x2 && x2 <= x4)
        if x4 > x1 && x2 > x3 {
            return true;
        }

        return false;
    }

    pub fn update_buffer(&mut self, id: LayerId) {
        self.layers.get_mut(&id).unwrap().draw_to_buffer();
    }

    pub fn update(&mut self, id: LayerId) {
        let layer = self.layers.get(&id).unwrap();
        let x = layer.x;
        let y = layer.y;
        let width = layer.width;
        let height = layer.height;
        let prev_x = layer.prev_x;
        let prev_y = layer.prev_y;
        let prev_width = layer.prev_width;
        let prev_height = layer.prev_height;
        drop(layer);

        let mut index: usize = 0;
        for (idx, layer_id) in self.layer_stack.iter().enumerate() {
            let layer = self.layers.get_mut(layer_id).unwrap();
            if *layer_id == id {
                index = idx;
                layer.update_prev_info();
                layer.transfer_buffer_range(self.painter, x, y, width, height);
                break;
            }
            layer.transfer_buffer_range(self.painter, prev_x, prev_y, prev_width, prev_height);
        }
        for i in self.layer_stack.iter().skip(index + 1) {
            let layer = self.layers.get_mut(i).unwrap();
            layer.transfer_buffer_range(self.painter, layer.x, layer.y, layer.width, layer.height);
        }
        self.painter.flush();
    }

    pub fn draw_all(&mut self) {
        for i in self.layer_stack.iter() {
            let layer = self.layers.get_mut(i).unwrap();
            layer.draw_to_buffer();
            layer.transfer_buffer_range(self.painter, layer.x, layer.y, layer.width, layer.height);
        }
        self.painter.flush();
    }

    pub fn get_layer_x(&self, id: LayerId) -> u32 {
        self.layers.get(&id).unwrap().x
    }

    pub fn get_layer_y(&self, id: LayerId) -> u32 {
        self.layers.get(&id).unwrap().y
    }

    pub fn get_layer_height(&self, id: LayerId) -> usize {
        for (i, layer_id) in self.layer_stack.iter().enumerate() {
            if *layer_id == id {
                return i;
            }
        }

        return 0;
    }

    pub fn move_layer(&mut self, id: LayerId, mut new_height: i32) {
        if new_height < 0 {
            self.hide_layer(id);
            return;
        }

        if new_height > self.layer_stack.len() as i32 {
            new_height = self.layer_stack.len() as i32;
        }

        let mut found = false;
        let mut index = 0;
        for (i, layer_id) in self.layer_stack.iter().enumerate() {
            if *layer_id == id {
                index = i;
                found = true;
                break;
            }
        }

        if !found {
            self.layer_stack.insert(new_height as usize, id);
            return;
        }

        self.layer_stack.remove(index);
        self.layer_stack.insert(new_height as usize, id);
    }

    pub fn move_rel(&mut self, id: LayerId, x: i32, y: i32) {
        self.layers.get_mut(&id).unwrap().move_rel(x, y);
    }

    pub fn move_abs(&mut self, id: LayerId, x: i32, y: i32) {
        self.layers.get_mut(&id).unwrap().move_abs(x, y);
    }

    pub fn on_event(&mut self, event: ObjectEvent, event_from: LayerId) {
        // let x = self.layers.get(&event_from).unwrap().x;
        // let y = self.layers.get(&event_from).unwrap().y;
        let mut affect_layer: Option<LayerId> = None;
        match event {
            ObjectEvent::MouseLeftPress(x, y) => {
                self.event_info.pressed_x = x;
                self.event_info.pressed_y = y;
                for (_i, layer_id) in self.layer_stack.iter().enumerate() {
                    let layer = self.layers.get_mut(layer_id).unwrap();
                    if layer.id == event_from {
                        break;
                    }
                    if Self::is_overlapping(x, x, layer.x, layer.x + layer.width)
                        && Self::is_overlapping(y, y, layer.y, layer.y + layer.height)
                    {
                        affect_layer = Some(layer.id);
                    }
                }
                if let Some(layer_id) = affect_layer {
                    self.event_info.pressed_id = layer_id;
                }
            }
            ObjectEvent::MouseLeftRelease(_x, _y) => {
                affect_layer = Some(self.event_info.pressed_id);
            }
        }

        if let Some(layer_id) = affect_layer {
            let layer = self.layers.get_mut(&layer_id).unwrap();

            let arena = unsafe { object_arena() };
            arena
                .get_mut(layer.object)
                .unwrap()
                .on_event(event, layer.id);
        }
    }
}

pub struct WindowManager {
    map: BTreeMap<ObjectId, LayerId>,
}

impl WindowManager {
    pub fn new() -> Self {
        WindowManager {
            map: BTreeMap::new(),
        }
    }

    pub fn create_window(
        &mut self,
        title: &str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let arena = unsafe { object_arena() };

        let mut window = Window::new(width, height);
        let title_bar_height = window.title_bar_height;
        window.set_title(title);

        let window_id = arena.alloc(Box::new(window));

        let lm = unsafe { layer_manager() };
        let layer_id = lm.create_layer(window_id, x, y, width, height + title_bar_height);

        self.map.insert(window_id, layer_id);

        window_id
    }

    pub fn show_window(&mut self, id: ObjectId) {
        let arena = unsafe { object_arena() };
        let _object = arena.get_mut(id).unwrap();
        let layer_id = self.map.get(&id).unwrap();

        let lm = unsafe { layer_manager() };
        lm.move_layer(*layer_id, 1);
        lm.update_buffer(*layer_id);
        lm.update(*layer_id);
    }

    pub fn get_window_frame(&mut self, id: ObjectId) -> FrameBuffer {
        let arena = unsafe { object_arena() };
        let window = arena.get(id).unwrap();
        (&**window)
            .as_any()
            .downcast_ref::<Window>()
            .unwrap()
            .get_frame()
    }

    pub fn update_window_frame(&mut self, id: ObjectId) {
        let arena = unsafe { object_arena() };
        let window = arena.get_mut(id).unwrap();

        let layer_id = self.map.get(&id).unwrap();
        let lm = unsafe { layer_manager() };
        let layer = lm.layers.get_mut(&layer_id).unwrap();
        (&mut **window)
            .as_mut_any()
            .downcast_mut::<Window>()
            .unwrap()
            .update_frame(&mut layer.buffer);
        lm.update(*layer_id);
    }

    pub fn get_highest_window_layer(&self) -> Option<LayerId> {
        let lm = unsafe { layer_manager() };
        for layer_id in lm.layer_stack.iter().rev() {
            if let Some(_) = lm
                .layers
                .get(layer_id)
                .unwrap()
                .as_any()
                .downcast_ref::<Window>()
            {
                return Some(*layer_id);
            }
        }

        return None;
    }
}

pub unsafe fn object_arena() -> &'static mut ObjectArena {
    match OBJECT_ARENA {
        Some(ref mut arena) => &mut *arena,
        None => panic!("object arena is uninitialized"),
    }
}

#[cfg(target_arch = "riscv64")]
pub unsafe fn layer_manager() -> &'static mut LayerManager<'static, Mutex<VirtioGpu>> {
    match LAYER_MANAGER {
        Some(ref mut lm) => &mut *lm,
        None => panic!("layer manager is uninitialized"),
    }
}

pub unsafe fn window_manager() -> &'static mut WindowManager {
    match WINDOW_MANAGER {
        Some(ref mut wm) => &mut *wm,
        None => panic!("window manager is uninitialized"),
    }
}

pub fn init() {
    let arena = ObjectArena::new();
    unsafe {
        OBJECT_ARENA = Some(arena);
    }

    let arena = unsafe { object_arena() };

    let display = unsafe { gpu_device() };
    let width = display.lock().width;
    let height = display.lock().height;
    let mut lm = LayerManager::new(display);
    let wm = WindowManager::new();
    let mouse_transparent_color = 0xff00ff00;
    let mouse = Mouse::new(mouse_transparent_color);
    let mouse_id = arena.alloc(Box::new(mouse));
    let desktop = Desktop::new(0xffffffff, width, height);
    let desktop_id = arena.alloc(Box::new(desktop));
    unsafe {
        MOUSE_LAYER_ID = lm.create_layer(mouse_id, 0, 0, 16, 23);
        DESKTOP_LAYER_ID = lm.create_layer(desktop_id, 0, 0, width, height);
        LAYER_MANAGER = Some(lm);
        WINDOW_MANAGER = Some(wm);
        let lm = layer_manager();
        lm.layers
            .get_mut(&MOUSE_LAYER_ID)
            .unwrap()
            .set_transparent_color(mouse_transparent_color);
        lm.move_layer(MOUSE_LAYER_ID, i32::max_value());
        lm.move_layer(DESKTOP_LAYER_ID, 0);
        lm.draw_all();
    }
}
