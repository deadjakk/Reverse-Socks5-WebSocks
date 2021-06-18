use std::error::Error;
use client::*;
mod config;
use config::{PORT,IP};


// base socks code sourced from https://github.com/ajmwagar/merino,
// with modifications by @deadjakk

fn main() -> Result<(), Box<dyn Error>> {
    let mut client = Client::new(PORT, IP)?;

    loop {
        match client.serve(){
            Ok(_) => (),
            Err(e) => println!("{:?}",e),
        };
    }
}
