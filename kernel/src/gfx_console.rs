use core::convert::Infallible;
use core::fmt;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use ibm437::IBM437_8X8_REGULAR;
use virtio_drivers::device::gpu::VirtIOGpu;
use virtio_drivers::device::input::{InputEvent, VirtIOInput};
use virtio_drivers::transport::DeviceType;
use virtio_drivers::transport::Transport;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::{BufferDirection, Hal, PhysAddr, PAGE_SIZE};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;
const CELL_W: u32 = 8;
const CELL_H: u32 = 8;
const COLS: usize = (WIDTH / CELL_W) as usize;
const ROWS: usize = (HEIGHT / CELL_H) as usize;

const MMIO_BASE: usize = 0x1000_1000;
const MMIO_STEP: usize = 0x1000;
const MMIO_COUNT: usize = 8;
const MMIO_SIZE: usize = 0x1000;

const VIRTIO_MAGIC: u32 = 0x7472_6976;
const DEVICE_ID_GPU: u32 = 16;
const DEVICE_ID_INPUT: u32 = 18;

const EV_KEY: u16 = 0x01;
const KEY_RELEASE: u32 = 0;
const KEY_PRESS: u32 = 1;
const KEY_REPEAT: u32 = 2;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_RIGHTSHIFT: u16 = 54;

const DMA_POOL_SIZE: usize = 2 * 1024 * 1024;

#[repr(align(4096))]
struct DmaPool([u8; DMA_POOL_SIZE]);

static DMA_NEXT: AtomicUsize = AtomicUsize::new(0);
static mut DMA_POOL: DmaPool = DmaPool([0; DMA_POOL_SIZE]);

struct KernelHal;

unsafe impl Hal for KernelHal {
    fn dma_alloc(
        pages: usize,
        _direction: BufferDirection,
    ) -> (PhysAddr, NonNull<u8>) {
        let bytes = pages * PAGE_SIZE;
        let start = DMA_NEXT.fetch_add(bytes, Ordering::SeqCst);
        assert!(start + bytes <= DMA_POOL_SIZE, "virtio DMA pool exhausted");

        let ptr = unsafe { core::ptr::addr_of_mut!(DMA_POOL.0).cast::<u8>().add(start) };
        unsafe { core::ptr::write_bytes(ptr, 0, bytes) };
        let vaddr = NonNull::new(ptr).expect("dma pointer is null");
        let paddr = vaddr.as_ptr() as u64;
        (paddr, vaddr)
    }

    unsafe fn dma_dealloc(_paddr: PhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as *mut u8).expect("mmio address is null")
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        buffer.as_ptr() as *mut u8 as u64
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}

#[derive(Debug)]
pub enum GraphicsInitError {
    MissingGpu,
    MissingKeyboard,
    Transport,
    Driver,
}

impl fmt::Display for GraphicsInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphicsInitError::MissingGpu => write!(f, "virtio-gpu device not found"),
            GraphicsInitError::MissingKeyboard => write!(f, "virtio-keyboard device not found"),
            GraphicsInitError::Transport => write!(f, "failed to initialize virtio transport"),
            GraphicsInitError::Driver => write!(f, "failed to initialize virtio driver"),
        }
    }
}

type Gpu = VirtIOGpu<KernelHal, MmioTransport<'static>>;
type Input = VirtIOInput<KernelHal, MmioTransport<'static>>;

pub struct FramebufferConsole {
    gpu: Gpu,
    input: Input,
    fb_ptr: NonNull<u8>,
    fb_len: usize,
    ansi_state: AnsiState,
    cursor_x: usize,
    cursor_y: usize,
    foreground: Rgb888,
    background: Rgb888,
    keybuf: [u8; 64],
    key_head: usize,
    key_tail: usize,
    shift_down: bool,
    dirty: bool,
}

