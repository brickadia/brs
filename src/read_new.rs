use std::convert::TryInto;
use std::io::{self, BufRead, Cursor, Read};
use crate::Brick;
use crate::save::SCREENSHOT_NONE;
use crate::ScreenshotFormat;
use crate::ue4_date_time_base;
use crate::{BrickOwner, Color, User, Version};
use crate::{MAGIC, MAX_VERSION_READ, MIN_VERSION_READ};
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use thiserror::Error;
use byteorder::{ReadBytesExt, ByteOrder, LittleEndian, BigEndian};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid magic (may not be a brs file)")]
    InvalidMagic,
    #[error("version too old (unsupported)")]
    VersionTooOld(u16),
    #[error("version too new (unsupported)")]
    VersionTooNew(u16),
    #[error("unknown version (unsupported)")]
    VersionUnknown {
        source: num_enum::TryFromPrimitiveError<Version>,
    },
    #[error("invalid compressed section")]
    InvalidCompressedSection,
    #[error("unknown screenshot format")]
    UnknownScreenshotFormat {
        source: num_enum::TryFromPrimitiveError<ScreenshotFormat>,
    },
    #[error("i/o error")]
    Io {
        #[from] source: io::Error,
        //backtrace: Backtrace,
    },
}

mod states {
    pub trait Sealed {}
    pub trait State: Sealed {}

    pub struct Init {}
    impl Sealed for Init {}
    impl State for Init {}

    pub struct PostScreenshot {}
    impl Sealed for PostScreenshot {}
    impl State for PostScreenshot {}

    pub struct PostBricks {}
    impl Sealed for PostBricks {}
    impl State for PostBricks {}

    pub struct PostComponents {}
    impl Sealed for PostComponents {}
    impl State for PostComponents {}
}

pub use states::{State, Init, PostScreenshot, PostBricks, PostComponents};

pub struct Reader<R: BufRead, S: State> {
    reader: R,
    file_version: Version,
    game_version: u32,
    header1: Header1,
    header2: Header2,
    screenshot_header: Option<(ScreenshotFormat, usize)>,
    #[allow(dead_code)]
    state: S,
}

impl<R: BufRead, S: State> Reader<R, S> {
    fn with_state<W: State>(self, state: W) -> Reader<R, W> {
        Reader {
            reader: self.reader,
            file_version: self.file_version,
            game_version: self.game_version,
            header1: self.header1,
            header2: self.header2,
            screenshot_header: self.screenshot_header,
            state,
        }
    }

    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R: BufRead> Reader<R, Init> {
    pub fn new(mut reader: R) -> Result<Self, Error> {
        // ReadAndCheckVersion
        let mut magic = [0; MAGIC.len()];
        reader.read_exact(&mut magic)
            .map_err(|e| if e.kind() == io::ErrorKind::UnexpectedEof {
                Error::InvalidMagic
            } else {
                e.into()
            })?;
        if magic != MAGIC {
            return Err(Error::InvalidMagic);
        }

        let version_raw = reader.read_u16::<LittleEndian>()?;

        if version_raw < MIN_VERSION_READ.into() {
            return Err(Error::VersionTooOld(version_raw));
        }

        if version_raw > MAX_VERSION_READ.into() {
            return Err(Error::VersionTooNew(version_raw));
        }

        let file_version: Version = version_raw.try_into()
            .map_err(|source| Error::VersionUnknown { source })?;

        let game_version = if file_version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
            reader.read_u32::<LittleEndian>()?
        } else {
            file_version.first_game_version()
        };

        let header1 = read_header1(&mut reader, file_version)?;
        let header2 = read_header2(&mut reader, file_version)?;

        let screenshot_header = if let Some(format) = read_image_format(&mut reader)? {
            let len = reader.read_i32::<LittleEndian>()?
                .try_into().unwrap_or(0);
            Some((format, len))
        } else {
            None
        };

        Ok(Self {
            reader,
            file_version,
            game_version,
            header1,
            header2,
            screenshot_header,
            state: Init {},
        })
    }

    fn skip_screenshot(mut self) -> Result<Reader<R, PostScreenshot>, Error> {
        if let Some((_, len)) = self.screenshot_header {
            skip(&mut self.reader, len.try_into().unwrap_or(0))?;
        }
        Ok(self.with_state(PostScreenshot {}))
    }

    pub fn screenshot_data(self) -> ScreenshotReader<R> {
        ScreenshotReader {
            remaining: self.screenshot_header
                .map(|(_, len)| len)
                .unwrap_or(0),
            reader: self,
        }
    }

    pub fn bricks(self) -> Result<(BrickReader, Reader<R, PostBricks>), Error> {
        self.skip_screenshot()?.bricks()
    }
}

pub struct ScreenshotReader<R: BufRead> {
    reader: Reader<R, Init>,
    remaining: usize,
}

