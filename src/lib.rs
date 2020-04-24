//! Interfaces for reading and writing Brickadia save files.
//!
//! Aims to be able to read all previous versions just like the game,
//! but only write the newest version of the format.
//!
//! # Usage
//!
//! ## Reading
//!
//! First, create a reader from any
//! [`Read`](https://doc.rust-lang.org/std/io/trait.Read.html)
//! source, such as a file or buffer.
//!
//! ```no_run
//! # use std::fs::File;
//! let reader = brs::Reader::new(File::open("village.brs")?)?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Brickadia save files have information split into sections ordered
//! such that one can extract simple information
//! without needing to parse the entire file.
//!
//! This library surfaces this by strictly enforcing the way that data is read
//! and made available at the type level; you can't go wrong.
//!
//! To continue, reading the first header gets you basic information.
//! For details on what is available, see
//! [`HasHeader1`](read/trait.HasHeader1.html).
//!
//! ```no_run
//! use brs::HasHeader1;
//! # let reader: brs::Reader<std::fs::File> = unimplemented!();
//! let reader = reader.read_header1()?;
//! println!("Brick count: {}", reader.brick_count());
//! println!("Map: {}", reader.map());
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! The next header contains data less likely to be relevant for simpler
//! introspection, but rather things such as tables for loading bricks.
//! See [`HasHeader2`](read/trait.HasHeader2.html).
//!
//! ```no_run
//! use brs::HasHeader2;
//! # let reader: brs::read::ReaderAfterHeader1<std::fs::File> = unimplemented!();
//! let reader = reader.read_header2()?;
//! println!("Mods: {:?}", reader.mods());
//! println!("Color count: {}", reader.colors().len());
//! // Properties from header 1 are still available:
//! use brs::HasHeader1;
//! println!("Description: {}", reader.description());
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! After both headers have been read, you may now iterate over the bricks.
//! See [`Brick`](struct.Brick.html).
//!
//! ```no_run
//! # let reader: brs::read::ReaderAfterHeader2<std::fs::File> = unimplemented!();
//! for brick in reader.iter_bricks()? {
//!     let brick = brick?;
//!     println!("{:?}", brick);
//! }
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! You may retain access to the header information while getting the iterator:
//!
//! ```no_run
//! # let reader: brs::read::ReaderAfterHeader2<std::fs::File> = unimplemented!();
//! let (reader, bricks) = reader.iter_bricks_and_reader()?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ## Writing
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
//!     author: todo!(),
//!     description: "A quaint park full of ducks and turkeys.".to_string(),
//!     save_time: chrono::Utc::now(),
//!     
//!     mods: Vec::new(),
//!     brick_assets: vec!["PB_DefaultBrick".to_string()],
//!     colors: vec![todo!()],
//!     materials: vec![todo!()],
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

pub use read::{HasHeader1, HasHeader2, Reader};
pub use save::*;
pub use write::{write_save, WriteData};

pub use chrono;
pub use uuid;

use chrono::prelude::*;

const MAGIC: [u8; 3] = [b'B', b'R', b'S'];

fn ue4_date_time_base() -> DateTime<Utc> {
    Utc.ymd(1, 1, 1).and_hms(0, 0, 0)
}
