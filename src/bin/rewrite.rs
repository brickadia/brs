use std::env::args;
use std::fs::File;
use std::io::BufReader;

fn main() -> std::io::Result<()> {
    let path = args().nth(1).expect("missing path");
    let reader = brs::Reader::new(BufReader::new(File::open(&path)?))?;
    let mut data = reader.into_write_data()?;
    data.version = brs::write::LATEST_VERSION;
    let mut new_path = path;
    new_path.push_str(".rewrite.brs");
    brs::write_save(&mut File::create(new_path)?, &data)?;
    Ok(())
}
