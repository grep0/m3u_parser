use core::fmt;
use std::collections::HashMap;

use enum_extract_macro::EnumExtract;

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Debug, EnumExtract)]
enum AttributeValue<'a> {
    Integer(u64),
    Float(f64),
    QuotedString(&'a str),
    EnumeratedString(&'a str),
    DecimalResolution(u64, u64),
}

type AttributeMap<'a> = HashMap<&'a str, AttributeValue<'a>>;

#[derive(Debug, EnumExtract)]
enum ParsedLine<'a> {
    ExtM3U,
    Tag(&'a str),
    TagWithAttributes(&'a str, AttributeMap<'a>),
    Uri(&'a str),
    Empty,
}

#[derive(Parser)]
#[grammar = "m3u8.pest"]
struct M3u8Parser;

fn parse_decimal_resolution<'a>(pair: Pair<'a, Rule>) -> Option<(u64, u64)> {
    let mut inner = pair.into_inner();
    let w = inner.next().unwrap().as_str().parse().ok()?;
    let h = inner.next().unwrap().as_str().parse().ok()?;
    Some((w, h))
}

fn parse_attribute_value<'a>(pair: Pair<'a, Rule>) -> Option<AttributeValue<'a>> {
    let mut inner = pair.into_inner();
    let tail = inner.next().unwrap();
    match tail.as_rule() {
        Rule::decimal_integer => tail.as_str().parse().ok().map(AttributeValue::Integer),
        Rule::float => tail.as_str().parse().ok().map(AttributeValue::Float),
        Rule::quoted_string => {
            let tail = tail.as_str();
            let unquoted = &tail[1..tail.len()-1];
            Some(AttributeValue::QuotedString(unquoted))
        },
        Rule::enumerated_string => Some(AttributeValue::EnumeratedString(tail.as_str())),
        Rule::decimal_resolution => {
            parse_decimal_resolution(tail)
                .map(|(w, h)| AttributeValue::DecimalResolution(w, h))
        },
        _ => None
    }
}

fn parse_line<'a>(line: &'a str) -> Option<ParsedLine<'a>> {
    if line.is_empty() {
        return Some(ParsedLine::Empty);
    }
    let parsed = M3u8Parser::parse(Rule::line, line).ok()?;
    for pair in parsed {
        match pair.as_rule() {
            Rule::extm3u => return Some(ParsedLine::ExtM3U),
            Rule::tag => return Some(ParsedLine::Tag(&pair.as_str()[1..])),
            Rule::tag_with_attributes => {
                let mut attr = HashMap::new();
                let mut name = "";
                for p in pair.clone().into_inner() {
                    match p.as_rule() {
                        Rule::tag_name => name = p.as_str(),
                        Rule::attribute => {
                            let mut av = p.into_inner();
                            let key = av.next().unwrap().as_str();
                            let value = parse_attribute_value(av.next().unwrap())?;
                            attr.insert(key, value);
                        },
                        _ => unreachable!()
                    }
                }
                return Some(ParsedLine::TagWithAttributes(name, attr));
            },
            Rule::uri => return Some(ParsedLine::Uri(pair.as_str())),
            _ => unreachable!()
        }
    }
    None
}

use crate::format;

#[derive(Debug)]
pub struct ParseError {
    message: &'static str,
    lineno: usize,
}

impl ParseError {
    pub fn new(message: &'static str, lineno: usize) -> Self {
        ParseError{message, lineno}
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // lineno+1 as we want base 1 line numbers
        write!(f, "{} at line {}", self.message, self.lineno+1)
    }
}

fn as_media_type(v: &AttributeValue) -> Option<format::MediaType> {
    match *(v.as_enumerated_string().ok()?) {
        "AUDIO" => Some(format::MediaType::Audio),
        "VIDEO" => Some(format::MediaType::Video),
        "SUBTITLES" => Some(format::MediaType::Subtitles),
        "CLOSED-CAPTIONS" => Some(format::MediaType::ClosedCaptions),
        _ => None
    }
}

fn as_bool(v: &AttributeValue) -> Option<bool> {
    match *(v.as_enumerated_string().ok()?) {
        "YES" => Some(true),
        "NO" => Some(false),
        _ => None
    }
}

fn as_video_range(v: &AttributeValue) -> Option<format::VideoRange> {
    match *(v.as_enumerated_string().ok()?) {
        "SDR" => Some(format::VideoRange::SDR),
        "HLG" => Some(format::VideoRange::HLG),
        "PQ" => Some(format::VideoRange::PQ),
        _ => None
    }
}