impl FramebufferConsole {
    pub fn new() -> Result<Self, GraphicsInitError> {
        let (gpu_transport, input_transport) = find_virtio_mmio_devices()?;

        let mut gpu = VirtIOGpu::<KernelHal, _>::new(gpu_transport)
            .map_err(|_| GraphicsInitError::Driver)?;
        let input = VirtIOInput::<KernelHal, _>::new(input_transport)
            .map_err(|_| GraphicsInitError::Driver)?;

        let fb = gpu
            .change_resolution(WIDTH, HEIGHT)
            .map_err(|_| GraphicsInitError::Driver)?;
        let fb_ptr = NonNull::new(fb.as_mut_ptr()).ok_or(GraphicsInitError::Driver)?;
        let fb_len = fb.len();

        let mut console = Self {
            gpu,
            input,
            fb_ptr,
            fb_len,
            ansi_state: AnsiState::Ground,
            cursor_x: 0,
            cursor_y: 0,
            foreground: Rgb888::new(0xee, 0xee, 0xee),
            background: Rgb888::new(0x14, 0x14, 0x18),
            keybuf: [0; 64],
            key_head: 0,
            key_tail: 0,
            shift_down: false,
            dirty: false,
        };

        console.clear_screen();
        console.flush();

        Ok(console)
    }

    fn flush(&mut self) {
        let _ = self.gpu.flush();
        self.dirty = false;
    }

    fn framebuffer_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.fb_ptr.as_ptr(), self.fb_len) }
    }

    fn clear_screen(&mut self) {
        let bg = self.background;
        let fb = self.framebuffer_mut();
        for pixel in fb.chunks_exact_mut(4) {
            pixel[0] = bg.b();
            pixel[1] = bg.g();
            pixel[2] = bg.r();
            pixel[3] = 0xff;
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.dirty = true;
    }

    fn scroll_up(&mut self) {
        let stride = (WIDTH as usize) * 4;
        let row_bytes = stride * (CELL_H as usize);
        let visible_bytes = stride * (HEIGHT as usize);
        let bg = self.background;
        let fb = self.framebuffer_mut();

        fb.copy_within(row_bytes..visible_bytes, 0);

        for pixel in fb[(visible_bytes - row_bytes)..visible_bytes].chunks_exact_mut(4) {
            pixel[0] = bg.b();
            pixel[1] = bg.g();
            pixel[2] = bg.r();
            pixel[3] = 0xff;
        }
        self.dirty = true;
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 1;
        if self.cursor_y >= ROWS {
            self.cursor_y = ROWS - 1;
            self.scroll_up();
        }
    }

    fn draw_char(&mut self, ch: char) {
        if self.cursor_x >= COLS {
            self.newline();
        }

        let px = (self.cursor_x as i32) * (CELL_W as i32);
        let py = (self.cursor_y as i32) * (CELL_H as i32);

        let style = MonoTextStyleBuilder::new()
            .font(&IBM437_8X8_REGULAR)
            .text_color(self.foreground)
            .background_color(self.background)
            .build();

        let mut surface = FramebufferSurface {
            width: WIDTH,
            height: HEIGHT,
            fb: self.framebuffer_mut(),
        };

        let mut text = [0u8; 4];
        let s = ch.encode_utf8(&mut text);
        let _ = Text::with_baseline(s, Point::new(px, py), style, Baseline::Top).draw(&mut surface);

        self.cursor_x += 1;
        self.dirty = true;
    }

    fn process_event(&mut self, event: InputEvent) {
        if event.event_type != EV_KEY {
            return;
        }

        if event.code == KEY_LEFTSHIFT || event.code == KEY_RIGHTSHIFT {
            self.shift_down = event.value != KEY_RELEASE;
            return;
        }

        if event.value != KEY_PRESS && event.value != KEY_REPEAT {
            return;
        }

        if let Some(byte) = map_keycode(event.code, self.shift_down) {
            self.enqueue_key(byte);
        }
    }

    fn poll_input(&mut self) {
        while let Some(event) = self.input.pop_pending_event() {
            self.process_event(event);
        }
    }

    fn enqueue_key(&mut self, key: u8) {
        let next = (self.key_tail + 1) % self.keybuf.len();
        if next == self.key_head {
            return;
        }
        self.keybuf[self.key_tail] = key;
        self.key_tail = next;
    }

    fn dequeue_key(&mut self) -> Option<u8> {
        if self.key_head == self.key_tail {
            return None;
        }
        let key = self.keybuf[self.key_head];
        self.key_head = (self.key_head + 1) % self.keybuf.len();
        Some(key)
    }

    fn csi_clear_screen(&mut self) {
        self.clear_screen();
    }

    fn csi_cursor_position(&mut self, row: usize, col: usize) {
        self.cursor_y = row.min(ROWS.saturating_sub(1));
        self.cursor_x = col.min(COLS.saturating_sub(1));
    }
}

