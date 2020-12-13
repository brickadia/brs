use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::fmt;
use uuid::Uuid;

/// A single brick in a save file.
///
/// `asset_name_index`, `material_index`, `owner_index` and the `Set` variant
/// of `color` target the lookup tables in either
/// [`WriteData`](struct.WriteData.html) or the headers, when reading.
///
/// `size` is used for procedural bricks. For fixed size brick assets, it's
/// more efficient to use `(0, 0, 0)` (the file will be smaller).
#[derive(Debug, Clone, PartialEq)]
pub struct Brick {
    pub asset_name_index: u32,
    pub size: (u32, u32, u32),
    pub position: (i32, i32, i32),
    pub direction: Direction,
    pub rotation: Rotation,
    pub collision: bool,
    pub visibility: bool,
    pub material_index: u32,
    pub color: ColorMode,
    pub owner_index: Option<u32>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
pub enum Direction {
    XPositive,
    XNegative,
    YPositive,
    YNegative,
    ZPositive,
    ZNegative,
}

#[repr(u8)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IntoPrimitive, TryFromPrimitive,
)]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    /// A color from the color lookup table.
    Set(u32),
    /// A custom color.
    Custom(Color),
}

/// Represents a RGBA color.
#[derive(Clone, Copy, PartialEq, Hash)]
pub struct Color(u32);

impl Color {
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(u32::from(b) | (u32::from(g) << 8) | (u32::from(r) << 16) | (u32::from(a) << 24))
    }
    pub fn r(self) -> u8 {
        ((self.0 >> 16) & 0xff) as u8
    }
    pub fn g(self) -> u8 {
        ((self.0 >> 8) & 0xff) as u8
    }
    pub fn b(self) -> u8 {
        (self.0 & 0xff) as u8
    }
    pub fn a(self) -> u8 {
        ((self.0 >> 24) & 0xff) as u8
    }
}

impl Into<Color> for u32 {
    fn into(self) -> Color {
        Color(self)
    }
}

impl Into<u32> for Color {
    fn into(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "#{:02x}{:02x}{:02x}{:02x}",
            self.r(),
            self.g(),
            self.b(),
            self.a()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct User {
    pub id: Uuid,
    pub name: String,
}

impl Default for User {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            name: "Unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct BrickOwner {
    pub user: User,
    pub brick_count: u32,
}

pub(crate) const SCREENSHOT_NONE: u8 = 0;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, IntoPrimitive, TryFromPrimitive)]
pub enum ScreenshotFormat {
    Png = 1,
}

pub struct Screenshot<R> {
    pub format: ScreenshotFormat,
    pub data: R,
}

/// A save file version.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, IntoPrimitive, TryFromPrimitive)]
pub enum Version {
    Initial = 1, // 0.2.0-0.2.4
    MaterialsStoredAsNames, // 0.3.0
    AddedOwnerData,
    AddedDateTime, // Alpha 4 Patch 1
    AddedComponentsData,
    AddedScreenshotData,
    AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials,
    RenamedComponentDescriptors, // Alpha 5 (QA)
}

impl Version {
    pub fn first_game_version(self) -> u32 {
        match self {
            Version::Initial => todo!(),
            Version::MaterialsStoredAsNames => todo!(),
            Version::AddedOwnerData => todo!(),
            Version::AddedDateTime => 3642,
            Version::AddedComponentsData => todo!(),
            Version::AddedScreenshotData => todo!(),
            Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials => todo!(),
            Version::RenamedComponentDescriptors => todo!(),
        }
    }
}

// MaterialsStoredAsNames was added between 0.2.4 and 0.3.0, so this
// allows us to prevent loading 0.2 saves (which are broken)
// TODO: Figure out if this can be removed
pub const MIN_VERSION_READ: Version = Version::MaterialsStoredAsNames;

pub const MAX_VERSION_READ: Version = Version::RenamedComponentDescriptors;
