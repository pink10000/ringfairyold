use std::{error::Error, fs::File};

use ringfairy::website::Website;

fn main() -> Result<(), Box<dyn Error>> {
    let websites: Vec<Website> = serde_json::from_reader(File::open("websites.json")?)?;

    eprintln!("{websites:#?}");

    Ok(())
}