fn as_resolution(v: &AttributeValue) -> Option<format::Resolution> {
    let res = v.as_decimal_resolution().ok()?;
    Some(format::Resolution{w: *res.0, h: *res.1})
}

pub fn parse_resolution_param(s: &str) -> Option<format::Resolution> {
    let mut parsed = M3u8Parser::parse(Rule::decimal_resolution, s).ok()?;
    let w = parsed.next().unwrap().as_str().parse().ok()?;
    let h = parsed.next().unwrap().as_str().parse().ok()?;
    Some(format::Resolution{w, h})
}

fn intepret_ext_x_media(attr: &AttributeMap) -> Option<format::Media> {
    Some(format::Media{
        type_: as_media_type(attr.get("TYPE")?)?,
        uri: attr.get("URI")?.as_quoted_string().ok()?.to_string(),
        group_id: attr.get("GROUP-ID")?.as_quoted_string().ok()?.to_string(),
        language: attr.get("LANGUAGE").map_or(None, |v| Some(v.as_quoted_string().ok()?.to_string())),
        name: attr.get("NAME")?.as_quoted_string().ok()?.to_string(),
        default: attr.get("DEFAULT").map_or(None, as_bool)?,
        autoselect: attr.get("AUTOSELECT").map_or(None, as_bool)?,
        channels: attr.get("CHANNELS").map_or(None, |v| Some(v.as_quoted_string().ok()?.to_string())),
    })
}

fn interpret_ext_x_stream_inf(attr: &AttributeMap) -> Option<format::StreamInf> {
    Some(format::StreamInf{
        uri: String::new(), // to be filled later
        bandwidth: *attr.get("BANDWIDTH")?.as_integer().ok()?,
        average_bandwidth: attr.get("AVERAGE-BANDWIDTH").map_or(None, |v| Some(*v.as_integer().ok()?)),
        codecs: attr.get("CODECS").map_or(None, |v| Some(v.as_quoted_string().ok()?.to_string())),
        resolution: attr.get("RESOLUTION").map_or(None, as_resolution),
        frame_rate: attr.get("FRAME-RATE").map_or(None,  |v| Some(*v.as_float().ok()?)),
        video_range: attr.get("VIDEO-RANGE").map_or(None, |v| as_video_range(v)),
        audio: attr.get("AUDIO").map_or(None, |v| Some(v.as_quoted_string().ok()?.to_string())),
        closed_captions: attr.get("CLOSED-CAPTIONS").map_or(None,
            |v| {
                match *v {
                    AttributeValue::QuotedString(s) => Some(s.to_string()),
                    AttributeValue::EnumeratedString("NONE") => None,
                    _ => None,
                }
            }),
    })
}

fn interpret_ext_x_i_frame_stream_inf(attr: &AttributeMap) -> Option<format::IFrameStreamInf> {
    Some(format::IFrameStreamInf{
        uri: attr.get("URI")?.as_quoted_string().ok()?.to_string(),
        bandwidth: *attr.get("BANDWIDTH")?.as_integer().ok()?,
        codecs: attr.get("CODECS").map_or(None, |v| Some(v.as_quoted_string().ok()?.to_string())),
        resolution: attr.get("RESOLUTION").map_or(None, as_resolution),
        video_range: attr.get("VIDEO-RANGE").map_or(None, |v| as_video_range(v)),
    })
}

