use std::env::args;
use std::fs::File;

fn main() -> std::io::Result<()> {
    let path = args().nth(1).expect("missing path");
    let reader = brs::Reader::new(File::open(&path)?)?;
    let reader = reader.read_header1()?;
    let reader = reader.read_header2()?;
    let data = reader.into_write_data()?;
    let mut new_path = path.clone();
    new_path.push_str(".rewrite.brs");
    brs::write_save(&mut File::create(new_path)?, &data)?;
    Ok(())
}
