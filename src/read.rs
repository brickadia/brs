use crate::{
    bit_reader::BitReader,
    save::{Brick, Color, ColorMode, Direction, Rotation, User},
    ue4_date_time_base, Version, MAGIC,
};
use byteorder::{BigEndian, ByteOrder, LittleEndian, ReadBytesExt};
use chrono::{prelude::*, Duration};
use libflate::zlib;
use std::{
    convert::TryInto,
    io::{self, prelude::*, Cursor},
};
use uuid::Uuid;

pub struct Reader<R: Read> {
    r: R,
    version: Version,
    game_version: u32,
}

impl<R: Read> Reader<R> {
    /// Create a new reader that reads from `r`.
    ///
    /// ```no_run
    /// # use std::fs::File;
    /// # use brs::Reader;
    /// let reader = Reader::new(File::open("village.brs")?)?;
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

        // TODO: Consider providing the first or last game version
        // that used this save version
        let game_version = 3642;

        Ok(Reader {
            r,
            version,
            game_version,
        })
    }

    /// Continue parsing to read the first header.
    /// See [`HasHeader1`](trait.HasHeader1.html) for what it makes available.
    ///
    /// ```no_run
    /// # let reader: brs::Reader<std::fs::File> = unimplemented!();
    /// let reader = reader.read_header1()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn read_header1(mut self) -> io::Result<ReaderAfterHeader1<R>> {
        let header1 = read_header1(&mut read_compressed(&mut self.r)?, self.version)?;

        Ok(ReaderAfterHeader1 {
            inner: self,
            header1,
        })
    }
}

pub struct ReaderAfterHeader1<R: Read> {
    inner: Reader<R>,
    header1: Header1,
}

impl<R: Read> ReaderAfterHeader1<R> {
    /// Continue parsing to read the second header.
    /// See [`HasHeader1`](trait.HasHeader2.html) for what it makes available.
    ///
    /// ```no_run
    /// # let reader: brs::read::ReaderAfterHeader1<std::fs::File> = unimplemented!();
    /// let reader = reader.read_header2()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn read_header2(mut self) -> io::Result<ReaderAfterHeader2<R>> {
        let header2 = read_header2(&mut read_compressed(&mut self.inner.r)?, self.inner.version)?;

        Ok(ReaderAfterHeader2 {
            inner: self,
            header2,
        })
    }
}

pub struct ReaderAfterHeader2<R: Read> {
    inner: ReaderAfterHeader1<R>,
    header2: Header2,
}

impl<R: Read> ReaderAfterHeader2<R> {
    /// Begin parsing the bricks and return an iterator over them.
    /// Consumes the reader.
    ///
    /// ```no_run
    /// # let reader: brs::read::ReaderAfterHeader2<std::fs::File> = unimplemented!();
    /// for brick in reader.iter_bricks()? {
    ///     let brick = brick?;
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn iter_bricks(mut self) -> io::Result<ReadBricks> {
        let rdr = &mut self.inner.inner;
        let mut brick_section = read_compressed(&mut rdr.r)?;
        let bricks_iter = read_bricks(
            &mut brick_section,
            rdr.version,
            &self.inner.header1,
            &self.header2,
        )?;
        Ok(bricks_iter)
    }

    /// Begin parsing the bricks and return an iterator over them,
    /// along with a finished reader that has header 1 and 2 data.
    ///
    /// ```no_run
    /// # let reader: brs::Reader<std::fs::File> = unimplemented!();
    /// # let reader = reader.read_header1()?;
    /// # let reader = reader.read_header2()?;
    /// let (rdr, bricks) = reader.iter_bricks_and_reader()?;
    ///
    /// for brick in bricks {
    ///     let brick = brick?;
    /// }
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn iter_bricks_and_reader(mut self) -> io::Result<(ReaderAfterBricks, ReadBricks)> {
        let rdr = &mut self.inner.inner;
        let mut brick_section = read_compressed(&mut rdr.r)?;
        let bricks_iter = read_bricks(
            &mut brick_section,
            rdr.version,
            &self.inner.header1,
            &self.header2,
        )?;
        let reader = ReaderAfterBricks {
            header1: self.inner.header1,
            header2: self.header2,
        };
        Ok((reader, bricks_iter))
    }

    /// Read the bricks and create a [`WriteData`](../struct.WriteData.html)
    /// for use with [`write_save`](../fn.write_save.html),
    /// which can be used to write a save file with identical content.
    ///
    /// ```no_run
    /// # use std::fs::File;
    /// # let reader: brs::read::ReaderAfterHeader2<File> = unimplemented!();
    /// let data = reader.into_write_data()?;
    /// brs::write_save(&mut File::create("park.brs")?, &data)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn into_write_data(self) -> io::Result<crate::WriteData> {
        let (reader, bricks_iter) = self.iter_bricks_and_reader()?;
        let bricks = bricks_iter.collect::<Result<_, _>>()?;

        Ok(crate::WriteData {
            map: reader.header1.map,
            author: reader.header1.author,
            description: reader.header1.description,
            save_time: reader.header1.save_time.unwrap_or_else(Utc::now),

            mods: reader.header2.mods,
            brick_assets: reader.header2.brick_assets,
            colors: reader.header2.colors,
            materials: reader.header2.materials,
            brick_owners: reader.header2.brick_owners,

            bricks,
        })
    }
}

