fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).expect("missing path");
    let reader = brs::Reader::new(std::fs::File::open(path)?)?;
    // Optional: let (reader, screenshot) = reader.screenshot_data()?;
    let (reader, bricks) = reader.bricks()?;
    println!("{:?}", reader);
    let mut last_brick = None;
    for brick in bricks {
        let brick = brick?;
        last_brick = Some(brick);
    }
    println!("{:?}", last_brick);
    let (_reader, components) = reader.components()?;
    println!("components: {:?}", components.iter().collect::<Vec<_>>());
    Ok(())
}
