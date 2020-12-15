use crate::{
    bit_reader::BitReader,
    save::{
        Brick, BrickOwner, Color, ColorMode, Direction, Rotation, Screenshot, ScreenshotFormat,
        User, SCREENSHOT_NONE,
    },
    ue4_date_time_base, Version, MAGIC,
};
use byteorder::{BigEndian, ByteOrder, LittleEndian, ReadBytesExt};
use chrono::{prelude::*, Duration};
use flate2::bufread::ZlibDecoder;
use std::{
    convert::TryInto,
    io::{self, prelude::*, Cursor},
};
use uuid::Uuid;

mod sealed {
    use std::fmt::Debug;
    pub trait Sealed {}
    pub trait ReaderState: Sealed + Debug {}
}

pub use sealed::ReaderState;
use sealed::Sealed;

#[derive(Debug)]
struct SharedReaderData<R> {
    r: R,
    version: Version,
    game_version: u32,
    header1: Header1,
    header2: Header2,
    screenshot_info: Option<(ScreenshotFormat, u32)>,
}

#[derive(Debug)]
pub struct Reader<R, S: ReaderState> {
    shared: Box<SharedReaderData<R>>,
    state: S,
}

#[derive(Debug)]
pub struct Init;
impl Sealed for Init {}
impl ReaderState for Init {}

impl<R: BufRead> Reader<R, Init> {
    /// Create a new reader that reads from `r`.
    ///
    /// ```no_run
    /// # use std::{fs::File, io::BufReader};
    /// # use brs::Reader;
    /// let reader = Reader::new(BufReader::new(File::open("village.brs")?))?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn new(mut r: R) -> io::Result<Self> {
        let mut magic = [0; 3];
        r.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid starting bytes",
            ));
        }

        let version: Version = r
            .read_u16::<LittleEndian>()?
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unsupported version"))?;

        let game_version;

        if version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
            game_version = r.read_u32::<LittleEndian>()?;
        } else {
            // TODO: Consider providing the first or last game version
            // that used this save version
            game_version = 3642;
        };

        let header1 = read_header1(&mut read_compressed(&mut r)?, version)?;
        let header2 = read_header2(&mut read_compressed(&mut r)?, version)?;

        let screenshot_info = if version >= Version::AddedScreenshotData {
            let format_raw = r.read_u8()?;
            if format_raw != SCREENSHOT_NONE {
                let format = format_raw.try_into().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Unknown screenshot format")
                })?;
                let len = r.read_u32::<LittleEndian>()?;
                Some((format, len))
            } else {
                None
            }
        } else {
            None
        };

        Ok(Reader {
            shared: Box::new(SharedReaderData {
                r,
                version,
                game_version,
                header1,
                header2,
                screenshot_info,
            }),
            state: Init,
        })
    }

    /// Read the raw screenshot image data. Only meaningful if
    /// [`screenshot_format`](struct.Reader.html#method.screenshot_format) is not
    /// [`ScreenshotFormat::None`](enum.ScreenshotFormat.html#variant.None).
    pub fn screenshot_data(
        mut self,
    ) -> io::Result<(Reader<R, AfterScreenshot>, Option<Screenshot<Vec<u8>>>)> {
        let screenshot = if let Some((format, len)) = self.shared.screenshot_info {
            let mut data = vec![0; len.try_into().expect("u32 into usize")];
            self.shared.r.read_exact(&mut data)?;
            Some(Screenshot { format, data })
        } else {
            None
        };
        Ok((
            Reader {
                shared: self.shared,
                state: AfterScreenshot,
            },
            screenshot,
        ))
    }

    fn skip_screenshot(mut self) -> io::Result<Reader<R, AfterScreenshot>> {
        if let Some((_, len)) = self.shared.screenshot_info {
            skip_read(&mut self.shared.r, len.into())?;
        }
        Ok(Reader {
            shared: self.shared,
            state: AfterScreenshot,
        })
    }

    /// Skip over the screenshot, and begin iterating over the bricks.
    ///
    /// ```no_run
    /// # let reader: brs::Reader<std::io::Empty, brs::read::Init> = unimplemented!();
    /// let (reader, bricks) = reader.bricks()?;
    ///
    /// for brick in bricks {
    ///     let brick = brick?;
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn bricks(self) -> io::Result<(Reader<R, AfterBricks>, ReadBricks)> {
        self.skip_screenshot()?.bricks()
    }

    /// Read the bricks and create a [`WriteData`](../struct.WriteData.html)
    /// for use with [`write_save`](../fn.write_save.html),
    /// which can be used to write a save file with identical content.
    ///
    /// ```no_run
    /// # use std::fs::File;
    /// # let reader: brs::Reader<std::io::Empty, brs::read::Init> = unimplemented!();
    /// let data = reader.into_write_data()?;
    /// brs::write_save(&mut File::create("park.brs")?, &data)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn into_write_data(self) -> io::Result<crate::WriteData> {
        let (reader, screenshot) = self.screenshot_data()?;
        let (reader, bricks) = reader.bricks()?;
        let bricks = bricks.collect::<Result<_, _>>()?;

        let header1 = reader.shared.header1;
        let header2 = reader.shared.header2;

        Ok(crate::WriteData {
            map: header1.map,
            author: header1.author,
            description: header1.description,
            save_time: header1.save_time.unwrap_or_else(Utc::now),

            mods: header2.mods,
            brick_assets: header2.brick_assets,
            colors: header2.colors,
            materials: header2.materials,
            brick_owners: header2.brick_owners.into_iter().map(|o| o.user).collect(),

            bricks,
        })
    }
}

