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
      --uri <URI>                      Filename or http:/https: url to parse
      --audio-group <AUDIO_GROUP>      Filter by AUDIO-GROUP
      --max-bandwidth <MAX_BANDWIDTH>  Filter EXT-X-STREAM-INF by bandwidth (maximum specified)
      --sort-by-bandwidth              Sort EXT-X-STREAM-INF by bandwidth (descending)
  -h, --help                           Print help
  -V, --version                        Print version
```

This models the situation when a player is looking for the best stream having constraints on audio codec and bandwidth.
Other constraints can be implemented in a similar way.

Example:
```
cargo run -- --uri data/master_unenc_hdr10_all.m3u8 --audio-group atmos --max-bandwidth 10000000 --sort-by-bandwidth
```
will only return streams with AUDIO-GROUP="atmos", limited to 10MBps bandwidth, sorted by bandwidth descending.