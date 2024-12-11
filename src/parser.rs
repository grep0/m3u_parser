use std::collections::HashMap;

use regex_static::{lazy_regex, once_cell::sync::Lazy};
use regex::{Regex, Captures};

#[derive(Debug)]
enum AttributeValue<'a> {
    Integer(u64),
    Float(f64),
    QuotedString(&'a str),
    EnumeratedString(&'a str),
    DecimalResolution(u64, u64),
}

#[derive(Debug)]
enum ParsedLine<'a> {
    ExtM3U,
    Tag(&'a str),
    TagWithAttributes(&'a str, HashMap<&'a str, AttributeValue<'a>>),
    Uri(&'a str),
    Empty,
}

static reTagName: Lazy<Regex> = regex_static::lazy_regex!(r#"^#(EXT-X-[[:alpha:]-]+)($|:)"#);
static reAttributeName : Lazy<Regex> = regex_static::lazy_regex!(r#"^([[:alpha:]-]+)="#);
static reAttributeValue: Lazy<Regex> = 
    regex_static::lazy_regex!(r#"^([0-9]+\.[0-9]+)|^"([^"]+)"|^([[:alpha:]-]+)|^([0-9]+x[0-9]+)|^([0-9]+)"#);
static reResolution: Lazy<Regex> = regex_static::lazy_regex!(r#"([0-9]+)x([0-9]+)"#);
static reUri: Lazy<Regex> = regex_static::lazy_regex!(r#"^([[:alnum:]/.:])+$"#);

fn get_tail<'a>(value: &'a str, c: &Captures) -> &'a str {
    &value[c.get(0).unwrap().len()..]
}

fn parse_resolution(res: &str) -> Option<AttributeValue> {
    if let Some(m) = reResolution.captures(res) {
        Some(AttributeValue::DecimalResolution(
            m.get(1).unwrap().as_str().parse().unwrap(),
            m.get(2).unwrap().as_str().parse().unwrap()))
    } else {
        None
    }
}

// FIXME: there are some corner cases not covered, e.g. too long integers. Ignore for now.
fn parse_attribute_value<'a>(value: &'a str) -> Option<(&'a str, AttributeValue<'a>)> {
    if let Some(m) = reAttributeValue.captures(value) {
        let tail = get_tail(value, &m);
        let av =
            if let Some(mf) = m.get(1) {
                AttributeValue::Float(mf.as_str().parse::<f64>().unwrap())
            } else if let Some(mqs) = m.get(2) {
                AttributeValue::QuotedString(mqs.as_str())
            } else if let Some(mes) = m.get(3) {
                AttributeValue::EnumeratedString(mes.as_str())
            } else if let Some(mres) = m.get(4) {
                parse_resolution(mres.as_str()).unwrap()
            } else if let Some(mdec) = m.get(5) {
                AttributeValue::Integer(mdec.as_str().parse::<u64>().unwrap())
            } else {
                panic!("unexpected parser state")
            };
        Some((tail, av))
    } else {
        None
    }
}

fn parse_attributes<'a>(value: &'a str) -> Option<HashMap<&'a str, AttributeValue<'a>>> {
    let mut tail = value;
    let mut result = HashMap::new();
    while !tail.is_empty() {
        let mkey = reAttributeName.captures(tail);
        if mkey.is_none() {
            return None
        }
        let mkey = mkey.unwrap();
        let key = mkey.get(1).unwrap().as_str();
        tail = get_tail(tail, &mkey);
        if let Some((t, av)) = parse_attribute_value(tail) {
            result.insert(key, av);
            if t.is_empty() { break }
            if !t.starts_with(",") { return None } // consume trailing comma
            tail = &t[1..];
        } else {
            return None
        }
    }
    Some(result)
}

fn parse_line<'a>(line: &'a str) -> Option<ParsedLine<'a>> {
    if line.is_empty() {
        return Some(ParsedLine::Empty);
    }
    if line == "#EXTM3U" {
        return Some(ParsedLine::ExtM3U);
    }
    if let Some(mtag) = reTagName.captures(line) {
        let tail = get_tail(line, &mtag);
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
    if let Some(_) = reUri.captures(line) {
        return Some(ParsedLine::Uri(line))
    }
    None
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

}