#[derive(Debug)]
pub struct AfterScreenshot;
impl Sealed for AfterScreenshot {}
impl ReaderState for AfterScreenshot {}

impl<R: BufRead> Reader<R, AfterScreenshot> {
    /// Begin parsing the bricks and return an iterator over them.
    ///
    /// ```no_run
    /// # let reader: brs::Reader<std::io::Empty, brs::read::AfterScreenshot> = unimplemented!();
    /// let (reader, bricks) = reader.bricks()?;
    ///
    /// for brick in bricks {
    ///     let brick = brick?;
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn bricks(mut self) -> io::Result<(Reader<R, AfterBricks>, ReadBricks)> {
        let mut brick_section = read_compressed(&mut self.shared.r)?;
        let bricks_iter = read_bricks(
            &mut brick_section,
            self.shared.version,
            &self.shared.header1,
            &self.shared.header2,
        )?;
        Ok((
            Reader {
                shared: self.shared,
                state: AfterBricks,
            },
            bricks_iter,
        ))
    }
}

#[derive(Debug)]
pub struct AfterBricks;
impl Sealed for AfterBricks {}
impl ReaderState for AfterBricks {}

impl<R: BufRead> Reader<R, AfterBricks> {
    /// Parse the components partially and return an interface for further access.
    ///
    /// ```no_run
    /// # let reader: brs::Reader<std::io::Empty, brs::read::AfterBricks> = unimplemented!();
    /// let (reader, components0) = reader.components()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn components(mut self) -> io::Result<(Reader<R, AfterComponents>, Components)> {
        let iter = if self.shared.version < Version::AddedComponentsData {
            Components::empty()
        } else {
            let section = read_compressed(&mut self.shared.r)?;
            Components::read(
                section.into_inner(),
                self.shared.version,
                &self.shared.header1,
                //&self.shared.header2,
            )?
        };
        Ok((
            Reader {
                shared: self.shared,
                state: AfterComponents,
            },
            iter,
        ))
    }
}

#[derive(Debug)]
pub struct AfterComponents;
impl Sealed for AfterComponents {}
impl ReaderState for AfterComponents {}

impl<R, S: ReaderState> Reader<R, S> {
    pub fn format_version(&self) -> Version {
        self.shared.version
    }

