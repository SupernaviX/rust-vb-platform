use bitfield_struct::bitfield;

use super::volatile::{mmio, mmstruct};

#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct Character(pub [u16; 8]);

mmio! {
    pub const CHARACTERS: [Character; 2048] = 0x00078000;
    pub const CHARACTER_HWS: [u16; 2048 * 8] = 0x00078000;
}

/// A description of a character and how to display it.
/// Used to describe a single object, or part of a background.
#[bitfield(u16)]
pub struct Cell {
    /// The index of the character to draw.
    #[bits(11)]
    pub character: u16,
    _pad: bool,
    /// If set, the character graphic will be reversed vertically.
    #[doc(alias = "bvflp")]
    #[doc(alias = "jvflp")]
    pub v_flip: bool,
    /// If set, the character graphic will be reversed horizontally.
    #[doc(alias = "bvflp")]
    #[doc(alias = "jvflp")]
    pub h_flip: bool,
    /// Specifies the palette index to use for this cell.
    #[doc(alias = "gplts")]
    #[doc(alias = "jplts")]
    #[bits(2)]
    pub palette: u8,
}

mmstruct! {
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct HBiasElement {
        /// The signed horizontal offset to apply to the row of pixels for the left eye.
        pub hofstl: i16,
        /// The signed horizontal offset to apply to the row of pixels for the right eye.
        pub hofstr: i16,
    }
}

mmstruct! {
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct AffineElement {
        pub mx: fixed::types::I13F3,
        pub mp: i16,
        pub my: fixed::types::I13F3,
        pub dx: fixed::types::I7F9,
        pub dy: fixed::types::I7F9,
        _padding: [i16; 3],
    }
}

mmio! {
    pub const BG_CELLS: [Cell; 64 * 64 * 16] = 0x00020000, size = 0x20000;
    pub const BG_MAPS: [[Cell; 64 * 64]; 16] = 0x00020000, size = 0x20000;
    pub const HBIAS: [HBiasElement; 0x7600] = 0x00020000, size = 0x1d800;
    pub const AFFINE: [AffineElement; 0x1d80] = 0x00020000, size = 0x1d800;
    pub const WORLD_PARAMS: [u16; 0xec00] = 0x00020000, size = 0x1d800;
}

/// Describes the contents of a world.
#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WorldMode {
    Normal = 0,
    HBias = 1,
    Affine = 2,
    Object = 3,
}
impl WorldMode {
    const fn into_bits(self) -> u8 {
        self as _
    }
    const fn from_bits(value: u8) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::HBias,
            2 => Self::Affine,
            _ => Self::Object,
        }
    }
}

#[bitfield(u16)]
pub struct WorldHeader {
    /// The index of the first background map in the world's background.
    #[bits(4)]
    pub bg_map_base: u8,
    #[bits(2)]
    _pad: u8,
    /// If set, this world and all worlds of lesser index will not be drawn to the frame buffer.
    pub end: bool,
    /// If clear, the world's background will repeat indefinitely. If set, characters beyond the background's bounds will use the character specified by Overplane Character.
    pub over: bool,
    /// Raise 2 to this power for the height of the world's background in background maps.
    #[bits(2)]
    pub scy: u8,
    /// Raise 2 to this power for the width of the world's background in background maps.
    #[bits(2)]
    pub scx: u8,
    /// Indicates the world's contents.
    #[bits(2)]
    pub bgm: WorldMode,
    /// If set, the world will be drawn to the right image.
    pub ron: bool,
    /// If set, the world will be drawn to the left image.
    pub lon: bool,
}

mmstruct! {
    #[repr(C, align(4))]
    #[derive(Clone, Copy)]
    pub struct World {
        /// Describes the world.
        pub header: WorldHeader,
        /// The signed horizontal coordinate of the left edge of the world from the left edge of the image.
        pub gx: i16,
        /// The signed parallax offset applied to the world's horizontal coordinate.
        pub gp: i16,
        /// The signed vertical coordinate of the top edge of the world from the top edge of the image.
        pub gy: i16,
        /// The signed horizontal source coordinate of the pixel within the world's background, relative to the top-left corner of the background, to be displayed in the top-left corner of the world.
        pub mx: i16,
        /// The signed parallax offset applied to the background's horizontal source coordinate.
        pub mp: i16,
        /// The signed vertical source coordinate of the pixel within the world's background, relative to the top-left corner of the background, to be displayed in the top-left corner of the world.
        pub my: i16,
        /// Add 1 to this figure to yield the width in pixels of the world. This field's format depends on BGM.
        pub w: i16,
        /// Add 1 to this figure to yield the height in pixels of the world.
        pub h: i16,
        /// Specifies the location in world parameter memory where this world's parameters can be found.
        pub param_base: u16,
        /// When OVER is set, characters beyond the background's bounds will use the cell in background map memory at the index given by this field.
        pub overplane_character: u16,
        _pad: [u16; 5],
    }
}

mmio! {
    pub const WORLDS: [World; 32] = 0x0003d800;
}

#[bitfield(u16)]
pub struct ObjectStereo {
    /// The signed parallax offset applied to the horizontal coordinate.
    #[bits(10)]
    pub jp: i16,
    #[bits(4)]
    _pad: u16,
    /// If set, the object will be drawn to the right image.
    pub jron: bool,
    /// If set, the object will be drawn to the left image.
    pub jlon: bool,
}