pub struct ReaderAfterBricks {
    pub header1: Header1,
    pub header2: Header2,
}

#[derive(Debug, Clone)]
pub struct Header1 {
    pub map: String,
    pub author: User,
    pub description: String,
    pub host: Option<User>,
    pub save_time: Option<DateTime<Utc>>,
    pub brick_count: i32,
}

#[derive(Debug, Clone)]
pub struct Header2 {
    pub mods: Vec<String>,
    pub brick_assets: Vec<String>,
    pub colors: Vec<Color>,
    pub materials: Vec<String>,
    pub brick_owners: Vec<User>,
}

/// Exposes information available in the first header.
pub trait HasHeader1 {
    fn header1(&self) -> &Header1;

    fn map(&self) -> &str {
        &self.header1().map
    }

    fn author(&self) -> &User {
        &self.header1().author
    }

    fn description(&self) -> &str {
        &self.header1().description
    }

    fn save_time(&self) -> Option<&DateTime<Utc>> {
        self.header1().save_time.as_ref()
    }

    fn brick_count(&self) -> i32 {
        self.header1().brick_count
    }
}

/// Exposes information available in the second header.
pub trait HasHeader2 {
    fn header2(&self) -> &Header2;

    fn mods(&self) -> &[String] {
        &self.header2().mods[..]
    }

    fn brick_assets(&self) -> &[String] {
        &self.header2().brick_assets[..]
    }

    fn colors(&self) -> &[Color] {
        &self.header2().colors[..]
    }

    fn materials(&self) -> &[String] {
        &self.header2().materials[..]
    }

    fn brick_owners(&self) -> &[User] {
        &self.header2().brick_owners[..]
    }
}

impl<R: Read> HasHeader1 for ReaderAfterHeader1<R> {
    fn header1(&self) -> &Header1 {
        &self.header1
    }
}

impl<R: Read> HasHeader1 for ReaderAfterHeader2<R> {
    fn header1(&self) -> &Header1 {
        &self.inner.header1
    }
}

impl HasHeader1 for ReaderAfterBricks {
    fn header1(&self) -> &Header1 {
        &self.header1
    }
}

impl<R: Read> HasHeader2 for ReaderAfterHeader2<R> {
    fn header2(&self) -> &Header2 {
        &self.header2
    }
}

impl HasHeader2 for ReaderAfterBricks {
    fn header2(&self) -> &Header2 {
        &self.header2
    }
}

fn read_header1(r: &mut impl Read, version: Version) -> io::Result<Header1> {
    let map = string(r)?;
    let author_name = string(r)?;
    let description = string(r)?;
    let author_id = uuid(r)?;

    let host = None;

    let save_time = if version >= Version::AddedDateTime {
        Some(date_time(r)?)
    } else {
        None
    };

    let brick_count = r.read_i32::<LittleEndian>()?;

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

    let brick_owners = if version >= Version::AddedOwnerData {
        array(r, read_user)?
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
    r: BitReader,
    brick_asset_num: u32,
    color_num: u32,
    brick_count: i32,
    index: i32,
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
        let material_index = if self.r.read_bit() {
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

fn read_compressed(r: &mut impl Read) -> io::Result<impl Read> {
    let uncompressed_size = r.read_i32::<LittleEndian>()?;
    let compressed_size = r.read_i32::<LittleEndian>()?;
    if uncompressed_size < 0 || compressed_size < 0 || compressed_size >= uncompressed_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid compressed section size",
        ));
    }

    // TODO: Don't read the entire thing into memory, somehow stream decode it
    if compressed_size == 0 {
        let mut uncompressed = vec![0; uncompressed_size as usize];
        r.read_exact(&mut uncompressed)?;
        Ok(Cursor::new(uncompressed))
    } else {
        let mut compressed = vec![0; compressed_size as usize];
        r.read_exact(&mut compressed)?;
        let mut decoder = zlib::Decoder::new(&compressed[..])?;
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
    let (size, is_ucs2) = match r.read_i32::<LittleEndian>()? {
        s if s >= 0 => (s, false),
        s => (-s, true),
    };

    let mut s = if is_ucs2 {
        // TODO: Verify that UTF-16 is backwards compatible with UCS-2.
        if size % 2 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid UCS-2 data size",
            ));
        }
        let mut data = vec![0; size as usize / 2];
        r.read_u16_into::<LittleEndian>(&mut data)?;
        String::from_utf16(data.as_slice())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UCS-2 data"))?
    } else {
        // TODO: Figure out the correct encoding.
        // 7-bit values should just be ASCII, so that part is fine,
        // but I don't know what 80h-FFh should be.
        // Hope that UTF-8 will error for now.
        let mut data = vec![0; size as usize];
        r.read_exact(&mut data)?;
        String::from_utf8(data)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid string data"))?
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

/// Splits a packed orientation into its corresponding direction and rotation.
fn split_orientation(orientation: u8) -> (Direction, Rotation) {
    let direction = ((orientation >> 2) % 6).try_into().unwrap();
    let rotation = (orientation & 0b11).try_into().unwrap();
    (direction, rotation)
}