impl game::io::Console for FramebufferConsole {
    fn write_byte(&mut self, byte: u8) {
        self.feed_ansi(byte);
    }

    fn read_byte_blocking(&mut self) -> u8 {
        if self.dirty {
            self.flush();
        }

        loop {
            self.poll_input();
            if let Some(key) = self.dequeue_key() {
                return key;
            }
            core::hint::spin_loop();
        }
    }
}

#[derive(Clone, Copy)]
enum AnsiState {
    Ground,
    Esc,
    Csi {
        params: [u16; 4],
        count: usize,
        current: u16,
        has_current: bool,
    },
}

impl FramebufferConsole {
    fn feed_ansi(&mut self, byte: u8) {
        match self.ansi_state {
            AnsiState::Ground => match byte {
                0x1b => self.ansi_state = AnsiState::Esc,
                b'\n' => self.newline(),
                b'\r' => self.cursor_x = 0,
                b'\t' => {
                    let next = (self.cursor_x + 8) & !7;
                    self.cursor_x = next.min(COLS.saturating_sub(1));
                }
                0x08 => self.cursor_x = self.cursor_x.saturating_sub(1),
                _ if byte >= 0x20 => self.draw_char(byte as char),
                _ => {}
            },
            AnsiState::Esc => {
                if byte == b'[' {
                    self.ansi_state = AnsiState::Csi {
                        params: [0; 4],
                        count: 0,
                        current: 0,
                        has_current: false,
                    };
                } else {
                    self.ansi_state = AnsiState::Ground;
                }
            }
            AnsiState::Csi {
                mut params,
                mut count,
                mut current,
                mut has_current,
            } => {
                match byte {
                    b'0'..=b'9' => {
                        current = current.saturating_mul(10).saturating_add((byte - b'0') as u16);
                        has_current = true;
                    }
                    b';' => {
                        if count < params.len() {
                            params[count] = if has_current { current } else { 0 };
                            count += 1;
                        }
                        current = 0;
                        has_current = false;
                    }
                    action => {
                        if count < params.len() {
                            params[count] = if has_current { current } else { 0 };
                            count += 1;
                        }
                        self.handle_csi(action, &params, count);
                        self.ansi_state = AnsiState::Ground;
                        return;
                    }
                }

                self.ansi_state = AnsiState::Csi {
                    params,
                    count,
                    current,
                    has_current,
                };
            }
        }
    }

    fn handle_csi(&mut self, action: u8, params: &[u16; 4], count: usize) {
        match action {
            b'J' => {
                let mode = get_param(params, count, 0, 0);
                if mode == 2 {
                    self.csi_clear_screen();
                }
            }
            b'H' | b'f' => {
                let row = get_param(params, count, 0, 1).saturating_sub(1) as usize;
                let col = get_param(params, count, 1, 1).saturating_sub(1) as usize;
                self.csi_cursor_position(row, col);
            }
            _ => {}
        }
    }
}

fn get_param(params: &[u16; 4], count: usize, index: usize, default: u16) -> u16 {
    if index >= count {
        return default;
    }
    let value = params[index];
    if value == 0 {
        default
    } else {
        value
    }
}

