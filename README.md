# m3u_parser

Build for debug:

```
cargo build
cargo test

target/debug/m3u_parser --uri data/master_unenc_hdr10_all.m3u8
target/debug/m3u_parser --uri https://lw.bamgrid.com/2.0/hls/vod/bam/ms02/hls/dplus/bao/master_unenc_hdr10_all.m3u8
```

Build for release:

```
cargo build --release
```

Some basic sorting and filtering methods are implemented:
```
Usage: m3u_parser [OPTIONS] --uri <URI>

Options:
      --uri <URI>
          Filename or http:/https: url to parse
      --audio-group <AUDIO_GROUP>
          Filter by AUDIO-GROUP
      --audio-channels <AUDIO_CHANNELS>
          Filter by AUDIO CHANNELS
      --max-bandwidth <MAX_BANDWIDTH>
          Filter EXT-X-STREAM-INF by bandwidth (maximum specified)
      --resolution <RESOLUTION>
          Filter EXT-X-STREAM-INF and EXT-X-I-FRAME-STREAM-INF by resolution (exact, WxH)
      --sort-by-bandwidth
          Sort EXT-X-STREAM-INF by bandwidth (descending)
  -h, --help
          Print help
  -V, --version
          Print version
```

This models the situation when a player is looking for the best stream having constraints on screen resolution,
codecs, bandwidth etc.
Other constraints can be implemented in a similar way.

Example:
```
cargo run -- --uri data/master_unenc_hdr10_all.m3u8 --audio-group atmos --max-bandwidth 10000000 --sort-by-bandwidth
```
Return streams with AUDIO-GROUP="atmos", limited to 10MBps bandwidth, sorted by bandwidth descending.

```
cargo run -- --uri data/master_unenc_hdr10_all.m3u8 --audio-channels 2 --resolution 640x360 --sort-by-bandwidth
```
Return streams with 2 audio channels and screen resolution 640x360, sorted by bandwidth descending.