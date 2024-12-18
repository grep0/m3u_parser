mod parser;
mod format;

use std::fs;
use serde_json;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Filename or http:/https: url to parse
    #[arg(long)]
    uri: String,
    /// Filter by AUDIO-GROUP
    #[arg(long)]
    audio_group: Option<String>,
    /// Filter by AUDIO CHANNELS
    #[arg(long)]
    audio_channels: Option<String>,
    /// Filter EXT-X-STREAM-INF by bandwidth (maximum specified)
    #[arg(long)]
    max_bandwidth: Option<u64>,
    /// Filter EXT-X-STREAM-INF and EXT-X-I-FRAME-STREAM-INF by resolution (exact, WxH)
    #[arg(long)]
    resolution: Option<String>,
    /// Sort EXT-X-STREAM-INF by bandwidth (descending)
    #[arg(long, default_value_t=false)]
    sort_by_bandwidth: bool,
}

fn main() {
    let args = Args::parse();

    let contents = 
        if args.uri.starts_with("http://") || args.uri.starts_with("https://") {
            ureq::get(&args.uri).call()
                .expect("Failed to read url")
                .into_string()
                .expect("Failed to parse url from string")
        } else {
            fs::read_to_string(&args.uri).expect("Failed to read file")
        };

    let mut m3u = parser::parse_playlist(&contents).expect("Failed to parse file");

    if let Some(ag) = &args.audio_group {
        m3u = m3u.select_audio_group(ag).expect("Failed to select audio group");
    }

    if let Some(ch) = &args.audio_channels {
        m3u = m3u.select_audio_by_channels(ch).expect("Failed to select audio channels");
    }

    if let Some(bw) = &args.max_bandwidth {
        m3u = m3u.select_max_bandwidth(*bw).expect("Failed to select by max bandwidth");
    }

    if let Some(resstr) = &args.resolution {
        let res = parser::parse_resolution_param(resstr).expect("Failed to parse resolution param");
        m3u = m3u.select_resolution(&res).expect("Failed to select by resolution");
    }

    if args.sort_by_bandwidth {
        m3u.sort_by_bandwidth();
    }

    m3u.validate().expect("Format validation error");

    println!("{}", serde_json::to_string_pretty(&m3u).unwrap());
}
