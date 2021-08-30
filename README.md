# Bounce

[![snapcraft](https://snapcraft.io/bounce-blockchain/badge.svg)](https://snapcraft.io/bounce-blockchain)

## Building and Testing

- Install [Rust](https://www.rust-lang.org/)
- `cargo build`
- `cargo test`

## Building Snap Package

Only tested on Ubuntu 20.04.

- Install [Snapcraft](https://snapcraft.io/install/snapcraft/ubuntu)
- `snapcraft`

It will produce file `.snap`, which you can use to install this project on supported Ubuntu systems.

## Binaries

There are two binaries, `flock` and `ground_station`. Simply open two terminals and run each binary in each terminal.

Currently, the `ground_station` binary will send a request to flock and
upon receiving the resposne, it will terminate.

`flock` binary runs indefinitely, so force terminate by using Ctrl-C, and
look at the log folder for logs.

### flock

```sh
$> ./target/debug/flock -h
A flock of Bounce cubesat units 0.1.0
Taegyun Kim <k.taegyun@gmail.com>

USAGE:
    flock [OPTIONS] [log-to-stdout]

FLAGS:
    -h, --help             Prints help information
        --log-to-stdout    By default logs are saved to files, if set log only to stdout.
    -V, --version          Prints version information

OPTIONS:
    -a <ADDRESS>        Specify an alternate address to use. [default: 0.0.0.0]
    -l, --log-dir <LOG_DIR>    Specify a directory to save logs [default: log]
    -p <PORT>           Specify an alternate port to use. [default: 50051]
```

### ground_station

```sh
$> ./target/debug/ground_station -h
Bounce ground station 0.1.0
Taegyun Kim <k.taegyun@gmail.com>

USAGE:
    ground_station [OPTIONS] [log-to-stdout]

FLAGS:
    -h, --help             Prints help information
        --log-to-stdout    By default logs are saved to files, if set log only to stdout.
    -V, --version          Prints version information

OPTIONS:
    -a <ADDRESS>        Specify an alternate address to connect to. [default: 0.0.0.0]
    -l, --log-dir <LOG_DIR>    Specify a directory to save logs [default: log]
    -p <PORT>           Specify an alternate port to connect to. [default: 50051]
```
