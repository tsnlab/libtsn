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

```sh
cmake -B build -G Ninja -DBUILD_EXAMPLES=ON .
cmake --build build
./build/examples/latency/latency
./build/examples/throughput/throughput
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

TBA


## License

The libtsn is distributed under GPLv3 license. See [license](./LICENSE)
