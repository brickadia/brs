use brs::{HasHeader1, HasHeader2};
use std::env::args;
use std::fs::File;

fn main() -> std::io::Result<()> {
    let path = args().nth(1).expect("missing path");
    let reader = brs::Reader::new(File::open(path)?)?;
    let reader = reader.read_header1()?;
    let reader = reader.read_header2()?;
    dbg!(reader.header1());
    dbg!(reader.header2());
    let mut first_brick = None;
    for brick in reader.iter_bricks()? {
        let brick = brick?;
        first_brick.get_or_insert(brick);
    }
    dbg!(first_brick);
    Ok(())
}