impl<R: BufRead> ScreenshotReader<R> {
    pub fn done(mut self) -> io::Result<Reader<R, PostScreenshot>> {
        skip(&mut self.reader.reader, self.remaining)?;
        Ok(self.reader.with_state(PostScreenshot {}))
    }
}

impl<R: BufRead> Read for ScreenshotReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let nread = buf.len().max(self.remaining);
        let nread = self.reader.reader.read(&mut buf[..nread])?;
        self.remaining = self.remaining.saturating_sub(nread);
        Ok(nread)
    }
}

impl<R: BufRead> Reader<R, PostScreenshot> {
    fn skip_bricks(mut self) -> Result<Reader<R, PostBricks>, Error> {
        skip_compressed(&mut self.reader)?;
        Ok(self.with_state(PostBricks {}))
    }

    pub fn bricks(mut self) -> Result<(BrickReader, Reader<R, PostBricks>), Error> {
        // SaveFileReader.ReadBrickList
        let bricks = read_bricks(&mut self.reader, &self.header1, &self.header2)?;
        Ok((bricks, self.with_state(PostBricks {})))
    }

    pub fn components(self) -> Result<((), Reader<R, PostComponents>), Error> {
        self.skip_bricks()?.components()
    }
}

impl<R: BufRead> Reader<R, PostBricks> {
    pub fn components(mut self) -> Result<((), Reader<R, PostComponents>), Error> {
        let components = todo!();
        Ok((components, self.with_state(PostComponents {})))
    }
}

impl<R: BufRead, S: State> Reader<R, S> {
    pub fn map(&self) -> &str {
        &self.header1.map
    }
    pub fn author(&self) -> &User {
        &self.header1.author
    }
    pub fn description(&self) -> &str {
        &self.header1.description
    }
    pub fn host(&self) -> Option<&User> {
        self.header1.host.as_ref()
    }
    pub fn time_saved(&self) -> Option<DateTime<Utc>> {
        self.header1.time_saved
    }
    pub fn brick_count(&self) -> u32 {
        self.header1.brick_count
    }

    pub fn mods(&self) -> &[String] {
        &self.header2.mods
    }
    pub fn brick_assets(&self) -> &[String] {
        &self.header2.brick_assets
    }
    pub fn colors(&self) -> &[Color] {
        &self.header2.colors
    }
    pub fn materials(&self) -> &[String] {
        &self.header2.materials
    }
    pub fn brick_owners(&self) -> &[BrickOwner] {
        &self.header2.brick_owners
    }

    pub fn screenshot_info(&self) -> Option<(ScreenshotFormat, usize)> {
        self.screenshot_header
    }
}

struct Header1 {
    map: String,
    author: User,
    description: String,
    host: Option<User>,
    time_saved: Option<DateTime<Utc>>,
    brick_count: u32,
}

struct Header2 {
    mods: Vec<String>,
    brick_assets: Vec<String>,
    colors: Vec<Color>,
    materials: Vec<String>,
    brick_owners: Vec<BrickOwner>,
}

fn read_header1(reader: &mut impl BufRead, file_version: Version) -> Result<Header1, Error> {
    let mut reader = compressed_section(reader)?;
    let map = read_string(&mut reader)?;
    let author_name = read_string(&mut reader)?;
    let description = read_string(&mut reader)?;
    let author_id = read_uuid(&mut reader)?;
    let host = if file_version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
        let name = read_string(&mut reader)?;
        let id = read_uuid(&mut reader)?;
        Some(User { id, name })
    } else {
        None
    };
    let time_saved = if file_version >= Version::AddedDateTime {
        Some(read_datetime(&mut reader)?)
    } else {
        None
    };
    let brick_count = reader.read_i32::<LittleEndian>()?;
    let brick_count = brick_count.try_into().map_err(|source| todo!())?;
    Ok(Header1 {
        map,
        author: User {
            id: author_id,
            name: author_name,
        },
        description,
        host,
        time_saved,
        brick_count,
    })
}

fn read_header2(reader: &mut impl BufRead, file_version: Version) -> Result<Header2, Error> {
    // SaveFileReader.SerializeHeader2
    let mut reader = compressed_section(reader)?;
    let mods = read_array(&mut reader, read_string)?;
    let brick_assets = read_array(&mut reader, read_string)?;
    let colors = read_array(&mut reader, |r| r.read_u32::<LittleEndian>().map(Into::into))?;
    let materials = if file_version >= Version::MaterialsStoredAsNames {
        read_array(&mut reader, read_string)?
    } else {
        vec![
            "BMC_Hologram".to_string(),
            "BMC_Plastic".to_string(),
            "BMC_Glow".to_string(),
            "BMC_Metallic".to_string(),
        ]
    };
    let brick_owners: Vec<BrickOwner> = if file_version >= Version::AddedOwnerData {
        if file_version >= Version::AddedGameVersionAndHostAndOwnerDataAndImprovedMaterials {
            fn read_with_brick_count(reader: &mut impl Read) -> Result<BrickOwner, Error> {
                let user_id = read_uuid(reader)?;
                let user_name = read_string(reader)?;
                let brick_count = reader.read_u32::<LittleEndian>()?;
                Ok(BrickOwner {
                    user: User { id: user_id, name: user_name },
                    brick_count,
                })
            }
            read_array(&mut reader, read_with_brick_count)?
        } else {
            fn read_without_brick_count(reader: &mut impl Read) -> Result<BrickOwner, Error> {
                let user_id = read_uuid(reader)?;
                let user_name = read_string(reader)?;
                Ok(BrickOwner {
                    user: User { id: user_id, name: user_name },
                    brick_count: 0,
                })
            }
            read_array(&mut reader, read_without_brick_count)?
        }
    } else {
        todo!()
    };
    Ok(Header2 {
        mods,
        brick_assets,
        colors,
        materials,
        brick_owners,
    })
}

