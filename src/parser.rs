use std::collections::HashMap;

use regex_static::once_cell::sync::Lazy;
use regex::{Regex, Captures};
use enum_extract_macro::EnumExtract;

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

// Consume regex in the beginning of the stream. If success, return caputres and tail of the string.
// For optimization, the regexes should start with '^', but it is not necessary
fn consume<'a>(s: &'a str, re: &Regex) -> Option<(Captures<'a>, &'a str)> {
    if let Some(m) = re.captures_at(s, 0) {
        let g0 = &m.get(0).unwrap();
        if g0.start() == 0 { // verify once again that capture starts as 0
            let tail = &s[g0.len()..];
            return Some((m, tail))
        }
    }
    None
}

static RE_RESOLUTION: Lazy<Regex> = regex_static::lazy_regex!(r#"^([0-9]+)x([0-9]+)$"#);

fn parse_resolution(res: &str) -> Option<AttributeValue> {
    if let Some(m) = RE_RESOLUTION.captures(res) {
        Some(AttributeValue::DecimalResolution(
            m.get(1)?.as_str().parse().ok()?,
            m.get(2)?.as_str().parse().ok()?))
    } else {
        None
    }
}

static RE_ATTRIBUTE_VALUE: Lazy<Regex> = 
    regex_static::lazy_regex!(r#"^([0-9]+\.[0-9]+)|^"([^"]+)"|^([[:alpha:]-]+)|^([0-9]+x[0-9]+)|^([0-9]+)"#);

// TODO: more verbose parse error
fn parse_attribute_value<'a>(value: &'a str) -> Option<(&'a str, AttributeValue<'a>)> {
    if let Some((m, tail)) = consume(value, &RE_ATTRIBUTE_VALUE) {
        let av =
            if let Some(mf) = m.get(1) {
                AttributeValue::Float(mf.as_str().parse::<f64>().ok()?)
            } else if let Some(mqs) = m.get(2) {
                AttributeValue::QuotedString(mqs.as_str())
            } else if let Some(mes) = m.get(3) {
                AttributeValue::EnumeratedString(mes.as_str())
            } else if let Some(mres) = m.get(4) {
                parse_resolution(mres.as_str()).unwrap()
            } else if let Some(mdec) = m.get(5) {
                AttributeValue::Integer(mdec.as_str().parse::<u64>().ok()?)
            } else {
                panic!("unexpected parser state")
            };
        Some((tail, av))
    } else {
        None
    }
}

static RE_ATTRIBUTE_NAME : Lazy<Regex> = regex_static::lazy_regex!(r#"^([[:alpha:]-]+)="#);

fn parse_attributes<'a>(value: &'a str) -> Option<AttributeMap<'a>> {
    let mut tail = value;
    let mut result = AttributeMap::new();
    while !tail.is_empty() {
        let Some((mkey, t)) = consume(tail, &RE_ATTRIBUTE_NAME)
        else { return None };
        let key = mkey.get(1).unwrap().as_str();
        tail = t;
        let Some((t, av)) = parse_attribute_value(tail)
        else { return None };
        result.insert(key, av);
        if t.is_empty() { break }
        if !t.starts_with(",") { return None } // consume trailing comma
        tail = &t[1..];
    }
    Some(result)
}

static RE_TAG_NAME: Lazy<Regex> = regex_static::lazy_regex!(r#"^#(EXT-X-[[:alpha:]-]+)($|:)"#);
static RE_URI: Lazy<Regex> = regex_static::lazy_regex!(r#"^([[:alnum:]/.:])+$"#);

fn parse_line<'a>(line: &'a str) -> Option<ParsedLine<'a>> {
    if line.is_empty() {
        return Some(ParsedLine::Empty);
    }
    if line == "#EXTM3U" {
        return Some(ParsedLine::ExtM3U);
    }
    if let Some((mtag, tail)) = consume(line, &RE_TAG_NAME) {
        let tag = mtag.get(1).unwrap().as_str();
        if tail.is_empty() {
            return Some(ParsedLine::Tag(tag));
        }
        if let Some (attr) = parse_attributes(tail) {
            return Some(ParsedLine::TagWithAttributes(tag, attr))
        } else {
            return None
        }
    }
    if let Some(_) = RE_URI.captures(line) {
        return Some(ParsedLine::Uri(line))
    }
    None
}

use crate::format;

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

