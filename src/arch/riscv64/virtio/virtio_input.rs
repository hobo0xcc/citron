use super::super::virtio;
use super::*;
use crate::arch::riscv64::interrupt::*;
use crate::process::process_manager;
use crate::process::ProcessEvent;
use alloc::alloc::{alloc, alloc_zeroed, Layout};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::NonNull;
use volatile_register::*;

pub const EVENT_BUFFER_SIZE: usize = VIRTIO_RING_SIZE;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DeviceType {
    Mouse,
    Keyboard,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum VirtioInputConfigSelect {
    InputCfgUnset = 0x00,
    InputCfgIdName = 0x01,
    InputCfgIdSerial = 0x02,
    InputCfgIdDevids = 0x03,
    InputCfgPropBits = 0x10,
    InputCfgEvBits = 0x11,
    InputCfgAbsInfo = 0x12,
}

impl VirtioInputConfigSelect {
    pub fn val(&self) -> u8 {
        *self as u8
    }
}

// from linux kernel: include/linux/input.h
#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum EventType {
    EV_SYN = 0,
    EV_KEY = 1,
    EV_REL = 2,
    EV_ABS = 3,
    EV_MSC = 4,
    EV_SW = 5,
    EV_LED = 17,
    EV_SND = 18,
    EV_REP = 20,
    EV_FF = 21,
    EV_PWR = 22,
    EV_FF_STATUS = 23,
    EV_UNK,
    EV_MAX = 31,
}

