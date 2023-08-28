# TSN library

![Build status](https://github.com/tsnlab/libtsn/actions/workflows/build.yml/badge.svg)

TSN library(libtsn) is a library for making <abbr title="Time Sensitive Networking">TSN</abbr> application.


## Build (Rust)

To install rust, run `curl -fsS https://sh.rustup.rs | sh`

```sh
cargo build --release  # Release build
cargo build  # Debug build
```

## Packaging

Use tsnlab debian repository to install libtsn library.

```sh
curl -fsSL https://packages.tsnlab.com/public.gpg | sudo apt-key add -
echo "deb https://packages.tsnlab.com/ unstable main" | sudo tee /etc/apt/sources.list/tsnlab.list
sudo apt update && sudo apt install libtsn
```

## License

The libtsn is distributed under GPLv3 license. See [license](./LICENSE)  
If you need other license than GPLv3 for proprietary use or professional support, please mail us to contact at tsnlab dot com.
