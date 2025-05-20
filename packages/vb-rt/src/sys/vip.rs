use bitfield_struct::bitfield;

use super::volatile::mmio;

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

    pub const BKCOL: u16 = 0x0005f870;
}
