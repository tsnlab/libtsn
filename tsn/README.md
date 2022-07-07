# TSN library for Rust

## Install Rust
- `curl https://sh.rustup.rs -sSf | sh`

## Build
- `cargo build`

## RUN
### Latency Test
- server RTT:
`sudo ./target/debug/latency --no-verbose --interface <interface_name> --mode s --size <packet_size> --no-oneway`
- server oneway:
`sudo ./target/debug/latency --no-verbose --interface <interface_name> --mode s --size <packet_size> --oneway`
- client RTT:
`sudo ./target/debug/latency --no-verbose --interface <interface_name> --mode c --target <target_mac_address> --count <send_packet_count> --size <packet_size> --no-precise --no-oneway`
- client oneway:
`sudo ./target/debug/latency --no-verbose --interface <interface_name> --mode c --target <target_mac_address> --count <send_packet_count> --size <packet_size> --no-precise --oneway`
