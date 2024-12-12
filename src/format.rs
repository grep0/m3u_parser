use std::collections::{HashMap, HashSet};

use serde::{Serialize, Deserialize};

// Partial implementation of Multivariant Playlist format as defined in RFC 8216bis

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub enum MediaType {
    Audio, Video, Subtitles, ClosedCaptions,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Resolution {
    pub w: u64,
    pub h: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum VideoRange {
    SDR, HLG, PQ,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IFrameStreamInf {
    pub uri: String,
    pub bandwidth: u64,
    pub codecs: Option<String>,
    pub resolution: Option<Resolution>,
    pub video_range: Option<VideoRange>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MultivariantPlaylist {
    pub independent_segments: bool,
    pub media: Vec<Media>,
    pub stream_inf: Vec<StreamInf>,
    pub i_frame_stream_inf: Vec<IFrameStreamInf>,
}

impl MultivariantPlaylist {
    pub fn new() -> Self {
        Self{
            independent_segments: false,
            media: vec![],
            stream_inf: vec![],
            i_frame_stream_inf: vec![]
        }
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
    pub fn validate(self: &Self) -> Result<(), String> {
        let mut group_ids = HashMap::<MediaType, HashSet<&str>>::new();
        for m in &self.media {
            if let Some(s) = group_ids.get_mut(&m.type_) {
                s.insert(&m.group_id);
            } else {
                group_ids.insert(m.type_.clone(), HashSet::from([m.group_id.as_str()]));
            }
        }
        for si in &self.stream_inf {
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

    /* Filter by audio GROUP-IN */
    pub fn select_audio_group(self: &Self, ag: &str) -> Result<Self, String> {
        let mut ret = Self::new();
        ret.independent_segments = self.independent_segments;
        let mut found = false;
        for m in &self.media {
            if m.type_ != MediaType::Audio || m.group_id==ag {
                ret.media.push(m.clone());
                found = true;
            }
        }
        if !found {
            return Err(format!("Audio group {} not found", ag).to_string());
        }
        found = false;
        for si in &self.stream_inf {
            if si.audio.is_none() || si.audio.as_ref().unwrap() == ag {
                ret.stream_inf.push(si.clone());
                found = true;
            }
        }
        if !found {
            return Err(format!("Audio group {} has no STREAM-ID associated", ag).to_string());
        }
        ret.i_frame_stream_inf = self.i_frame_stream_inf.clone();
        Ok(ret)
    }

    /* filter by bandwidth (maximum specified) */
    pub fn select_max_bandwidth(self: &Self, bw: u64) -> Result<Self, String> {
        let mut ret = Self::new();
        ret.independent_segments = self.independent_segments;
        ret.media = self.media.clone();
        let mut found = false;
        for si in &self.stream_inf {
            if si.bandwidth <= bw {
                ret.stream_inf.push(si.clone());
                found = true;
            }
        }
        if !found {
            return Err(format!("No streams with bandwidth lower than {}", bw).to_string());
        }
        ret.i_frame_stream_inf = self.i_frame_stream_inf.clone();
        Ok(ret)
    }

    // Sort EXT-I-STREAM-INF by bandwidth, descending
    pub fn sort_by_bandwidth(self: &mut Self) {
        self.stream_inf.sort_by(|a, b| b.bandwidth.cmp(&a.bandwidth));
    }

}


#[cfg(test)]
mod tests {
    use super::MultivariantPlaylist;

    fn playlist() -> MultivariantPlaylist {
        let json = include_str!("../data/playlist.json");
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn test_select_audio_group() {
        let sel = playlist().select_audio_group("aac-128k").unwrap();
        assert_eq!(sel.media.len(), 1);
        assert_eq!(sel.stream_inf.len(), 10);
        assert_eq!(sel.i_frame_stream_inf.len(), playlist().i_frame_stream_inf.len());
    }

    #[test]
    fn test_select_audio_group_not_found() {
        let sel = playlist().select_audio_group("unknown");
        assert!(sel.is_err());
    }

    fn is_sorted_rev<T>(data: &[T]) -> bool
    where T: Ord,
    {
        data.windows(2).all(|w| w[0] >= w[1])
    }

    #[test]
    fn test_select_max_bandwidth() {
        let sel = playlist().select_audio_group("atmos").unwrap();
        let sel = sel.select_max_bandwidth(10000000).unwrap();
        assert_eq!(sel.stream_inf.len(), 6);
    }

    #[test]
    fn test_sort_by_bandwidth() {
        let mut sel = playlist().select_audio_group("aac-128k").unwrap();
        sel.sort_by_bandwidth();
        let bw = sel.stream_inf.iter().map(|v| v.bandwidth).collect::<Vec<_>>();
        assert!(is_sorted_rev(&bw));
    }

}