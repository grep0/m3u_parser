use serde::{Serialize, Deserialize};

// Partial implementation of Multivariant Playlist format as defined in RFC 8216bis

#[derive(Serialize, Deserialize, Debug)]
pub enum MediaType {
    Audio, Video, Subtitles, ClosedCaptions,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Media {
    pub type_: MediaType,
    pub uri: String,
    pub group_id: String,
    pub language: Option<String>,
    pub name: String,
    pub default: bool,
    pub autoselect: bool,
    pub channels: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resolution {
    pub w: u64,
    pub h: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum VideoRange {
    SDR, HLG, PQ,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StreamInf {
    pub uri: String,
    pub bandwidth: u64,
    pub average_bandwidth: Option<u64>,
    pub codecs: Option<String>,
    pub resolution: Option<Resolution>,
    pub frame_rate: Option<f64>, // could be decimal for precision
    pub video_range: Option<VideoRange>,
    pub audio: Option<String>,
    pub closed_captions: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IFrameStreamInf {
    pub uri: String,
    pub bandwidth: u64,
    pub codecs: Option<String>,
    pub resolution: Option<Resolution>,
    pub video_range: Option<VideoRange>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MultivariantPlaylist {
    pub independent_segments: bool,
    pub media: Vec<Media>,
    pub stream_inf: Vec<StreamInf>,
    pub i_frame_stream_inf: Vec<IFrameStreamInf>,
}