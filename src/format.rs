use std::collections::{HashMap, HashSet};

use serde::{Serialize, Deserialize};

// Partial implementation of Multivariant Playlist format as defined in RFC 8216bis

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
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
    pub channels: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Resolution {
    pub w: u64,
    pub h: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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

/* 
Perform basic validation of the playlist:

In EXT-X-STREAM-INF:
   *  AUDIO value MUST match the value of the
      GROUP-ID attribute of an EXT-X-MEDIA tag elsewhere in the
      Multivariant Playlist whose TYPE attribute is AUDIO.
   * CLOSED-CAPTIONS can be either a quoted-string or an enumerated-string
      with the value NONE.  If the value is a quoted-string, it MUST
      match the value of the GROUP-ID attribute of an EXT-X-MEDIA tag
      elsewhere in the Playlist whose TYPE attribute is CLOSED-CAPTIONS
   
TODO: consider implementing more validation.
*/
pub fn validate(m3u: &MultivariantPlaylist) -> Result<(), String> {
    let mut group_ids = HashMap::<MediaType, HashSet<&str>>::new();
    for m in &m3u.media {
        if let Some(s) = group_ids.get_mut(&m.type_) {
            s.insert(&m.group_id);
        } else {
            group_ids.insert(m.type_.clone(), HashSet::from([m.group_id.as_str()]));
        }
    }
    for si in &m3u.stream_inf {
        if let Some(au) = &si.audio {
            if !group_ids.get(&MediaType::Audio).map(|s| s.contains(au.as_str()))
                .unwrap_or(false) {
                return Err(format!("Reference to unknown AUDIO group {}", au).to_string())
            }
        }
        if let Some(cc) = &si.closed_captions {
            if !group_ids.get(&MediaType::ClosedCaptions).map(|s| s.contains(cc.as_str()))
                .unwrap_or(false) {
                return Err(format!("Reference to unknown CLOSED-CAPTIONS group {}", cc).to_string())
            }
        }
    }

    Ok(())
}
