use crate::{
    bit_writer::BitWriter,
    save::{Brick, Color, ColorMode, Direction, Rotation, User},
    ue4_date_time_base, MAGIC,
};
use byteorder::{BigEndian, ByteOrder, LittleEndian, WriteBytesExt};
use chrono::prelude::*;
use libflate::zlib;
use std::{
    convert::TryFrom,
    io::{self, prelude::*},
};
use uuid::Uuid;

const LATEST_VERSION: u16 = 4;

/// Data written to save files by [`write_save`](fn.write_save.html).
pub struct WriteData {
    // Header 1
    /// The name of the map that the save file was created on.
    pub map: String,
    /// The user that created the save.
    pub author: User,
    /// A short description of the save file.
    pub description: String,
    /// When the save file was created.
    pub save_time: DateTime<Utc>,
    // pub brick_count: i32,

    // Header 2
    /// The mods used by the save file. Format not yet defined.
    pub mods: Vec<String>,
    /// The name lookup table used by bricks. Example values include
    /// `"PB_DefaultBrick"`, `"PB_DefaultTile"`, `"B_1x_Octo_T"`, etc.
    pub brick_assets: Vec<String>,
    /// The color lookup table used by bricks.
    pub colors: Vec<Color>,
    /// The material lookup table used by bricks. Common values include:
    /// * `"BMC_Plastic"`
    /// * `"BMC_Glow"`
    /// * `"BMC_Metallic"`
    /// * `"BMC_Hologram"`
    pub materials: Vec<String>,
    /// The brick owner lookup table used by bricks.
    pub brick_owners: Vec<User>,

    // Bricks
    /// All the bricks in the save file.
    pub bricks: Vec<Brick>,
}

/// Write a save file consisting of `data` to `w`.
pub fn write_save(w: &mut impl Write, data: &WriteData) -> io::Result<()> {
    if data.bricks.len() > i32::max_value() as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Brick count out of range",
        ));
    }

    w.write_all(&MAGIC)?;
    w.write_u16::<LittleEndian>(LATEST_VERSION)?;

    let mut s = Compressed::new();
    write_string(&mut s, &data.map)?;
    write_string(&mut s, &data.author.name)?;
    write_string(&mut s, &data.description)?;
    write_uuid(&mut s, &data.author.id)?;
    write_date_time(&mut s, data.save_time)?;
    s.write_i32::<LittleEndian>(data.bricks.len() as i32)?;
    s.finish(w)?;

    let mut s = Compressed::new();
    write_array(&mut s, |w, s| write_string(w, s), &data.mods)?;
    write_array(&mut s, |w, s| write_string(w, s), &data.brick_assets)?;
    write_array(
        &mut s,
        |w, c| w.write_u32::<LittleEndian>((*c).into()),
        &data.colors,
    )?;
    write_array(&mut s, |w, s| write_string(w, s), &data.materials)?;
    write_array(
        &mut s,
        |w, o| {
            write_uuid(w, &o.id)?;
            write_string(w, &o.name)
        },
        &data.brick_owners,
    )?;
    s.finish(w)?;

    assert!(data.brick_assets.len() <= u32::max_value() as usize);
    assert!(data.colors.len() <= u32::max_value() as usize);

    let mut s = BitWriter::new(Compressed::new());
    for brick in &data.bricks {
        s.byte_align()?;
        s.write_int(
            brick.asset_name_index,
            data.brick_assets.len().max(2) as u32,
        )?;
        if s.write_bit(brick.size != (0, 0, 0))? {
            s.write_positive_int_vector_packed(brick.size)?;
        }
        s.write_int_vector_packed(brick.position)?;
        let orientation = combine_orientation(brick.direction, brick.rotation);
        s.write_int(u32::from(orientation), 24)?;
        s.write_bit(brick.collision)?;
        s.write_bit(brick.visibility)?;
        if s.write_bit(brick.material_index != 1)? {
            s.write_int_packed(brick.material_index)?;
        }
        match brick.color {
            ColorMode::Set(i) => {
                s.write_bit(false)?;
                s.write_int(i, data.colors.len() as u32)?;
            }
            ColorMode::Custom(c) => {
                s.write_bit(true)?;
                s.write_u32::<LittleEndian>(c.into())?;
            }
        }

        s.write_int_packed(match brick.owner_index {
            None => 0,
            Some(i) => i + 1,
        })?;
    }
    s.finish()?.finish(w)?;

    Ok(())
}