impl EventType {
    pub fn from(type_: u16) -> EventType {
        match type_ {
            0 => EventType::EV_SYN,
            1 => EventType::EV_KEY,
            2 => EventType::EV_REL,
            3 => EventType::EV_ABS,
            4 => EventType::EV_MSC,
            5 => EventType::EV_SW,
            17 => EventType::EV_LED,
            18 => EventType::EV_SND,
            20 => EventType::EV_REP,
            21 => EventType::EV_FF,
            22 => EventType::EV_PWR,
            23 => EventType::EV_FF_STATUS,
            31 => EventType::EV_MAX,
            _ => panic!("unknown event type: {}", type_),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum EV_REL {
    REL_X = 0,
    REL_Y = 1,
    REL_Z = 2,
    REL_RX = 3,
    REL_RY = 4,
    REL_RZ = 5,
    REL_HWHEEL = 6,
    REL_DIAL = 7,
    REL_WHEEL = 8,
    REL_MISC = 9,
    REL_RESERVED = 10,
    REL_WHEEL_HI_RES = 11,
    REL_HWHEEL_HI_RES = 12,
    REL_MAX = 15,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum EV_KEY {
    KEY_RESERVED = 0,
    KEY_ESC = 1,
    KEY_1 = 2,
    KEY_2 = 3,
    KEY_3 = 4,
    KEY_4 = 5,
    KEY_5 = 6,
    KEY_6 = 7,
    KEY_7 = 8,
    KEY_8 = 9,
    KEY_9 = 10,
    KEY_0 = 11,
    KEY_MINUS = 12,
    KEY_EQUAL = 13,
    KEY_BACKSPACE = 14,
    KEY_TAB = 15,
    KEY_Q = 16,
    KEY_W = 17,
    KEY_E = 18,
    KEY_R = 19,
    KEY_T = 20,
    KEY_Y = 21,
    KEY_U = 22,
    KEY_I = 23,
    KEY_O = 24,
    KEY_P = 25,
    KEY_LEFTBRACE = 26,
    KEY_RIGHTBRACE = 27,
    KEY_ENTER = 28,
    KEY_LEFTCTRL = 29,
    KEY_A = 30,
    KEY_S = 31,
    KEY_D = 32,
    KEY_F = 33,
    KEY_G = 34,
    KEY_H = 35,
    KEY_J = 36,
    KEY_K = 37,
    KEY_L = 38,
    KEY_SEMICOLON = 39,
    KEY_APOSTROPHE = 40,
    KEY_GRAVE = 41,
    KEY_LEFTSHIFT = 42,
    KEY_BACKSLASH = 43,
    KEY_Z = 44,
    KEY_X = 45,
    KEY_C = 46,
    KEY_V = 47,
    KEY_B = 48,
    KEY_N = 49,
    KEY_M = 50,
    KEY_COMMA = 51,
    KEY_DOT = 52,
    KEY_SLASH = 53,
    KEY_RIGHTSHIFT = 54,
    KEY_KPASTERISK = 55,
    KEY_LEFTALT = 56,
    KEY_SPACE = 57,
    KEY_CAPSLOCK = 58,
    KEY_F1 = 59,
    KEY_F2 = 60,
    KEY_F3 = 61,
    KEY_F4 = 62,
    KEY_F5 = 63,
    KEY_F6 = 64,
    KEY_F7 = 65,
    KEY_F8 = 66,
    KEY_F9 = 67,
    KEY_F10 = 68,
    KEY_NUMLOCK = 69,
    KEY_SCROLLLOCK = 70,
    KEY_KP7 = 71,
    KEY_KP8 = 72,
    KEY_KP9 = 73,
    KEY_KPMINUS = 74,
    KEY_KP4 = 75,
    KEY_KP5 = 76,
    KEY_KP6 = 77,
    KEY_KPPLUS = 78,
    KEY_KP1 = 79,
    KEY_KP2 = 80,
    KEY_KP3 = 81,
    KEY_KP0 = 82,
    KEY_KPDOT = 83,
    KEY_ZENKAKUHANKAKU = 85,
    KEY_102ND = 86,
    KEY_F11 = 87,
    KEY_F12 = 88,
    KEY_RO = 89,
    KEY_KATAKANA = 90,
    KEY_HIRAGANA = 91,
    KEY_HENKAN = 92,
    KEY_KATAKANAHIRAGANA = 93,
    KEY_MUHENKAN = 94,
    KEY_KPJPCOMMA = 95,
    KEY_KPENTER = 96,
    KEY_RIGHTCTRL = 97,
    KEY_KPSLASH = 98,
    KEY_SYSRQ = 99,
    KEY_RIGHTALT = 100,
    KEY_LINEFEED = 101,
    KEY_HOME = 102,
    KEY_UP = 103,
    KEY_PAGEUP = 104,
    KEY_LEFT = 105,
    KEY_RIGHT = 106,
    KEY_END = 107,
    KEY_DOWN = 108,
    KEY_PAGEDOWN = 109,
    KEY_INSERT = 110,
    KEY_DELETE = 111,
    KEY_MACRO = 112,
    KEY_MUTE = 113,
    KEY_VOLUMEDOWN = 114,
    KEY_VOLUMEUP = 115,
    KEY_POWER = 116,
    KEY_KPEQUAL = 117,
    KEY_KPPLUSMINUS = 118,
    KEY_PAUSE = 119,
    KEY_SCALE = 120,
    KEY_KPCOMMA = 121,
    KEY_HANGEUL = 122,
    KEY_HANJA = 123,
    KEY_YEN = 124,
    KEY_LEFTMETA = 125,
    KEY_RIGHTMETA = 126,
    KEY_COMPOSE = 127,
    KEY_STOP = 128,
    KEY_AGAIN = 129,
    KEY_PROPS = 130,
    KEY_UNDO = 131,
    KEY_FRONT = 132,
    KEY_COPY = 133,
    KEY_OPEN = 134,
    KEY_PASTE = 135,
    KEY_FIND = 136,
    KEY_CUT = 137,
    KEY_HELP = 138,
    KEY_MENU = 139,
    KEY_CALC = 140,
    KEY_SETUP = 141,
    KEY_SLEEP = 142,
    KEY_WAKEUP = 143,
    KEY_FILE = 144,
    KEY_SENDFILE = 145,
    KEY_DELETEFILE = 146,
    KEY_XFER = 147,
    KEY_PROG1 = 148,
    KEY_PROG2 = 149,
    KEY_WWW = 150,
    KEY_MSDOS = 151,
    KEY_COFFEE = 152,
    KEY_ROTATE_DISPLAY = 153,
    KEY_CYCLEWINDOWS = 154,
    KEY_MAIL = 155,
    KEY_BOOKMARKS = 156,
    KEY_COMPUTER = 157,
    KEY_BACK = 158,
    KEY_FORWARD = 159,
    KEY_CLOSECD = 160,
    KEY_EJECTCD = 161,
    KEY_EJECTCLOSECD = 162,
    KEY_NEXTSONG = 163,
    KEY_PLAYPAUSE = 164,
    KEY_PREVIOUSSONG = 165,
    KEY_STOPCD = 166,
    KEY_RECORD = 167,
    KEY_REWIND = 168,
    KEY_PHONE = 169,
    KEY_ISO = 170,
    KEY_CONFIG = 171,
    KEY_HOMEPAGE = 172,
    KEY_REFRESH = 173,
    KEY_EXIT = 174,
    KEY_MOVE = 175,
    KEY_EDIT = 176,
    KEY_SCROLLUP = 177,
    KEY_SCROLLDOWN = 178,
    KEY_KPLEFTPAREN = 179,
    KEY_KPRIGHTPAREN = 180,
    KEY_NEW = 181,
    KEY_REDO = 182,
    KEY_F13 = 183,
    KEY_F14 = 184,
    KEY_F15 = 185,
    KEY_F16 = 186,
    KEY_F17 = 187,
    KEY_F18 = 188,
    KEY_F19 = 189,
    KEY_F20 = 190,
    KEY_F21 = 191,
    KEY_F22 = 192,
    KEY_F23 = 193,
    KEY_F24 = 194,
    KEY_PLAYCD = 200,
    KEY_PAUSECD = 201,
    KEY_PROG3 = 202,
    KEY_PROG4 = 203,
    KEY_DASHBOARD = 204,
    KEY_SUSPEND = 205,
    KEY_CLOSE = 206,
    KEY_PLAY = 207,
    KEY_FASTFORWARD = 208,
    KEY_BASSBOOST = 209,
    KEY_PRINT = 210,
    KEY_HP = 211,
    KEY_CAMERA = 212,
    KEY_SOUND = 213,
    KEY_QUESTION = 214,
    KEY_EMAIL = 215,
    KEY_CHAT = 216,
    KEY_SEARCH = 217,
    KEY_CONNECT = 218,
    KEY_FINANCE = 219,
    KEY_SPORT = 220,
    KEY_SHOP = 221,
    KEY_ALTERASE = 222,
    KEY_CANCEL = 223,
    KEY_BRIGHTNESSDOWN = 224,
    KEY_BRIGHTNESSUP = 225,
    KEY_MEDIA = 226,
    KEY_SWITCHVIDEOMODE = 227,
    KEY_KBDILLUMTOGGLE = 228,
    KEY_KBDILLUMDOWN = 229,
    KEY_KBDILLUMUP = 230,
    KEY_SEND = 231,
    KEY_REPLY = 232,
    KEY_FORWARDMAIL = 233,
    KEY_SAVE = 234,
    KEY_DOCUMENTS = 235,
    KEY_BATTERY = 236,
    KEY_BLUETOOTH = 237,
    KEY_WLAN = 238,
    KEY_UWB = 239,
    KEY_UNKNOWN = 240,
    KEY_VIDEO_NEXT = 241,
    KEY_VIDEO_PREV = 242,
    KEY_BRIGHTNESS_CYCLE = 243,
    KEY_BRIGHTNESS_AUTO = 244,
    KEY_DISPLAY_OFF = 245,
    KEY_WWAN = 246,
    KEY_RFKILL = 247,
    KEY_MICMUTE = 248,
    KEY_OK = 352,
    KEY_SELECT = 353,
    KEY_GOTO = 354,
    KEY_CLEAR = 355,
    KEY_POWER2 = 356,
    KEY_OPTION = 357,
    KEY_INFO = 358,
    KEY_TIME = 359,
    KEY_VENDOR = 360,
    KEY_ARCHIVE = 361,
    KEY_PROGRAM = 362,
    KEY_CHANNEL = 363,
    KEY_FAVORITES = 364,
    KEY_EPG = 365,
    KEY_PVR = 366,
    KEY_MHP = 367,
    KEY_LANGUAGE = 368,
    KEY_TITLE = 369,
    KEY_SUBTITLE = 370,
    KEY_ANGLE = 371,
    KEY_FULL_SCREEN = 372,
    KEY_MODE = 373,
    KEY_KEYBOARD = 374,
    KEY_ASPECT_RATIO = 375,
    KEY_PC = 376,
    KEY_TV = 377,
    KEY_TV2 = 378,
    KEY_VCR = 379,
    KEY_VCR2 = 380,
    KEY_SAT = 381,
    KEY_SAT2 = 382,
    KEY_CD = 383,
    KEY_TAPE = 384,
    KEY_RADIO = 385,
    KEY_TUNER = 386,
    KEY_PLAYER = 387,
    KEY_TEXT = 388,
    KEY_DVD = 389,
    KEY_AUX = 390,
    KEY_MP3 = 391,
    KEY_AUDIO = 392,
    KEY_VIDEO = 393,
    KEY_DIRECTORY = 394,
    KEY_LIST = 395,
    KEY_MEMO = 396,
    KEY_CALENDAR = 397,
    KEY_RED = 398,
    KEY_GREEN = 399,
    KEY_YELLOW = 400,
    KEY_BLUE = 401,
    KEY_CHANNELUP = 402,
    KEY_CHANNELDOWN = 403,
    KEY_FIRST = 404,
    KEY_LAST = 405,
    KEY_AB = 406,
    KEY_NEXT = 407,
    KEY_RESTART = 408,
    KEY_SLOW = 409,
    KEY_SHUFFLE = 410,
    KEY_BREAK = 411,
    KEY_PREVIOUS = 412,
    KEY_DIGITS = 413,
    KEY_TEEN = 414,
    KEY_TWEN = 415,
    KEY_VIDEOPHONE = 416,
    KEY_GAMES = 417,
    KEY_ZOOMIN = 418,
    KEY_ZOOMOUT = 419,
    KEY_ZOOMRESET = 420,
    KEY_WORDPROCESSOR = 421,
    KEY_EDITOR = 422,
    KEY_SPREADSHEET = 423,
    KEY_GRAPHICSEDITOR = 424,
    KEY_PRESENTATION = 425,
    KEY_DATABASE = 426,
    KEY_NEWS = 427,
    KEY_VOICEMAIL = 428,
    KEY_ADDRESSBOOK = 429,
    KEY_MESSENGER = 430,
    KEY_DISPLAYTOGGLE = 431,
    KEY_SPELLCHECK = 432,
    KEY_LOGOFF = 433,
    KEY_DOLLAR = 434,
    KEY_EURO = 435,
    KEY_FRAMEBACK = 436,
    KEY_FRAMEFORWARD = 437,
    KEY_CONTEXT_MENU = 438,
    KEY_MEDIA_REPEAT = 439,
    KEY_10CHANNELSUP = 440,
    KEY_10CHANNELSDOWN = 441,
    KEY_IMAGES = 442,
    KEY_DEL_EOL = 448,
    KEY_DEL_EOS = 449,
    KEY_INS_LINE = 450,
    KEY_DEL_LINE = 451,
    KEY_FN = 464,
    KEY_FN_ESC = 465,
    KEY_FN_F1 = 466,
    KEY_FN_F2 = 467,
    KEY_FN_F3 = 468,
    KEY_FN_F4 = 469,
    KEY_FN_F5 = 470,
    KEY_FN_F6 = 471,
    KEY_FN_F7 = 472,
    KEY_FN_F8 = 473,
    KEY_FN_F9 = 474,
    KEY_FN_F10 = 475,
    KEY_FN_F11 = 476,
    KEY_FN_F12 = 477,
    KEY_FN_1 = 478,
    KEY_FN_2 = 479,
    KEY_FN_D = 480,
    KEY_FN_E = 481,
    KEY_FN_F = 482,
    KEY_FN_S = 483,
    KEY_FN_B = 484,
    KEY_BRL_DOT1 = 497,
    KEY_BRL_DOT2 = 498,
    KEY_BRL_DOT3 = 499,
    KEY_BRL_DOT4 = 500,
    KEY_BRL_DOT5 = 501,
    KEY_BRL_DOT6 = 502,
    KEY_BRL_DOT7 = 503,
    KEY_BRL_DOT8 = 504,
    KEY_BRL_DOT9 = 505,
    KEY_BRL_DOT10 = 506,
    KEY_NUMERIC_0 = 512,
    KEY_NUMERIC_1 = 513,
    KEY_NUMERIC_2 = 514,
    KEY_NUMERIC_3 = 515,
    KEY_NUMERIC_4 = 516,
    KEY_NUMERIC_5 = 517,
    KEY_NUMERIC_6 = 518,
    KEY_NUMERIC_7 = 519,
    KEY_NUMERIC_8 = 520,
    KEY_NUMERIC_9 = 521,
    KEY_NUMERIC_STAR = 522,
    KEY_NUMERIC_POUND = 523,
    KEY_NUMERIC_A = 524,
    KEY_NUMERIC_B = 525,
    KEY_NUMERIC_C = 526,
    KEY_NUMERIC_D = 527,
    KEY_CAMERA_FOCUS = 528,
    KEY_WPS_BUTTON = 529,
    KEY_TOUCHPAD_TOGGLE = 530,
    KEY_TOUCHPAD_ON = 531,
    KEY_TOUCHPAD_OFF = 532,
    KEY_CAMERA_ZOOMIN = 533,
    KEY_CAMERA_ZOOMOUT = 534,
    KEY_CAMERA_UP = 535,
    KEY_CAMERA_DOWN = 536,
    KEY_CAMERA_LEFT = 537,
    KEY_CAMERA_RIGHT = 538,
    KEY_ATTENDANT_ON = 539,
    KEY_ATTENDANT_OFF = 540,
    KEY_ATTENDANT_TOGGLE = 541,
    KEY_LIGHTS_TOGGLE = 542,
    KEY_ALS_TOGGLE = 560,
    KEY_ROTATE_LOCK_TOGGLE = 561,
    KEY_BUTTONCONFIG = 576,
    KEY_TASKMANAGER = 577,
    KEY_JOURNAL = 578,
    KEY_CONTROLPANEL = 579,
    KEY_APPSELECT = 580,
    KEY_SCREENSAVER = 581,
    KEY_VOICECOMMAND = 582,
    KEY_ASSISTANT = 583,
    KEY_KBD_LAYOUT_NEXT = 584,
    KEY_BRIGHTNESS_MIN = 592,
    KEY_BRIGHTNESS_MAX = 593,
    KEY_KBDINPUTASSIST_PREV = 608,
    KEY_KBDINPUTASSIST_NEXT = 609,
    KEY_KBDINPUTASSIST_PREVGROUP = 610,
    KEY_KBDINPUTASSIST_NEXTGROUP = 611,
    KEY_KBDINPUTASSIST_ACCEPT = 612,
    KEY_KBDINPUTASSIST_CANCEL = 613,
    KEY_RIGHT_UP = 614,
    KEY_RIGHT_DOWN = 615,
    KEY_LEFT_UP = 616,
    KEY_LEFT_DOWN = 617,
    KEY_ROOT_MENU = 618,
    KEY_MEDIA_TOP_MENU = 619,
    KEY_NUMERIC_11 = 620,
    KEY_NUMERIC_12 = 621,
    KEY_AUDIO_DESC = 622,
    KEY_3D_MODE = 623,
    KEY_NEXT_FAVORITE = 624,
    KEY_STOP_RECORD = 625,
    KEY_PAUSE_RECORD = 626,
    KEY_VOD = 627,
    KEY_UNMUTE = 628,
    KEY_FASTREVERSE = 629,
    KEY_SLOWREVERSE = 630,
    KEY_DATA = 631,
    KEY_ONSCREEN_KEYBOARD = 632,
    KEY_MAX = 767,
    BTN_0 = 256,
    BTN_1 = 257,
    BTN_2 = 258,
    BTN_3 = 259,
    BTN_4 = 260,
    BTN_5 = 261,
    BTN_6 = 262,
    BTN_7 = 263,
    BTN_8 = 264,
    BTN_9 = 265,
    BTN_LEFT = 272,
    BTN_RIGHT = 273,
    BTN_MIDDLE = 274,
    BTN_SIDE = 275,
    BTN_EXTRA = 276,
    BTN_FORWARD = 277,
    BTN_BACK = 278,
    BTN_TASK = 279,
    BTN_TRIGGER = 288,
    BTN_THUMB = 289,
    BTN_THUMB2 = 290,
    BTN_TOP = 291,
    BTN_TOP2 = 292,
    BTN_PINKIE = 293,
    BTN_BASE = 294,
    BTN_BASE2 = 295,
    BTN_BASE3 = 296,
    BTN_BASE4 = 297,
    BTN_BASE5 = 298,
    BTN_BASE6 = 299,
    BTN_DEAD = 303,
    BTN_SOUTH = 304,
    BTN_EAST = 305,
    BTN_C = 306,
    BTN_NORTH = 307,
    BTN_WEST = 308,
    BTN_Z = 309,
    BTN_TL = 310,
    BTN_TR = 311,
    BTN_TL2 = 312,
    BTN_TR2 = 313,
    BTN_SELECT = 314,
    BTN_START = 315,
    BTN_MODE = 316,
    BTN_THUMBL = 317,
    BTN_THUMBR = 318,
    BTN_TOOL_PEN = 320,
    BTN_TOOL_RUBBER = 321,
    BTN_TOOL_BRUSH = 322,
    BTN_TOOL_PENCIL = 323,
    BTN_TOOL_AIRBRUSH = 324,
    BTN_TOOL_FINGER = 325,
    BTN_TOOL_MOUSE = 326,
    BTN_TOOL_LENS = 327,
    BTN_TOOL_QUINTTAP = 328,
    BTN_STYLUS3 = 329,
    BTN_TOUCH = 330,
    BTN_STYLUS = 331,
    BTN_STYLUS2 = 332,
    BTN_TOOL_DOUBLETAP = 333,
    BTN_TOOL_TRIPLETAP = 334,
    BTN_TOOL_QUADTAP = 335,
    BTN_GEAR_DOWN = 336,
    BTN_GEAR_UP = 337,
    BTN_DPAD_UP = 544,
    BTN_DPAD_DOWN = 545,
    BTN_DPAD_LEFT = 546,
    BTN_DPAD_RIGHT = 547,
    BTN_TRIGGER_HAPPY1 = 704,
    BTN_TRIGGER_HAPPY2 = 705,
    BTN_TRIGGER_HAPPY3 = 706,
    BTN_TRIGGER_HAPPY4 = 707,
    BTN_TRIGGER_HAPPY5 = 708,
    BTN_TRIGGER_HAPPY6 = 709,
    BTN_TRIGGER_HAPPY7 = 710,
    BTN_TRIGGER_HAPPY8 = 711,
    BTN_TRIGGER_HAPPY9 = 712,
    BTN_TRIGGER_HAPPY10 = 713,
    BTN_TRIGGER_HAPPY11 = 714,
    BTN_TRIGGER_HAPPY12 = 715,
    BTN_TRIGGER_HAPPY13 = 716,
    BTN_TRIGGER_HAPPY14 = 717,
    BTN_TRIGGER_HAPPY15 = 718,
    BTN_TRIGGER_HAPPY16 = 719,
    BTN_TRIGGER_HAPPY17 = 720,
    BTN_TRIGGER_HAPPY18 = 721,
    BTN_TRIGGER_HAPPY19 = 722,
    BTN_TRIGGER_HAPPY20 = 723,
    BTN_TRIGGER_HAPPY21 = 724,
    BTN_TRIGGER_HAPPY22 = 725,
    BTN_TRIGGER_HAPPY23 = 726,
    BTN_TRIGGER_HAPPY24 = 727,
    BTN_TRIGGER_HAPPY25 = 728,
    BTN_TRIGGER_HAPPY26 = 729,
    BTN_TRIGGER_HAPPY27 = 730,
    BTN_TRIGGER_HAPPY28 = 731,
    BTN_TRIGGER_HAPPY29 = 732,
    BTN_TRIGGER_HAPPY30 = 733,
    BTN_TRIGGER_HAPPY31 = 734,
    BTN_TRIGGER_HAPPY32 = 735,
    BTN_TRIGGER_HAPPY33 = 736,
    BTN_TRIGGER_HAPPY34 = 737,
    BTN_TRIGGER_HAPPY35 = 738,
    BTN_TRIGGER_HAPPY36 = 739,
    BTN_TRIGGER_HAPPY37 = 740,
    BTN_TRIGGER_HAPPY38 = 741,
    BTN_TRIGGER_HAPPY39 = 742,
    BTN_TRIGGER_HAPPY40 = 743,
}

#[repr(C, packed)]
pub struct VirtioInputAbsInfo {
    min: RW<u32>,
    max: RW<u32>,
    fuzz: RW<u32>,
    flat: RW<u32>,
    res: RW<u32>,
}

#[repr(C, packed)]
pub struct VirtioInputDevids {
    bustype: RW<u16>,
    vendor: RW<u16>,
    product: RW<u16>,
    version: RW<u16>,
}

#[repr(C, packed)]
pub struct VirtioInputConfig {
    select: RW<u8>,
    subsel: RW<u8>,
    size: RW<u8>,
    reserved: [RW<u8>; 5],
    // u: [RW<u8>; 128],
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct VirtioInputEvent {
    pub type_: u16,
    pub code: u16,
    pub value: u32,
}

impl Default for VirtioInputEvent {
    fn default() -> Self {
        VirtioInputEvent {
            type_: 0,
            code: 0,
            value: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub enum VirtioInputQueue {
    Eventq = 0,
    Statusq = 1,
}

#[allow(dead_code)]
pub struct VirtioInput {
    base: usize,
    device_type: DeviceType,
    virtqueue: [NonNull<Virtqueue>; 2],
    event_buffer: *mut VirtioInputEvent,
    curr_queue: VirtioInputQueue,
    free_desc: [bool; VIRTIO_RING_SIZE], // true if the desc is free
    desc_indexes: Option<Vec<u16>>,
    event_ack_used_index: u16,
    event_index: u16,
    status_ack_used_index: u16,
    pub event_queue: VecDeque<VirtioInputEvent>,
    pub sid: usize,
    pid: usize,
}

impl VirtioInput {
    pub fn new(base: usize, device_type: DeviceType) -> Self {
        let pm = unsafe { process_manager() };
        let layout =
            Layout::from_size_align(size_of::<VirtioInputEvent>() * EVENT_BUFFER_SIZE, 8).unwrap();
        let event_buffer = unsafe { alloc_zeroed(layout) } as *mut VirtioInputEvent;
        VirtioInput {
            base,
            device_type,
            virtqueue: [NonNull::dangling(); 2],
            event_buffer,
            curr_queue: VirtioInputQueue::Eventq,
            free_desc: [true; VIRTIO_RING_SIZE],
            desc_indexes: None,
            event_ack_used_index: 0,
            event_index: 0,
            status_ack_used_index: 0,
            event_queue: VecDeque::new(),
            sid: pm.create_semaphore(1),
            pid: 0,
        }
    }

    pub fn read_reg32(&mut self, offset: usize) -> u32 {
        virtio::read_reg32(self.base, offset)
    }

    pub fn read_reg64(&mut self, offset: usize) -> u64 {
        virtio::read_reg64(self.base, offset)
    }

    pub fn write_reg32(&mut self, offset: usize, val: u32) {
        virtio::write_reg32(self.base, offset, val)
    }

    pub fn write_reg64(&mut self, offset: usize, val: u64) {
        virtio::write_reg64(self.base, offset, val)
    }

    pub fn init_virtq(&mut self, queue: VirtioInputQueue) {
        assert_eq!(size_of::<VirtqDesc>(), 16);
        let desc_layout = Layout::from_size_align(16 * VIRTIO_RING_SIZE, 16).unwrap();
        let desc = unsafe { alloc_zeroed(desc_layout) } as *mut VirtqDesc;

        assert_eq!(size_of::<VirtqAvail>(), 6 + 2 * VIRTIO_RING_SIZE);
        let avail_layout = Layout::from_size_align(6 + 2 * VIRTIO_RING_SIZE, 2).unwrap();
        let avail = unsafe { alloc_zeroed(avail_layout) } as *mut VirtqAvail;

        assert_eq!(size_of::<VirtqUsed>(), 6 + 8 * VIRTIO_RING_SIZE);
        let used_layout = Layout::from_size_align(6 + 8 * VIRTIO_RING_SIZE, 2).unwrap();
        let used = unsafe { alloc_zeroed(used_layout) } as *mut VirtqUsed;

        assert_eq!(size_of::<Virtqueue>(), 24);
        let virtqueue_layout = Layout::from_size_align(size_of::<Virtqueue>(), 8).unwrap();
        let virtqueue = unsafe { alloc(virtqueue_layout) } as *mut Virtqueue;
        unsafe {
            *virtqueue = Virtqueue::new(desc, avail, used);
        }

        self.virtqueue[queue as usize] = NonNull::new(virtqueue).unwrap();
    }

    pub fn find_free_desc(&mut self) -> u16 {
        for (i, is_free) in self.free_desc.iter_mut().enumerate() {
            if *is_free {
                *is_free = false;
                return i as u16;
            }
        }

        panic!("free desc exhausted");
    }

    pub fn allocate_desc(&mut self, n: usize, indexes: &mut Vec<u16>) {
        for _ in 0..n {
            let index = self.find_free_desc();
            indexes.push(index);
        }
    }

    pub fn deallocate_desc(&mut self, indexes: &Vec<u16>) {
        for i in indexes.iter() {
            self.free_desc[*i as usize] = true;
        }
    }

    pub fn write_desc(&mut self, i: usize, queue: VirtioInputQueue, desc: VirtqDesc) {
        unsafe {
            let desc_ptr = self.virtqueue[queue as usize].as_mut().desc.add(i);
            *desc_ptr = desc;
        }
    }

    pub fn send_desc(&mut self, queue: VirtioInputQueue, desc_indexes: Vec<u16>) {
        unsafe {
            let virtqueue = self.virtqueue[queue as usize].as_mut();
            self.write_reg64(VirtioReg::QueueDescLow.val(), virtqueue.desc as u64);
            self.write_reg64(VirtioReg::QueueDriverLow.val(), virtqueue.avail as u64);
            self.write_reg64(VirtioReg::QueueDeviceLow.val(), virtqueue.used as u64);
            self.curr_queue = queue;

            let mut avail = virtqueue.avail.as_mut().unwrap();
            let index = avail.idx as usize;
            avail.ring[index % VIRTIO_RING_SIZE] = desc_indexes[0];
            asm!("fence iorw, iorw");
            avail.idx = avail.idx.wrapping_add(1);
            asm!("fence iorw, iorw");
            // self.desc_indexes = Some(desc_indexes);
            // pm.io_wait(self.pid);
            // self.write_reg32(VirtioReg::QueueNotify.val(), queue as u32);
            // pm.schedule();
        }
    }

    pub fn init(&mut self) {
        let pm = unsafe { process_manager() };

        pm.wait_semaphore(self.sid);

        let magic_value = self.read_reg32(VirtioReg::MagicValue.val());
        let version = self.read_reg32(VirtioReg::Version.val());
        let device_id = self.read_reg32(VirtioReg::DeviceId.val());
        if magic_value != 0x74726976 || version != 2 || device_id != 18 {
            panic!("unrecognized virtio device: {:#018x}", self.base);
        }

        let mut status_bits: u32 = 0;
        status_bits |= VirtioDeviceStatus::Acknowoledge.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        status_bits |= VirtioDeviceStatus::Driver.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        let features = self.read_reg32(VirtioReg::DeviceFeatures.val());
        self.write_reg32(VirtioReg::DeviceFeatures.val(), features);

        status_bits |= VirtioDeviceStatus::FeaturesOk.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        if self.read_reg32(VirtioReg::Status.val()) & VirtioDeviceStatus::FeaturesOk.val() == 0 {
            self.write_reg32(VirtioReg::Status.val(), VirtioDeviceStatus::Failed.val());
            panic!(
                "virtio-blk({:#018x}) does not support the required features",
                self.base
            );
        }

        self.write_reg32(VirtioReg::QueueSel.val(), 0);

        if self.read_reg32(VirtioReg::QueueReady.val()) != 0 {
            panic!("queue is already in use");
        }

        let queue_num_max = self.read_reg32(VirtioReg::QueueNumMax.val());
        if queue_num_max == 0 {
            panic!("queue is not available");
        } else if queue_num_max < (VIRTIO_RING_SIZE as u32) {
            panic!("QueueNumMax too short");
        }

        self.write_reg32(VirtioReg::QueueNum.val(), VIRTIO_RING_SIZE as u32);

        self.init_virtq(VirtioInputQueue::Eventq);
        self.init_virtq(VirtioInputQueue::Statusq);

        self.write_reg32(VirtioReg::QueueNum.val(), VIRTIO_RING_SIZE as u32);

        let virtqueue = unsafe { self.virtqueue[0].as_mut() };
        self.write_reg64(VirtioReg::QueueDescLow.val(), virtqueue.desc as u64);
        self.write_reg64(VirtioReg::QueueDriverLow.val(), virtqueue.avail as u64);
        self.write_reg64(VirtioReg::QueueDeviceLow.val(), virtqueue.used as u64);

        self.write_reg32(VirtioReg::QueueReady.val(), 1);

        status_bits |= VirtioDeviceStatus::DriverOk.val();
        self.write_reg32(VirtioReg::Status.val(), status_bits);

        pm.signal_semaphore(self.sid);
    }

    pub fn setup_config(&mut self) {
        let pm = unsafe { process_manager() };
        pm.wait_semaphore(self.sid);

        let config = (self.base + VirtioReg::Config as usize) as *mut VirtioInputConfig;
        unsafe {
            pm.io_wait(self.pid);
            (*config)
                .select
                .write(VirtioInputConfigSelect::InputCfgIdName.val());
            pm.schedule();

            let mut name = String::new();
            let size = (*config).size.read();
            let u = (self.base + VirtioReg::Config as usize + 8) as *mut u8;
            for i in 0..size as usize {
                let ch = u.add(i).read_volatile() as char;
                name.push(ch);
            }
            println!("input device detected: {}", name);

            let event_type = match self.device_type {
                DeviceType::Mouse => EventType::EV_REL,
                DeviceType::Keyboard => EventType::EV_KEY,
            };

            pm.io_wait(self.pid);
            (*config).subsel.write(event_type as u8);
            (*config)
                .select
                .write(VirtioInputConfigSelect::InputCfgEvBits as u8);
            pm.schedule();
        }

        pm.signal_semaphore(self.sid);
    }

    pub fn repopulate_event(&mut self, i: usize) {
        let buffer = unsafe { self.event_buffer.add(i) };
        let flag = VirtqDescFlag::VirtqDescFWrite.val();
        let desc = VirtqDesc::new(buffer as u64, size_of::<VirtioInputEvent>() as u32, flag, 0);

        let head = self.event_index;
        self.write_desc(self.event_index as usize, VirtioInputQueue::Eventq, desc);
        self.event_index = (self.event_index + 1) % VIRTIO_RING_SIZE as u16;

        let desc_indexes = vec![head];
        self.send_desc(VirtioInputQueue::Eventq, desc_indexes);
    }

    pub fn init_input_event(&mut self) {
        // self.setup_config();

        let pm = unsafe { process_manager() };
        pm.wait_semaphore(self.sid);

        self.pid = pm.running;

        for i in 0..(EVENT_BUFFER_SIZE / 2) {
            self.repopulate_event(i);
        }

        // self.send_desc(VirtioInputQueue::Eventq, desc_indexes);
        pm.signal_semaphore(self.sid);
    }

    pub fn pending(&mut self) {
        let mask = interrupt_disable();

        let interrupt_status = self.read_reg32(VirtioReg::InterruptStatus.val());
        self.write_reg32(VirtioReg::InterruptACK.val(), interrupt_status & 0x3);
        let virtqueue = unsafe { self.virtqueue[self.curr_queue as usize].as_mut() };
        let desc = virtqueue.desc;
        let used = unsafe { virtqueue.used.as_mut().unwrap() };

        while self.event_ack_used_index != used.idx {
            let index = self.event_ack_used_index % VIRTIO_RING_SIZE as u16;
            let elem = used.ring[index as usize];

            self.repopulate_event(elem.id as usize);

            self.event_ack_used_index = self.event_ack_used_index.wrapping_add(1);
            unsafe {
                let desc = desc.add(elem.id as usize).as_mut().unwrap();
                let event = (desc.addr as *mut VirtioInputEvent).as_mut().unwrap();
                // println!("event: {:?}", event);
                self.event_queue.push_back(*event);
            }
        }

        let pm = unsafe { process_manager() };
        match self.device_type {
            DeviceType::Mouse => pm.event_signal(ProcessEvent::MouseEvent),
            DeviceType::Keyboard => pm.event_signal(ProcessEvent::KeyboardEvent),
        }

        interrupt_restore(mask);
    }
}

pub fn init(base: usize, device_type: DeviceType) -> VirtioInput {
    let mut input = VirtioInput::new(base, device_type);
    input.init();
    input
}
