# m3u_parser

Build for debug:

```
cargo build
cargo test

target/debug/m3u_parser data/master_unenc_hdr10_all.m3u8
target/debug/m3u_parser \
    https://lw.bamgrid.com/2.0/hls/vod/bam/ms02/hls/dplus/bao/master_unenc_hdr10_all.m3u8
```

Build for release:

```
cargo build --release

target/release/m3u_parser data/master_unenc_hdr10_all.m3u8
target/release/m3u_parser \
    https://lw.bamgrid.com/2.0/hls/vod/bam/ms02/hls/dplus/bao/master_unenc_hdr10_all.m3u8
```