    /// The numeric game version (CLxxxx).
    pub fn game_changelist(&self) -> u32 {
        self.shared.game_version
    }

    /// Do the `brick_count` values of the `BrickOwner` entries in this build have meaningful values?
    /// Available in builds saved in Alpha 5+. Value is `0` otherwise.
    pub fn has_brick_owner_counts(&self) -> bool {
        self.shared.version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials
    }

    pub fn map(&self) -> &str {
        &self.shared.header1.map
    }

    pub fn author(&self) -> &User {
        &self.shared.header1.author
    }

    pub fn description(&self) -> &str {
        &self.shared.header1.description
    }

    pub fn save_time(&self) -> Option<&DateTime<Utc>> {
        self.shared.header1.save_time.as_ref()
    }

    pub fn brick_count(&self) -> u32 {
        self.shared.header1.brick_count
    }

    pub fn mods(&self) -> &[String] {
        &self.shared.header2.mods[..]
    }

    pub fn brick_assets(&self) -> &[String] {
        &self.shared.header2.brick_assets[..]
    }

    pub fn colors(&self) -> &[Color] {
        &self.shared.header2.colors[..]
    }

    pub fn materials(&self) -> &[String] {
        &self.shared.header2.materials[..]
    }

    pub fn brick_owners(&self) -> &[BrickOwner] {
        &self.shared.header2.brick_owners[..]
    }

    pub fn screenshot_format(&self) -> Option<ScreenshotFormat> {
        self.shared.screenshot_info.map(|(format, _)| format)
    }
}

#[derive(Debug, Clone)]
pub struct Header1 {
    pub map: String,
    pub author: User,
    pub description: String,
    pub host: Option<User>,
    pub save_time: Option<DateTime<Utc>>,
    pub brick_count: u32,
}

#[derive(Debug, Clone)]
pub struct Header2 {
    pub mods: Vec<String>,
    pub brick_assets: Vec<String>,
    pub colors: Vec<Color>,
    pub materials: Vec<String>,
    pub brick_owners: Vec<BrickOwner>,
}

fn read_header1(r: &mut impl Read, version: Version) -> io::Result<Header1> {
    let map = string(r)?;
    let author_name = string(r)?;
    let description = string(r)?;
    let author_id = uuid(r)?;

    let host = if version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
        Some(read_user_name_first(r)?)
    } else {
        None
    };

    let save_time = if version >= Version::AddedDateTime {
        Some(date_time(r)?)
    } else {
        None
    };

    let brick_count = r.read_i32::<LittleEndian>()?.try_into().unwrap_or(0);

    Ok(Header1 {
        map,
        author: User {
            id: author_id,
            name: author_name,
        },
        description,
        host,
        save_time,
        brick_count,
    })
}

fn read_header2(r: &mut impl Read, version: Version) -> io::Result<Header2> {
    let mods = array(r, string)?;
    let brick_assets = array(r, string)?;
    let colors = array(r, |r| r.read_u32::<LittleEndian>().map(Into::into))?;

    let materials = if version >= Version::MaterialsStoredAsNames {
        array(r, string)?
    } else {
        vec!["BMC_Hologram", "BMC_Plastic", "BMC_Glow", "BMC_Metallic"]
            .into_iter()
            .map(String::from)
            .collect()
    };

    let brick_owners =
        if version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
            //(*BitArchive) << Data.BrickOwners;
            array(r, read_brick_owner)?
        } else if version >= Version::AddedOwnerData {
            array(r, read_user)?
                .into_iter()
                .map(|user| BrickOwner {
                    user,
                    brick_count: 0,
                })
                .collect()
        } else {
            Vec::new()
        };

    Ok(Header2 {
        mods,
        brick_assets,
        colors,
        materials,
        brick_owners,
    })
}

pub struct ReadBricks {
    version: Version,
    r: BitReader<Vec<u8>>,
    brick_asset_num: u32,
    color_num: u32,
    material_num: u32,
    brick_count: u32,
    index: u32,
}

