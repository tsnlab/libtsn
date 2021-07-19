# TSN library

![Build status](https://github.com/tsnlab/libtsn/actions/workflows/build.yml/badge.svg)

TSN library(libtsn) is a library for making <abbr title="Time Sensitive Networking">TSN</abbr> application.


## Build

libtsn has following build dependencies

- [GCC]
- [CMake]
- [Ninja build]

After installing build dependencies, Follow these steps

```sh
cmake -B build -G Ninja .
cmake --build build
```

[GCC]: https://gcc.gnu.org/
[CMake]: https://cmake.org/
[Ninja build]: https://ninja-build.org/

### Running examples

Before running examples, Install runtime dependencies

- Python3.8+

```sh
pip3 install -r requirements.txt
```

```sh
cmake -B build -G Ninja -DBUILD_EXAMPLES=ON .
cmake --build build

# Edit config.yaml and run daemon on both listener, talker side
sudo ./build/tsnucd

# Run latency
# Listener
sudo ./build/examples/latency/latency -s -i devname
# Talker
sudo ./build/examples/latency/latency -c -i devname -t c0:ff:ee:de:ca:ff -C 210

# Run throughput
# Listener
sudo ./build/examples/throughput/throughput -s -i devname
# Talker
sudo ./build/examples/throughput/throughput -c -i devname -t c0:ff:ee:de:ca:ff -T 60
```


## lint

### C lint

```sh
bin/check-clang-format
# Install as git commit hook
cp bin/check-clang-format .git/hooks/pre-commit
```


### Python lint

```sh
pip3 install flake8
flake8
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
