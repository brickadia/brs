//! Interfaces for reading and writing Brickadia save files.
//!
//! Aims to be able to read all previous versions just like the game,
//! but only write the newest version of the format.
//!
//! # Reading
//!
//! A [`Reader`](crate::Reader) can be created from any
//! [`io::Read`](std::io::Read) source, such as a file or buffer.
//!
//! ```no_run
//! # use std::{fs::File, io::BufReader};
//! let reader = brs::Reader::new(BufReader::new(File::open("village.brs")?))?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! This will read the initial headers, metadata, etc., and make them available with accessors:
//!
//! ```no_run
//! # let reader: brs::Reader<std::io::Empty, brs::read::Init> = unimplemented!();
//! println!("Brick count: {}", reader.brick_count());
//! println!("Map: {}", reader.map());
//! println!("Description: {}", reader.description());
//! println!("Brick owners: {:?}", reader.brick_owners());
//! println!("Color count: {}", reader.colors().len());
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Newer saves contain an embedded screenshot you can request before reading the bricks.
//! This step can be skipped.
//!
//! ```no_run
//! # let reader: brs::Reader<std::io::Empty, brs::read::Init> = unimplemented!();
//! let (reader, screenshot) = reader.screenshot()?;
//! if let Some(screenshot) = screenshot {
//!     dbg!(screenshot.format);
//! }
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Bricks can then be iterated over. See [`Brick`](crate::Brick).
//!
//! ```no_run
//! # let reader: brs::Reader<std::io::Empty, brs::read::AfterScreenshot> = unimplemented!();
//! let (reader, bricks) = reader.bricks()?;
//! for brick in bricks {
//!     let brick = brick?;
//!     println!("{} {:?}", reader.brick_count(), brick);
//! }
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! # Writing
//!
//! Writing save files isn't as fancy, for now you simply just put all the data
//! in the [`WriteData`](crate::WriteData) struct and pass it to
//! [`write_save`](crate::write_save) along with a
//! [`Write`](std::io::Write) destination.
//!
//! ```no_run
//! # use std::fs::File;
//! let data = brs::WriteData {
//!     map: "Plate".to_string(),
//!     author: brs::User {
//!         id: brs::uuid::Uuid::nil(),
//!         name: "Jensen".to_string(),
//!     },
//!     description: "A quaint park full of ducks and turkeys.".to_string(),
//!     save_time: chrono::Utc::now(),
//!
//!     brick_assets: vec!["PB_DefaultBrick".to_string()],
//!     colors: vec![brs::Color::from_rgba(255, 23, 198, 255)],
//!     materials: vec!["BMC_Plastic".to_string()],
//!     brick_owners: Vec::new(),
//!
//!     bricks: Vec::new(),
//!
//!     ..Default::default()
//! };
//! brs::write_save(&mut File::create("park.brs")?, &data)?;
//! # Ok::<(), std::io::Error>(())
//! ```

mod bit_reader;
mod bit_writer;
mod save;

//pub mod read;
pub mod read_new;
pub use read_new as read;
pub mod write;

pub use read_new::Reader;
pub use save::*;
pub use write::{write_save, WriteData};

pub use chrono;
pub use uuid;

use chrono::prelude::*;

const MAGIC: [u8; 3] = [b'B', b'R', b'S'];

fn ue4_date_time_base() -> DateTime<Utc> {
    Utc.ymd(1, 1, 1).and_hms(0, 0, 0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bit_reader_writer_equal_order() {
        use crate::{bit_reader::BitReader, bit_writer::BitWriter};
        let mut writer = BitWriter::new(Vec::new());
        writer.write_bit(true).unwrap();
        writer.write_bit(false).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(false).unwrap();
        writer.byte_align().unwrap();
        writer.write_bit(false).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(true).unwrap();
        writer.write_bit(true).unwrap();
        writer.byte_align().unwrap();
        let packed = 398475;
        writer.write_int_packed(packed).unwrap();
        let written = writer.finish().unwrap();
        for (i, v) in written.iter().enumerate() {
            println!("written[{}]: {:b}", i, v);
        }
        let mut reader = BitReader::new(&written);
        assert_eq!(dbg!(reader.read_bit()), true);
        assert_eq!(dbg!(reader.read_bit()), false);
        assert_eq!(dbg!(reader.read_bit()), true);
        assert_eq!(dbg!(reader.read_bit()), true);
        assert_eq!(dbg!(reader.read_bit()), false);
        reader.eat_byte_align();
        assert_eq!(dbg!(reader.read_bit()), false);
        assert_eq!(dbg!(reader.read_bit()), true);
        assert_eq!(dbg!(reader.read_bit()), true);
        assert_eq!(dbg!(reader.read_bit()), true);
        reader.eat_byte_align();
        assert_eq!(dbg!(reader.read_int_packed()), packed);
    }
}
