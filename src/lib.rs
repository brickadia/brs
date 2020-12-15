//! Interfaces for reading and writing Brickadia save files.
//!
//! Aims to be able to read all previous versions just like the game,
//! but only write the newest version of the format.
//!
//! # Reading
//!
//! A [`Reader`](struct.Reader.html) can be created from any
//! [`io::Read`](https://doc.rust-lang.org/std/io/trait.Read.html)
//! source, such as a file or buffer.
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
//! let image_format = reader.screenshot_format();
//! let (reader, image_bytes) = reader.screenshot_data()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Bricks can then be iterated over.
//! See [`Brick`](struct.Brick.html).
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
//! in the [`WriteData`](struct.WriteData.html) struct and pass it to
//! [`write_save`](fn.write_save.html) along with a
//! [`Write`](https://doc.rust-lang.org/std/io/trait.Write.html) destination.
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
//!     mods: Vec::new(),
//!     brick_assets: vec!["PB_DefaultBrick".to_string()],
//!     colors: vec![brs::Color::from_rgba(255, 23, 198, 255)],
//!     materials: vec!["BMC_Plastic".to_string()],
//!     brick_owners: Vec::new(),
//!
//!     bricks: Vec::new(),
//! };
//! brs::write_save(&mut File::create("park.brs")?, &data)?;
//! # Ok::<(), std::io::Error>(())
//! ```

mod bit_reader;
mod bit_writer;
mod save;

pub mod read;
mod write;

pub use read::Reader;
pub use save::*;
pub use write::{write_save, WriteData};

pub use chrono;
pub use uuid;

use chrono::prelude::*;

const MAGIC: [u8; 3] = [b'B', b'R', b'S'];

fn ue4_date_time_base() -> DateTime<Utc> {
    Utc.ymd(1, 1, 1).and_hms(0, 0, 0)
}