fn read_bricks(
    r: &mut impl Read,
    version: Version,
    header1: &Header1,
    header2: &Header2,
) -> io::Result<ReadBricks> {
    let mut buf = vec![];
    r.read_to_end(&mut buf)?;
    Ok(ReadBricks {
        version,
        r: BitReader::new(buf),
        brick_asset_num: header2.brick_assets.len() as u32,
        color_num: header2.colors.len() as u32,
        material_num: header2.materials.len() as u32,
        brick_count: header1.brick_count,
        index: 0,
    })
}

impl ReadBricks {
    fn read_brick(&mut self) -> io::Result<Brick> {
        self.r.eat_byte_align();
        let asset_name_index = self.r.read_int(self.brick_asset_num.max(2));
        let size = if self.r.read_bit() {
            self.r.read_positive_int_vector_packed()
        } else {
            (0, 0, 0)
        };
        let position = self.r.read_int_vector_packed();
        let orientation = self.r.read_int(24) as u8;
        let collision = self.r.read_bit();
        let visibility = self.r.read_bit();

        let material_index =
            if self.version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
                self.r.read_int(self.material_num.max(2))
            } else if self.r.read_bit() {
                self.r.read_int_packed()
            } else {
                1
            };

        let color = if !self.r.read_bit() {
            ColorMode::Set(self.r.read_int(self.color_num))
        } else {
            ColorMode::Custom(self.r.read_u32::<LittleEndian>()?.into())
        };

        let owner_index = if self.version >= Version::AddedOwnerData {
            self.r.read_int_packed()
        } else {
            0
        };
        let owner_index = match owner_index {
            0 => None,
            n => Some(n - 1),
        };

        let (direction, rotation) = split_orientation(orientation);

        Ok(Brick {
            asset_name_index,
            size,
            position,
            direction,
            rotation,
            collision,
            visibility,
            material_index,
            color,
            owner_index,
        })
    }
}

impl Iterator for ReadBricks {
    type Item = io::Result<Brick>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.brick_count {
            let result = self.read_brick();

            if result.is_ok() {
                self.index += 1;
            } else {
                self.index = self.brick_count;
            }

            Some(result)
        } else {
            None
        }
    }
}

pub struct Components {
    compressed: BitReader<Vec<u8>>,
    version: Version,
    brick_count: u32,
    components: Vec<ComponentEntry>,
}

impl Components {
    fn empty() -> Self {
        Self {
            compressed: BitReader::new(Vec::new()),
            version: Version::Initial,
            brick_count: 0,
            components: Vec::new(),
        }
    }

    fn read(compressed: Vec<u8>, version: Version, header1: &Header1) -> io::Result<Self> {
        let mut compressed = BitReader::new(compressed);
        let filled_components = compressed
            .read_i32::<LittleEndian>()?
            .try_into()
            .unwrap_or(0);
        let mut components = Vec::with_capacity(filled_components);

        for _ in 0..filled_components {
            compressed.eat_byte_align();

            let mut name = string(&mut compressed).unwrap();
            if version < Version::RenamedComponentDescriptors {
                // TODO: Ignore case
                name = name.replace("BTD", "BCD");
            }

            let data_len: usize = compressed
                .read_u32::<LittleEndian>()
                .unwrap()
                .try_into()
                .expect("u32 -> usize failed");
            let data_pos = compressed.pos();
            compressed.seek(data_pos + (data_len << 3));

            components.push(ComponentEntry {
                name,
                data_len,
                data_pos,
            });
        }

        Ok(Self {
            compressed,
            version,
            brick_count: header1.brick_count,
            components,
        })
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.components.iter().map(|e| e.name.as_str())
    }
}

struct ComponentEntry {
    name: String,
    data_len: usize,
    data_pos: usize,
}

