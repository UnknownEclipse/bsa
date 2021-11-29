use bitflags::bitflags;
use num_enum::{IntoPrimitive, TryFromPrimitive};

macro_rules! read_u32 {
    ($bytes:expr, $index:expr) => {
        u32::from_le_bytes($bytes[$index * 4..($index + 1) * 4].try_into().unwrap())
    };
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Header {
    pub flags: HeaderFlags,
    pub height: u32,
    pub width: u32,
    pub pitch_or_linear_size: u32,
    pub depth: u32,
    pub mipmap_count: u32,
    pub pixel_format: PixelFormat,
    pub caps: Caps,
    pub caps2: Caps2,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct HeaderDx10 {
    pub format: DxgiFormat,
    pub dimension: Dimension,
    pub misc_flags: MiscFlags,
    pub array_size: u32,
    pub alpha_mode: AlphaMode,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PixelFormat {
    pub flags: PixelFormatFlags,
    pub fourcc: FourCc,
    pub rgb_bit_count: u32,
    pub red_bit_mask: u32,
    pub green_bit_mask: u32,
    pub blue_bit_mask: u32,
    pub alpha_bit_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Dimension {
    Texture1D = 2,
    Texture2D = 3,
    Texture3D = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum AlphaMode {
    Unknown = 0x0,
    Straight = 0x1,
    Premultiplied = 0x2,
    Opaque = 0x3,
    Custom = 0x4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FourCc([u8; 4]);

bitflags! {
    pub struct PixelFormatFlags: u32 {
        const ALPHA_PIXELS = 0x1;
        const ALPHA = 0x2;
        const FOURCC = 0x4;
        const RGB = 0x40;
        const YUV = 0x200;
        const LUMINANCE = 0x20000;
    }
}

bitflags! {
    pub struct HeaderFlags: u32 {
        const CAPS = 0x1;
        const HEIGHT = 0x2;
        const WIDTH = 0x4;
        const PITCH = 0x8;
        const PIXEL_FORMAT = 0x1000;
        const MIPMAP_COUNT = 0x20000;
        const LINEAR_SIZE = 0x80000;
        const DEPTH = 0x800000;
    }
}

bitflags! {
    pub struct Caps: u32 {
        const COMPLEX = 0x8;
        const MIPMAP = 0x400000;
        const TEXTURE = 0x1000;
    }
}

bitflags! {
    pub struct Caps2: u32 {
        const CUBEMAP = 0x200;
        const CUBEMAP_POSITIVE_X = 0x400;
        const CUBEMAP_NEGATIVE_X = 0x800;
        const CUBEMAP_POSITIVE_Y = 0x1000;
        const CUBEMAP_NEGATIVE_Y = 0x2000;
        const CUBEMAP_POSITIVE_Z = 0x4000;
        const CUBEMAP_NEGATIVE_Z = 0x8000;
        const VOLUME = 0x200000;
    }
}

bitflags! {
    pub struct MiscFlags: u32 {
        const CUBEMAP = 0x4;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum DxgiFormat {
    Bc7,
}

impl Header {
    pub fn from_bytes(bytes: [u8; 124]) -> Option<Header> {
        let size = read_u32!(bytes, 0);
        if size != 124 {
            return None;
        }

        let flags = read_u32!(bytes, 1);
        let flags = HeaderFlags::from_bits(flags)?;
        let height = read_u32!(bytes, 2);
        let width = read_u32!(bytes, 3);
        let pitch_or_linear_size = read_u32!(bytes, 4);
        let depth = read_u32!(bytes, 5);
        let mipmap_count = read_u32!(bytes, 6);
        let pixel_format = PixelFormat::from_bytes(bytes[18 * 4..18 * 4 + 32].try_into().unwrap())?;
        let caps = read_u32!(bytes, 26);
        let caps = Caps::from_bits(caps)?;
        let caps2 = read_u32!(bytes, 27);
        let caps2 = Caps2::from_bits(caps2)?;

        Some(Header {
            flags,
            height,
            width,
            pitch_or_linear_size,
            depth,
            mipmap_count,
            pixel_format,
            caps,
            caps2,
        })
    }
}

impl PixelFormat {
    pub fn from_bytes(bytes: [u8; 32]) -> Option<PixelFormat> {
        let mut chunks = bytes.chunks(4);
        let mut next_dword = || u32::from_le_bytes(chunks.next().unwrap().try_into().unwrap());

        let size = next_dword();
        if size != 32 {
            return None;
        }

        let flags = next_dword();
        let flags = PixelFormatFlags::from_bits(flags)?;
        let fourcc = next_dword();
        let fourcc = FourCc(fourcc.to_le_bytes());
        let rgb_bit_count = next_dword();
        let red_bit_mask = next_dword();
        let green_bit_mask = next_dword();
        let blue_bit_mask = next_dword();
        let alpha_bit_mask = next_dword();

        let pixel_format = PixelFormat {
            flags,
            fourcc,
            rgb_bit_count,
            red_bit_mask,
            green_bit_mask,
            blue_bit_mask,
            alpha_bit_mask,
        };

        Some(pixel_format)
    }
}