fn map_keycode(code: u16, shift: bool) -> Option<u8> {
    let key = match code {
        2 => if shift { b'!' } else { b'1' },
        3 => if shift { b'@' } else { b'2' },
        4 => if shift { b'#' } else { b'3' },
        5 => if shift { b'$' } else { b'4' },
        6 => if shift { b'%' } else { b'5' },
        7 => if shift { b'^' } else { b'6' },
        8 => if shift { b'&' } else { b'7' },
        9 => if shift { b'*' } else { b'8' },
        10 => if shift { b'(' } else { b'9' },
        11 => if shift { b')' } else { b'0' },
        16 => if shift { b'Q' } else { b'q' },
        17 => if shift { b'W' } else { b'w' },
        18 => if shift { b'E' } else { b'e' },
        19 => if shift { b'R' } else { b'r' },
        20 => if shift { b'T' } else { b't' },
        21 => if shift { b'Y' } else { b'y' },
        22 => if shift { b'U' } else { b'u' },
        23 => if shift { b'I' } else { b'i' },
        24 => if shift { b'O' } else { b'o' },
        25 => if shift { b'P' } else { b'p' },
        30 => if shift { b'A' } else { b'a' },
        31 => if shift { b'S' } else { b's' },
        32 => if shift { b'D' } else { b'd' },
        33 => if shift { b'F' } else { b'f' },
        34 => if shift { b'G' } else { b'g' },
        35 => if shift { b'H' } else { b'h' },
        36 => if shift { b'J' } else { b'j' },
        37 => if shift { b'K' } else { b'k' },
        38 => if shift { b'L' } else { b'l' },
        44 => if shift { b'Z' } else { b'z' },
        45 => if shift { b'X' } else { b'x' },
        46 => if shift { b'C' } else { b'c' },
        47 => if shift { b'V' } else { b'v' },
        48 => if shift { b'B' } else { b'b' },
        49 => if shift { b'N' } else { b'n' },
        50 => if shift { b'M' } else { b'm' },
        28 => b'\n',
        57 => b' ',

        // Arrow keys (Linux evdev KEY_LEFT/UP/RIGHT/DOWN).
        105 => b'h',
        103 => b'k',
        106 => b'l',
        108 => b'j',

        // Numpad digits with NumLock on (KEY_KP1..KEY_KP9).
        79 => b'1',
        80 => b'2',
        81 => b'3',
        75 => b'4',
        76 => b'5',
        77 => b'6',
        71 => b'7',
        72 => b'8',
        73 => b'9',

        // Numpad-as-navigation with NumLock off.
        107 => b'1', // End
        109 => b'3', // PageDown
        102 => b'7', // Home
        104 => b'9', // PageUp
        _ => return None,
    };
    Some(key)
}

fn find_virtio_mmio_devices() -> Result<(MmioTransport<'static>, MmioTransport<'static>), GraphicsInitError> {
    let mut gpu = None;
    let mut input = None;

    for index in 0..MMIO_COUNT {
        let base = MMIO_BASE + index * MMIO_STEP;
        let magic = unsafe { (base as *const u32).read_volatile() };
        if magic != VIRTIO_MAGIC {
            continue;
        }

        let device_id = unsafe { ((base + 0x008) as *const u32).read_volatile() };
        if device_id != DEVICE_ID_GPU && device_id != DEVICE_ID_INPUT {
            continue;
        }

        let header = NonNull::new(base as *mut VirtIOHeader).ok_or(GraphicsInitError::Transport)?;
        let transport = unsafe { MmioTransport::new(header, MMIO_SIZE) }
            .map_err(|_| GraphicsInitError::Transport)?;

        match transport.device_type() {
            DeviceType::GPU if gpu.is_none() => gpu = Some(transport),
            DeviceType::Input if input.is_none() => input = Some(transport),
            _ => {}
        }

        if gpu.is_some() && input.is_some() {
            break;
        }
    }

    let gpu = gpu.ok_or(GraphicsInitError::MissingGpu)?;
    let input = input.ok_or(GraphicsInitError::MissingKeyboard)?;
    Ok((gpu, input))
}

struct FramebufferSurface<'a> {
    width: u32,
    height: u32,
    fb: &'a mut [u8],
}

impl OriginDimensions for FramebufferSurface<'_> {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for FramebufferSurface<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let width = self.width as i32;
        let height = self.height as i32;
        let stride = self.width as usize * 4;

        for Pixel(point, color) in pixels {
            let x = point.x;
            let y = point.y;
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }

            let offset = (y as usize) * stride + (x as usize) * 4;
            self.fb[offset] = color.b();
            self.fb[offset + 1] = color.g();
            self.fb[offset + 2] = color.r();
            self.fb[offset + 3] = 0xff;
        }

        Ok(())
    }
}
