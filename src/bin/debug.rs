fn main() -> std::io::Result<()> {
    use std::io::BufReader;
    let path = std::env::args().nth(1).expect("missing path");
    let reader = brs::Reader::new(BufReader::new(std::fs::File::open(path)?))?;
    // Optional: let (reader, screenshot) = reader.screenshot_data()?;
    let (reader, bricks) = reader.bricks()?;
    println!("{:?}", reader);
    let mut last_brick = None;
    for (index, brick) in bricks.iter().enumerate() {
        let brick = brick?;
        continue;
        let asset = reader.brick_assets().get(brick.asset_name_index as usize);
        if asset.is_none() {
            dbg!(index);
            dbg!(&brick);
            panic!("invalid asset name index");
        }
        let asset = asset.unwrap();
        if brick.size == (0, 0, 0) {
            if asset.starts_with("PB_") {
                dbg!(index);
                dbg!(&brick);
                panic!("invalid size for {}", asset);
            }
        } else {
            if asset.starts_with("B_") {
                dbg!(index);
                dbg!(&brick);
                panic!("invalid size for {}", asset);
            }
        }
        if let Some(index) = brick.owner_index {
            if !((index as usize) < reader.brick_owners().len()) {
                dbg!(index);
                dbg!(&brick);
                panic!("invalid owner index");
            }
        }
        last_brick = Some(brick);
        break;
    }
    println!("{:?}", last_brick);
    let (_reader, components) = reader.components()?;
    println!("components: {:?}", components.iter().collect::<Vec<_>>());
    Ok(())
}