fn read_bricks(reader: &mut impl BufRead, header1: &Header1, header2: &Header2) -> Result<BrickReader, Error> {
    let compressed = compressed_section(reader)?;
    let data = compressed.into_inner();
    Ok(BrickReader {
        data,
        remaining: header1.brick_count,
    })
}

pub struct BrickReader {
    data: Vec<u8>,
    remaining: u32,
}

impl Iterator for BrickReader {
    type Item = Result<Brick, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

fn compressed_section(reader: &mut impl BufRead) -> Result<Cursor<Vec<u8>>, Error> {
    let len = reader.read_i32::<LittleEndian>()?;
    let compressed_len = reader.read_i32::<LittleEndian>()?;
    if !(1..i32::MAX-1).contains(&len) ||
        !(0..len).contains(&compressed_len)
    {
        return Err(Error::InvalidCompressedSection);
    }

    let len = len.try_into().unwrap();
    let compressed_len: u64 = compressed_len.try_into().unwrap();

    if compressed_len == 0 {
        // Stored uncompressed
        let mut data = vec![0; len];
        reader.read_exact(&mut data)?;
        Ok(Cursor::new(data))
    } else {
        // Stored zlib compressed
        let compressed = reader.by_ref().take(compressed_len);
        let mut decoder = flate2::bufread::ZlibDecoder::new(compressed);
        let mut uncompressed = vec![0; len];
        decoder.read_exact(&mut uncompressed)?;
        Ok(Cursor::new(uncompressed))
    }
}

fn skip_compressed(reader: &mut impl Read) -> Result<(), Error> {
    let len = reader.read_i32::<LittleEndian>()?;
    let compressed_len = reader.read_i32::<LittleEndian>()?;
    if !(1..i32::MAX-1).contains(&len) ||
        !(0..len).contains(&compressed_len)
    {
        return Err(Error::InvalidCompressedSection);
    }

    let len = len.try_into().unwrap();
    let compressed_len = compressed_len.try_into().unwrap();

    let to_skip = if compressed_len == 0 {
        len
    } else {
        compressed_len
    };

    skip(reader, to_skip)?;
    Ok(())
}

fn read_array<T, E, R: Read, F>(reader: &mut R, mut each: F) -> Result<Vec<T>, E>
where
    F: FnMut(&mut R) -> Result<T, E>,
    E: From<io::Error>,
{
    let len = reader.read_i32::<LittleEndian>()?;
    let mut vec = Vec::with_capacity(len as usize);
    for _ in 0..len {
        vec.push(each(reader)?);
    }
    Ok(vec)
}

fn read_string(reader: &mut impl Read) -> Result<String, Error> {
    todo!()
}

fn read_uuid(reader: &mut impl Read) -> Result<Uuid, io::Error> {
    let mut words = [0; 4];
    reader.read_u32_into::<LittleEndian>(&mut words)?;
    let mut bytes = [0; 16];
    BigEndian::write_u32_into(&words, &mut bytes);
    Ok(Uuid::from_bytes(bytes))
}

fn read_datetime(reader: &mut impl Read) -> Result<DateTime<Utc>, Error> {
    let ticks = reader.read_i64::<LittleEndian>()?;
    Ok(ue4_date_time_base()
        + Duration::microseconds(ticks / 10)
        + Duration::nanoseconds((ticks % 10) * 100))
}

fn read_image_format(reader: &mut impl Read) -> Result<Option<ScreenshotFormat>, Error> {
    let byte = reader.read_u8()?;
    if byte == SCREENSHOT_NONE {
        Ok(None)
    } else {
        byte.try_into()
            .map_err(|source| Error::UnknownScreenshotFormat { source })
            .map(Some)
    }
}

fn skip(reader: &mut impl Read, mut len: usize) -> io::Result<()> {
    const MAX_READ: usize = 32_768;
    let mut chunk = vec![0; MAX_READ];
    while len != 0 {
        let read_len = chunk.len().min(len.into());
        reader.read_exact(&mut chunk[..read_len])?;
        len -= read_len;
    }
    Ok(())
}