mmstruct! {
    #[repr(C, align(4))]
    #[derive(Clone, Copy)]
    pub struct Object {
        /// The signed horizontal coordinate of the left edge of the object from the left edge of the image.
        pub jx: i16,
        /// How to render the object in each eye.
        pub stereo: ObjectStereo,
        /// The vertical coordinate of the top edge of the object from the top edge of the image.
        pub jy: i16,
        /// The character to draw, and other information about how to draw it.
        pub cell: Cell,
    }
}

mmio! {
    pub const OBJS: [Object; 1024] = 0x0003e000;
}

#[bitfield(u16)]
pub struct InterruptFlags {
    /// The mirrors are not stable.
    pub scanerr: bool,
    /// The display procedure has completed for the left eye.
    pub lfbend: bool,
    /// The display procedure has completed for the right eye.
    pub rfbend: bool,
    /// The drawing procedure has begun.
    pub gamestart: bool,
    /// The display procedure has begun.
    pub framestart: bool,
    #[bits(8)]
    _pad: u16,
    /// Drawing has begun on the group of 8 rows of pixels specified in the SBCMP field of XPCTRL.
    pub sbhit: bool,
    /// The drawing procedure has finished.
    pub xpend: bool,
    /// Drawing is still in progress when the drawing procedure should begin. Detects the OVERTIME flag in XPSTTS.
    pub timeerr: bool,
}

mmio! {
    pub const INTPND: InterruptFlags = 0x0005f800;
    pub const INTENB: InterruptFlags = 0x0005f802;
    pub const INTCLR: InterruptFlags = 0x0005f804;
}

#[bitfield(u16)]
pub struct DisplayFlags {
    /// When set, display functions are reset.
    pub dprst: bool,
    /// When set, the display is enabled.
    pub disp: bool,
    /// Left frame buffer 0 is being displayed.
    pub l0bsy: bool,
    /// Right frame buffer 0 is being displayed.
    pub r0bsy: bool,
    /// Left frame buffer 1 is being displayed.
    pub l1bsy: bool,
    /// Right frame buffer 1 is being displayed.
    pub r1bsy: bool,
    /// When set, the mirrors are stable.
    pub scanrdy: bool,
    /// The display frame clock signal is high.
    pub fclk: bool,
    /// When clear, memory refresh signals will not be issued on VIP memory.
    pub re: bool,
    /// When clear, display sync signals are not sent to the display servo, preventing images from being displayed.
    pub synce: bool,
    /// When set, CTA is prevented from updating.
    pub lock: bool,
    #[bits(5)]
    _pad: u16,
}

mmio! {
    pub const DPSTTS: DisplayFlags = 0x0005f820;
    pub const DPCTRL: DisplayFlags = 0x0005f822;

    pub const BRTA: u16 = 0x0005f824;
    pub const BRTB: u16 = 0x0005f826;
    pub const BRTC: u16 = 0x0005f828;
    pub const REST: u16 = 0x0005f82a;
}

#[bitfield(u16)]
pub struct DrawingFlags {
    /// When set, drawing functions are reset. When clear, no action occurs.
    pub xprst: bool,
    /// When set, drawing is enabled.
    pub xpen: bool,
    /// Frame buffer 0 is being drawn to.
    pub f0bsy: bool,
    /// Frame buffer 1 is being drawn to.
    pub f1bsy: bool,
    /// The drawing procedure has taken longer than the alloted time.
    pub overtime: bool,
    #[bits(3)]
    _pad0: u16,
    /// When read: the current group of 8 rows of pixels, relative to the top of the image, currently being drawn.
    /// When written: the group of 8 rows of pixels, relative to the top of the image, to compare to while drawing.
    #[doc(alias = "sbcmp")]
    #[bits(5)]
    pub sbcount: u16,
    #[bits(2)]
    _pad1: u16,
    /// Set when a group of 8 rows of pixels begens to draw.
    pub sbout: bool,
}

mmio! {
    pub const XPSTTS: DrawingFlags = 0x0005f840;
    pub const XPCTRL: DrawingFlags = 0x0005f842;

    pub const SPT: [u16; 4] = 0x0005f848;
    pub const SPT0: u16 = 0x0005f848;
    pub const SPT1: u16 = 0x0005f84a;
    pub const SPT2: u16 = 0x0005f84c;
    pub const SPT3: u16 = 0x0005f84e;
}

#[bitfield(u16)]
pub struct Palette {
    #[bits(2)]
    _pad1: u8,
    /// The frame buffer pixel value for character pixel value 1.
    #[bits(2)]
    pub c1: u8,
    /// The frame buffer pixel value for character pixel value 2.
    #[bits(2)]
    pub c2: u8,
    /// The frame buffer pixel value for character pixel value 3.
    #[bits(2)]
    pub c3: u8,
    _pad2: u8,
}

mmio! {
    pub const GPLT: [Palette; 4] = 0x0005f860;
    pub const GPLT0: Palette = 0x0005f860;
    pub const GPLT1: Palette = 0x0005f862;
    pub const GPLT2: Palette = 0x0005f864;
    pub const GPLT3: Palette = 0x0005f866;
    pub const JPLT: [Palette; 4] = 0x0005f868;
    pub const JPLT0: Palette = 0x0005f868;
    pub const JPLT1: Palette = 0x0005f86a;
    pub const JPLT2: Palette = 0x0005f86c;
    pub const JPLT3: Palette = 0x0005f86e;
    pub const BKCOL: u16 = 0x0005f870;
}