pub fn parse_playlist(data: &str) -> Result<format::MultivariantPlaylist, String> {
    let mut playlist = format::MultivariantPlaylist{
        independent_segments: false,
        media: vec![],
        stream_inf: vec![],
        i_frame_stream_inf: vec![]
    };
    let mut expect_uri = false;
    for (lineno, line) in data.split('\n').enumerate() {
        let Some(parsed) = parse_line(line) else {
            return Err(format!("Parse error at line {}", lineno).to_string())
        };
        if lineno == 0 {
            match parsed {
                ParsedLine::ExtM3U => (),
                _ => return Err("No #EXTM3U at first line".to_string())
            }
        } else if expect_uri {
            match parsed {
                ParsedLine::Uri(uri) => {
                    playlist.stream_inf.last_mut().unwrap().uri = uri.to_string();
                    expect_uri = false;
                },
                _ => return Err(format!("Expected URI line not found at line {}", lineno).to_string())
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
                        return Err(format!("Failed to interpret EXT-X-MEDIA at line {}", lineno).to_string())
                    }
                },
                ParsedLine::TagWithAttributes("EXT-X-STREAM-INF", attr) => {
                    if let Some(m) = interpret_ext_x_stream_inf(&attr) {
                        playlist.stream_inf.push(m);
                        expect_uri = true;
                    } else {
                        return Err(format!("Failed to interpret EXT-X-STREAM-INF at line {}", lineno).to_string())
                    }
                },
                ParsedLine::TagWithAttributes("EXT-X-I-FRAME-STREAM-INF", attr) => {
                    if let Some(m) = interpret_ext_x_i_frame_stream_inf(&attr) {
                        playlist.i_frame_stream_inf.push(m)
                    } else {
                        return Err(format!("Failed to interpret EXT-X-I-FRAME-STREAM-INF at line {}", lineno).to_string())
                    }
                },
                _ => {
                    return Err(format!("Unexpected line {}", lineno).to_string())
                }
            }
        }
    }
    if expect_uri {
        return Err("Expected URI at last line not found".to_string());
    }
    if playlist.media.is_empty() && playlist.stream_inf.is_empty() && playlist.i_frame_stream_inf.is_empty() {
        return Err("Empty playlist".to_string());
    }

    Ok(playlist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_attribute_value() {
        if let Some((tail,AttributeValue::Float(d))) = parse_attribute_value("12.5,tail") {
            assert_eq!(tail, ",tail");
            assert_eq!(d, 12.5);
        } else {
            assert!(false)
        }

        if let Some((_,AttributeValue::DecimalResolution(w, h))) = parse_attribute_value("2560x1440") {
            assert_eq!(w, 2560);
            assert_eq!(h, 1440);
        } else {
            assert!(false)
        }

        if let Some((_,AttributeValue::Integer(v))) = parse_attribute_value("10058085") {
            assert_eq!(v, 10058085);
        } else {
            assert!(false)
        }

        if let Some((_,AttributeValue::QuotedString(v))) = parse_attribute_value(r#""mp4a.40.2,hvc1.2.4.L150.90""#) {
            assert_eq!(v, "mp4a.40.2,hvc1.2.4.L150.90");
        } else {
            assert!(false)
        }


        if let Some((tail,AttributeValue::EnumeratedString(v))) = parse_attribute_value("PQ,SOMETHING") {
            assert_eq!(tail, ",SOMETHING");
            assert_eq!(v, "PQ");
        } else {
            assert!(false)
        }
    }

    #[test]
    fn test_parse_attribute_str() {
        let astr = r#"BANDWIDTH=15811232,AVERAGE-BANDWIDTH=10058085,CODECS="mp4a.40.2,hvc1.2.4.L150.90",RESOLUTION=2560x1440,FRAME-RATE=23.97,VIDEO-RANGE=PQ,AUDIO="aac-128k",CLOSED-CAPTIONS=NONE"#;
        let parsed = parse_attributes(astr);
        assert!(parsed.is_some());
        let parsed = parsed.unwrap();
        if let AttributeValue::Integer(bw) = &parsed["BANDWIDTH"] {
            assert_eq!(*bw, 15811232);
        } else {
            assert!(false)
        }
        if let AttributeValue::Integer(bw) = &parsed["AVERAGE-BANDWIDTH"] {
            assert_eq!(*bw, 10058085);
        } else {
            assert!(false)
        }
        if let AttributeValue::QuotedString(s) = &parsed["CODECS"] {
            assert_eq!(*s, "mp4a.40.2,hvc1.2.4.L150.90");
        } else {
            assert!(false)
        }
        if let AttributeValue::EnumeratedString(s) = &parsed["CLOSED-CAPTIONS"] {
            assert_eq!(*s, "NONE");
        } else {
            assert!(false)
        }
    }

    #[test]
    fn test_parse_line() {
        if let Some(ParsedLine::Empty) = parse_line("") {
            assert!(true);
        } else {
            assert!(false);
        }

        if let Some(ParsedLine::ExtM3U) = parse_line("#EXTM3U") {
            assert!(true);
        } else {
            assert!(false);
        }

        if let Some(ParsedLine::Tag(tag)) = parse_line("#EXT-X-INDEPENDENT-SEGMENTS") {
            assert_eq!(tag, "EXT-X-INDEPENDENT-SEGMENTS");
        } else {
            assert!(false);
        }

        let lmedia = r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aac-128k",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,CHANNELS="2",URI="audio/unenc/aac_128k/vod.m3u8""#;
        if let Some(ParsedLine::TagWithAttributes(tag, attrs)) = parse_line(lmedia) {
            assert_eq!(tag, "EXT-X-MEDIA");
            if let AttributeValue::EnumeratedString(s) = attrs["TYPE"] {
                assert_eq!(s, "AUDIO");
            } else {
                assert!(false);
            }
            if let AttributeValue::QuotedString(s) = attrs["URI"] {
                assert_eq!(s, "audio/unenc/aac_128k/vod.m3u8");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        if let Some(ParsedLine::Uri(u)) = parse_line("hdr10/unenc/1650k/vod.m3u8") {
            assert_eq!(u, "hdr10/unenc/1650k/vod.m3u8");
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_intepret_ext_x_media() {
        let l = r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aac-128k",NAME="English",LANGUAGE="en",DEFAULT=YES,AUTOSELECT=YES,CHANNELS="2",URI="audio/unenc/aac_128k/vod.m3u8""#;
        let parsed = parse_line(l).unwrap();
        let attr = parsed.extract_as_tag_with_attributes().1;
        if let Some(m) = intepret_ext_x_media(attr) {
            assert_eq!(m.type_, format::MediaType::Audio);
            assert_eq!(m.group_id, "aac-128k");
            assert_eq!(m.name, "English");
            assert_eq!(m.language.unwrap(), "en");
            assert!(m.default);
            assert!(m.autoselect);
            assert_eq!(m.channels.unwrap(), "2");
            assert_eq!(m.uri, "audio/unenc/aac_128k/vod.m3u8");
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_intepret_ext_x_stream_inf() {
        let l = r#"#EXT-X-STREAM-INF:BANDWIDTH=2483789,AVERAGE-BANDWIDTH=1762745,CODECS="mp4a.40.2,hvc1.2.4.L90.90",RESOLUTION=960x540,FRAME-RATE=23.97,VIDEO-RANGE=PQ,AUDIO="aac-128k",CLOSED-CAPTIONS=NONE"#;
        let parsed = parse_line(l).unwrap();
        let attr = parsed.extract_as_tag_with_attributes().1;
        if let Some(m) = interpret_ext_x_stream_inf(attr) {
            assert_eq!(m.uri, "");
            assert_eq!(m.bandwidth, 2483789);
            assert_eq!(m.average_bandwidth.unwrap(), 1762745);
            assert_eq!(m.codecs.unwrap(), "mp4a.40.2,hvc1.2.4.L90.90");
            assert_eq!(m.resolution.unwrap(), format::Resolution{w: 960, h: 540});
            assert_eq!(m.frame_rate.unwrap(), 23.97);
            assert_eq!(m.video_range.unwrap(), format::VideoRange::PQ);
            assert_eq!(m.audio.unwrap(), "aac-128k");
            assert_eq!(m.closed_captions, None);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_interpret_ext_x_i_frame_stream_inf() {
        let l = r#"#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=222552,CODECS="hvc1.2.4.L93.90",RESOLUTION=1280x720,VIDEO-RANGE=PQ,URI="hdr10/unenc/3300k/vod-iframe.m3u8""#;
        let parsed = parse_line(l).unwrap();
        let attr = parsed.extract_as_tag_with_attributes().1;
        if let Some(m) = interpret_ext_x_i_frame_stream_inf(attr) {
            assert_eq!(m.uri, "hdr10/unenc/3300k/vod-iframe.m3u8");
            assert_eq!(m.bandwidth, 222552);
            assert_eq!(m.codecs.unwrap(), "hvc1.2.4.L93.90");
            assert_eq!(m.resolution, Some(format::Resolution{w: 1280, h: 720}));
            assert_eq!(m.video_range.unwrap(), format::VideoRange::PQ);
        } else {
            assert!(false)
        }
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
}