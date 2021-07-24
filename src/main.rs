use std::fs::{read_dir, File, DirEntry};
use std::io::{self, BufReader, Result};
use std::path::{Path, PathBuf};
use brs::Reader;

const DIR: &str = "/home/ns/brickadia/builds";

fn main() -> io::Result<()> {
    let mut count = 0;
    visit_dirs(DIR.as_ref(), &mut move |e| {
        if e.path().extension() == Some("brs".as_ref()) {
            match process_file(e.path()) {
                Err(error) => {
                    println!("failed: {:?}: {:?}", e.path(), error);
                }
                _ => {}
            }

            count += 1;

            use std::io::Write;
            let mut stdout = io::stdout();
            write!(stdout, "\r{}", count)?;
            stdout.flush()?;
        }
        Ok(())
    })
}

fn process_file(path: impl AsRef<Path>) -> io::Result<()> {
    let reader = Reader::new(BufReader::new(File::open(path)?))?;
    let data = reader.into_write_data()?;
    Ok(())
}

fn visit_dirs(dir: &Path, cb: &mut impl FnMut(&DirEntry) -> io::Result<()>) -> io::Result<()> {
    if dir.is_dir() {
        for entry in read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry)?;
            }
        }
    }
    Ok(())
}
