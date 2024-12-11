mod parser;
mod format;

use std::{env, fs};
use serde_json;

fn main() {
    let argv: Vec<String> = env::args().collect();
    if argv.len() != 2 {
        println!("Usage: {} filename_or_url", argv[0]);
        return;
    }
    let uri = &argv[1];
    let contents = 
        if uri.starts_with("http://") || uri.starts_with("https://") {
            ureq::get(uri).call()
                .expect("Failed to read url")
                .into_string()
                .expect("Failed to parse url from string")
        } else {
            fs::read_to_string(uri).expect("Failed to read file")
        };
    let m3u = parser::parse_playlist(&contents).expect("Failed to parse file");
    format::validate(&m3u).expect("Format validation error");

    print!("{}", serde_json::to_string_pretty(&m3u).unwrap());
}
