# TSN library

![Build status](https://github.com/tsnlab/libtsn/actions/workflows/build.yml/badge.svg)

TSN library(libtsn) is a library for making <abbr title="Time Sensitive Networking">TSN</abbr> application.


## Build (Rust)

To install rust, run `curl -fsS https://sh.rustup.rs | sh`

```sh
cargo build --release  # Release build
cargo build  # Debug build
```

## Run examples

```sh
#Run latency

#Server
sudo ./target/release/latency -s -i <interface>
#Client
sudo ./target/release/latency -c -i <interface> -t <target MAC address>
```

```sh
#Run throughput

#Server
sudo ./target/release/throughput -s -i <interface>
#Client
sudo ./target/release/latency -c -i <interface> -t <target MAC address>
```

## License

The libtsn is distributed under GPLv3 license. See [license](./LICENSE)  
If you need other license than GPLv3 for proprietary use or professional support, please mail us to contact at tsnlab dot com.
