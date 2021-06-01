# Bounce

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

There are two binaries, `cubesat-flock` and `ground-station`. Simply open two terminals and run each binary in each terminal.

Currently, the `ground-station` binary will send a request to cubesat-flock and
upon receiving the resposne, it will terminate.

`cubesat-flock` binary runs indefinitely, so force terminate by using Ctrl-C, and
look at the log folder for logs.

### cubesat-flock

```sh
$> ./target/debug/cubesat-flock -h
A flock of Bounce cubesat units 0.1.0
Taegyun Kim <k.taegyun@gmail.com>

USAGE:
    cubesat-flock [OPTIONS] [log_to_stdout]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a <ADDRESS>        Specify an alternate address to use. [default: 0.0.0.0]
    -p <PORT>           Specify an alternate port to use. [default: 50051]

ARGS:
    <log_to_stdout>    By default logs are saved to files, if set log only to stdout.
```

### ground-station

```sh
#> ./target/debug/ground-station -h
Bounce ground station 0.1.0
Taegyun Kim <k.taegyun@gmail.com>

USAGE:
    ground-station [OPTIONS] [log_to_stdout]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a <ADDRESS>        Specify an alternate address to connect to. [default: 0.0.0.0]
    -p <PORT>           Specify an alternate port to connect to. [default: 50051]

ARGS:
    <log_to_stdout>    By default logs are saved to files, if set log only to stdout.
```