fn read_compressed(r: &mut impl BufRead) -> io::Result<Cursor<Vec<u8>>> {
    let uncompressed_size = r.read_i32::<LittleEndian>()?;
    let compressed_size = r.read_i32::<LittleEndian>()?;
    if uncompressed_size < 0 || compressed_size < 0 || compressed_size >= uncompressed_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid compressed section size",
        ));
    }

    // TODO: Would be nice to return impl Read instead,
    // but not sure how to reconcile difference between
    // Take<R> and Take<ZlibDecoder<Take<R>>>
    // without resorting to dyn Read.
    if compressed_size == 0 {
        let mut uncompressed = vec![0; uncompressed_size as usize];
        r.read_exact(&mut uncompressed)?;
        Ok(Cursor::new(uncompressed))
    } else {
        let compressed = r.by_ref().take(compressed_size as u64);
        let mut decoder = ZlibDecoder::new(compressed);
        let mut uncompressed = vec![0; uncompressed_size as usize];
        decoder.read_exact(&mut uncompressed)?;
        Ok(Cursor::new(uncompressed))
    }
}

fn array<T, E, R: Read>(r: &mut R, mut f: impl FnMut(&mut R) -> Result<T, E>) -> Result<Vec<T>, E>
where
    E: From<io::Error>,
{
    let count = r.read_i32::<LittleEndian>()?;
    let mut vec = Vec::with_capacity(count as usize);
    for _ in 0..count {
        vec.push(f(r)?);
    }
    Ok(vec)
}

fn string(r: &mut impl Read) -> io::Result<String> {
    let size = r.read_i32::<LittleEndian>()?;
    let is_unicode = size < 0;

    let mut s = if is_unicode {
        // TODO: Is this UCS-2 or UTF-16?
        if size == i32::MIN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid length for Unicode string",
            ));
        }
        let size = -size;
        let mut data = vec![0u16; size as usize];
        r.read_u16_into::<LittleEndian>(&mut data)?;
        String::from_utf16(data.as_slice()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid Unicode string data")
        })?
    } else {
        // TODO: This is actually ASCII.
        let mut data = vec![0; size as usize];
        r.read_exact(&mut data)?;
        String::from_utf8(data)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid ASCII string data"))?
    };

    s.pop();
    Ok(s)
}

fn uuid(r: &mut impl Read) -> io::Result<Uuid> {
    let mut abcd = [0; 4];
    r.read_u32_into::<LittleEndian>(&mut abcd)?;
    let mut bytes = [0; 16];
    BigEndian::write_u32_into(&abcd, &mut bytes);
    Ok(Uuid::from_bytes(bytes))
}

/// Read a UE4 serialized date time from `r`.
fn date_time(r: &mut impl Read) -> io::Result<DateTime<Utc>> {
    let ticks = r.read_i64::<LittleEndian>()?;
    Ok(ue4_date_time_base()
        + Duration::microseconds(ticks / 10)
        + Duration::nanoseconds((ticks % 10) * 100))
}

fn read_user(r: &mut impl Read) -> io::Result<User> {
    Ok(User {
        id: uuid(r)?,
        name: string(r)?,
    })
}

fn read_user_name_first(r: &mut impl Read) -> io::Result<User> {
    Ok(User {
        name: string(r)?,
        id: uuid(r)?,
    })
}

fn read_brick_owner(r: &mut impl Read) -> io::Result<BrickOwner> {
    Ok(BrickOwner {
        user: read_user(r)?,
        brick_count: r.read_u32::<LittleEndian>()?,
    })
}

/// Splits a packed orientation into its corresponding direction and rotation.
fn split_orientation(orientation: u8) -> (Direction, Rotation) {
    let direction = ((orientation >> 2) % 6).try_into().unwrap();
    let rotation = (orientation & 0b11).try_into().unwrap();
    (direction, rotation)
}

fn skip_read(read: &mut impl Read, len: u64) -> io::Result<()> {
    std::io::copy(&mut read.take(len), &mut NullWrite).map(|_| ())
}

struct NullWrite;

impl Write for NullWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