pub fn parse_playlist(data: &str) -> Result<format::MultivariantPlaylist, ParseError> {
    let mut playlist = format::MultivariantPlaylist::new();
    let mut expect_uri = false;
    for (lineno, line) in data.split('\n').enumerate() {
        let Some(parsed) = parse_line(line) else {
            return Err(ParseError::new("Failed to parse line", lineno))
        };
        if lineno == 0 {
            match parsed {
                ParsedLine::ExtM3U => (),
                _ => return Err(ParseError::new("No #EXTM3U", 0))
            }
        } else if expect_uri {
            match parsed {
                ParsedLine::Uri(uri) => {
                    playlist.stream_inf.last_mut().unwrap().uri = uri.to_string();
                    expect_uri = false;
                },
                _ => return Err(ParseError::new("Expected URI line not found", lineno))
            }
        } else {
            match parsed {
                ParsedLine::Empty => (), // ignore empty lines
                ParsedLine::Tag("EXT-X-INDEPENDENT-SEGMENTS") => {
                    playlist.independent_segments = true;
                },
                ParsedLine::TagWithAttributes("EXT-X-MEDIA", attr) => {
                    if let Some(m) = intepret_ext_x_media(&attr) {
                        playlist.media.push(m)
                    } else {
                        return Err(ParseError::new("Failed to interpret EXT-X-MEDIA", lineno))
                    }
                },
                ParsedLine::TagWithAttributes("EXT-X-STREAM-INF", attr) => {
                    if let Some(m) = interpret_ext_x_stream_inf(&attr) {
                        playlist.stream_inf.push(m);
                        expect_uri = true;
                    } else {
                        return Err(ParseError::new("Failed to interpret EXT-X-STREAM-INF", lineno))
                    }
                },
                ParsedLine::TagWithAttributes("EXT-X-I-FRAME-STREAM-INF", attr) => {
                    if let Some(m) = interpret_ext_x_i_frame_stream_inf(&attr) {
                        playlist.i_frame_stream_inf.push(m)
                    } else {
                        return Err(ParseError::new("Failed to interpret EXT-X-I-FRAME-STREAM-INF", lineno))
                    }
                },
                _ => {
                    return Err(ParseError::new("Unexpected line", lineno))
                }
            }
        }
    }
    if expect_uri {
        return Err(ParseError::new("File truncated without an expected URI line after EXT-X-STREAM-INF", 0));
    }
    if playlist.media.is_empty() && playlist.stream_inf.is_empty() && playlist.i_frame_stream_inf.is_empty() {
        return Err(ParseError::new("Empty playlist", 0));
    }

    Ok(playlist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        let Some(ParsedLine::Empty) = parse_line("") else { panic!() };

        let Some(ParsedLine::ExtM3U) = parse_line("#EXTM3U") else { panic!() };

        let Some(ParsedLine::Tag("EXT-X-INDEPENDENT-SEGMENTS")) = parse_line("#EXT-X-INDEPENDENT-SEGMENTS") else { panic!() };

        let lmedia = r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aac-128k",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,CHANNELS="2",URI="audio/unenc/aac_128k/vod.m3u8""#;
        let Some(ParsedLine::TagWithAttributes("EXT-X-MEDIA", attrs)) = parse_line(lmedia)
        else { panic!() };
        let AttributeValue::EnumeratedString("AUDIO") = attrs["TYPE"] else { panic!() };
        let AttributeValue::QuotedString("audio/unenc/aac_128k/vod.m3u8") = attrs["URI"] else { panic!() };

        let Some(ParsedLine::Uri("hdr10/unenc/1650k/vod.m3u8")) = parse_line("hdr10/unenc/1650k/vod.m3u8") else { panic!() };
    }

    #[test]
    fn test_intepret_ext_x_media() {
        let l = r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aac-128k",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,CHANNELS="2",URI="audio/unenc/aac_128k/vod.m3u8""#;
        let parsed = parse_line(l).unwrap();
        let attr = parsed.extract_as_tag_with_attributes().1;
        let m = intepret_ext_x_media(attr).unwrap();
        assert_eq!(m.type_, format::MediaType::Audio);
        assert_eq!(m.group_id, "aac-128k");
        assert_eq!(m.name, "English");
        assert_eq!(m.language.unwrap(), "en");
        assert!(m.default);
        assert!(m.autoselect);
        assert_eq!(m.channels.unwrap(), "2");
        assert_eq!(m.uri, "audio/unenc/aac_128k/vod.m3u8");
    }

    #[test]
    fn test_intepret_ext_x_stream_inf() {
        let l = r#"#EXT-X-STREAM-INF:BANDWIDTH=2483789,AVERAGE-BANDWIDTH=1762745,CODECS="mp4a.40.2,hvc1.2.4.L90.90",RESOLUTION=960x540,FRAME-RATE=23.97,VIDEO-RANGE=PQ,AUDIO="aac-128k",CLOSED-CAPTIONS=NONE"#;
        let parsed = parse_line(l).unwrap();
        let attr = parsed.extract_as_tag_with_attributes().1;
        let m = interpret_ext_x_stream_inf(attr).unwrap();
        assert_eq!(m.uri, "");
        assert_eq!(m.bandwidth, 2483789);
        assert_eq!(m.average_bandwidth.unwrap(), 1762745);
        assert_eq!(m.codecs.unwrap(), "mp4a.40.2,hvc1.2.4.L90.90");
        assert_eq!(m.resolution.unwrap(), format::Resolution{w: 960, h: 540});
        assert_eq!(m.frame_rate.unwrap(), 23.97);
        assert_eq!(m.video_range.unwrap(), format::VideoRange::PQ);
        assert_eq!(m.audio.unwrap(), "aac-128k");
        assert_eq!(m.closed_captions, None);
    }

    #[test]
    fn test_interpret_ext_x_i_frame_stream_inf() {
        let l = r#"#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=222552,CODECS="hvc1.2.4.L93.90",RESOLUTION=1280x720,VIDEO-RANGE=PQ,URI="hdr10/unenc/3300k/vod-iframe.m3u8""#;
        let parsed = parse_line(l).unwrap();
        let attr = parsed.extract_as_tag_with_attributes().1;
        let m = interpret_ext_x_i_frame_stream_inf(attr).unwrap();
        assert_eq!(m.uri, "hdr10/unenc/3300k/vod-iframe.m3u8");
        assert_eq!(m.bandwidth, 222552);
        assert_eq!(m.codecs.unwrap(), "hvc1.2.4.L93.90");
        assert_eq!(m.resolution, Some(format::Resolution{w: 1280, h: 720}));
        assert_eq!(m.video_range.unwrap(), format::VideoRange::PQ);
    }

    #[test]
    fn test_parse_playlist() {
        let pl = 
            r#"#EXTM3U
#EXT-X-INDEPENDENT-SEGMENTS

#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aac-128k",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,CHANNELS="2",URI="audio/unenc/aac_128k/vod.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=2483789,AVERAGE-BANDWIDTH=1762745,CODECS="mp4a.40.2,hvc1.2.4.L90.90",RESOLUTION=960x540,FRAME-RATE=23.97,VIDEO-RANGE=PQ,AUDIO="aac-128k",CLOSED-CAPTIONS=NONE
hdr10/unenc/1650k/vod.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=15811232,AVERAGE-BANDWIDTH=10058085,CODECS="mp4a.40.2,hvc1.2.4.L150.90",RESOLUTION=2560x1440,FRAME-RATE=23.97,VIDEO-RANGE=PQ,AUDIO="aac-128k",CLOSED-CAPTIONS=NONE
hdr10/unenc/10000k/vod.m3u8

#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=222552,CODECS="hvc1.2.4.L93.90",RESOLUTION=1280x720,VIDEO-RANGE=PQ,URI="hdr10/unenc/3300k/vod-iframe.m3u8"

"#;
        let m3u = parse_playlist(pl).unwrap();
        assert!(m3u.independent_segments);
        assert_eq!(m3u.media.len(), 1);
        assert_eq!(m3u.media[0].group_id, "aac-128k");
        assert_eq!(m3u.stream_inf.len(), 2);
        assert_eq!(m3u.stream_inf[0].resolution, Some(format::Resolution{w: 960, h: 540}));
        assert_eq!(m3u.stream_inf[0].uri, "hdr10/unenc/1650k/vod.m3u8");
        assert_eq!(m3u.stream_inf[1].resolution, Some(format::Resolution{w: 2560, h: 1440}));
        assert_eq!(m3u.stream_inf[1].uri, "hdr10/unenc/10000k/vod.m3u8");
        assert_eq!(m3u.i_frame_stream_inf.len(), 1);
        assert_eq!(m3u.i_frame_stream_inf[0].uri, "hdr10/unenc/3300k/vod-iframe.m3u8");
    }

    #[test]
    fn test_missing_extm3u() {
        let data = include_str!("../data/missing_extm3u.m3u8");
        let parsed = parse_playlist(&data);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_missing_stream_inf_uri() {
        let data = include_str!("../data/missing_stream_inf_uri.m3u8");
        let parsed = parse_playlist(&data);
        assert!(parsed.is_err());
    }
    
    #[test]
    fn test_truncated() {
        let data = include_str!("../data/truncated.m3u8");
        let parsed = parse_playlist(&data);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_validation_error() {
        let data = include_str!("../data/validation_error.m3u8");
        let Ok(m3u) = parse_playlist(&data) else { panic!(); };
        let validate = m3u.validate();
        assert!(validate.is_err());
    }

}