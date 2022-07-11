# TSN library for Rust

## Install Rust
- `curl https://sh.rustup.rs -sSf | sh`

## Build
- `cargo build`

## RUN
### Latency Test
- server RTT:
`sudo ./target/debug/latency server -i <interface_name> [-s <packet_size>]`
- server oneway:
`sudo ./target/debug/latency server --interface <interface_name> [--size <packet_size>] --oneway`
- client RTT:
`sudo ./target/debug/latency client --interface <interface_name> --target <target_mac_address> [--count <send_packet_count>] [--size <packet_size>]`
- client oneway:
`sudo ./target/debug/latency client --interface <interface_name> --target <target_mac_address> [--count <send_packet_count>] [--size <packet_size>] --oneway`