struct Compressed {
    encoder: zlib::Encoder<Vec<u8>>,
    uncompressed: Vec<u8>,
}

impl Compressed {
    fn new() -> Self {
        let encoder = zlib::Encoder::new(vec![]).unwrap();
        Self {
            encoder,
            uncompressed: vec![],
        }
    }

    fn finish(self, w: &mut impl Write) -> io::Result<()> {
        let compressed = self.encoder.finish().into_result()?;

        let uncompressed_size = self.uncompressed.len();
        let compressed_size = compressed.len();

        if uncompressed_size >= i32::max_value() as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "uncompressed_size out of range",
            ));
        }

        if compressed_size >= i32::max_value() as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "compressed_size out of range",
            ));
        }

        let uncompressed_size = uncompressed_size as i32;
        let compressed_size = compressed_size as i32;

        w.write_i32::<LittleEndian>(uncompressed_size)?;

        if compressed_size >= uncompressed_size {
            w.write_i32::<LittleEndian>(0)?;
            w.write_all(&self.uncompressed)
        } else {
            w.write_i32::<LittleEndian>(compressed_size)?;
            w.write_all(&compressed)
        }
    }
}

impl Write for Compressed {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        let written = self.encoder.write(src)?;
        self.uncompressed.extend(&src[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        unimplemented!()
    }
}

fn write_array<T, W: Write>(
    w: &mut W,
    write: impl Fn(&mut W, &T) -> io::Result<()>,
    values: &[T],
) -> io::Result<()> {
    if values.len() > i32::max_value() as usize {
        return Err(io::Error::from(io::ErrorKind::Other));
    }

    w.write_i32::<LittleEndian>(values.len() as i32)?;

    for value in values {
        write(w, value)?;
    }

    Ok(())
}

fn is_ucs2(number: impl Into<u32>) -> bool {
    let number = number.into();
    number <= 0xd7ff || number >= 0xe000
}

fn write_string(w: &mut impl Write, s: impl AsRef<str>) -> io::Result<()> {
    let s = s.as_ref();

    if s.is_ascii() {
        let len = s.len() + 1;
        assert!(len <= i32::max_value() as usize);
        w.write_i32::<LittleEndian>(len as i32)?;
        w.write_all(s.as_bytes())?;
        w.write_u8(0)?;
    } else {
        let len = -(((s.len() + 1) * 2) as isize);
        assert!(len >= i32::min_value() as isize);
        w.write_i32::<LittleEndian>(len as i32)?;

        for character in s.chars() {
            if is_ucs2(character) {
                w.write_u16::<LittleEndian>(character as u16)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "String contains non-UCS2 characters",
                ));
            }
        }

        w.write_u16::<LittleEndian>(0)?;
    }

    Ok(())
}

fn write_uuid(w: &mut impl Write, uuid: &Uuid) -> io::Result<()> {
    let mut abcd = [0; 4];
    BigEndian::read_u32_into(uuid.as_bytes(), &mut abcd);
    for element in abcd.iter() {
        w.write_u32::<LittleEndian>(*element)?;
    }
    Ok(())
}

fn write_date_time(w: &mut impl Write, date_time: DateTime<Utc>) -> io::Result<()> {
    let duration = date_time - ue4_date_time_base();
    let duration = duration
        .to_std()
        .unwrap_or(std::time::Duration::from_secs(0));
    let ticks_secs = i64::try_from(duration.as_secs() * 10_000_000).unwrap();
    let ticks_nanos = i64::from(duration.subsec_nanos() / 100);
    w.write_i64::<LittleEndian>(ticks_secs + ticks_nanos)
}

/// Combines a direction and rotation into their corresponding packed orientation.
fn combine_orientation(direction: Direction, rotation: Rotation) -> u8 {
    (u8::from(direction) << 2) | u8::from(rotation)
}
