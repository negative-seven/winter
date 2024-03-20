# Winter

A work-in-progress tool which runs Windows programs in a deterministic environment with playback of specified user actions by means of API hooking.

## Usage

Prerequisites:

* Rust toolchain version 1.76.0+ with the targets `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc`
* `cargo` package manager

Run Winter with the following command:

```text
cargo run <executable> [<movie_file>]
```

The optional `movie_file` argument specifies a path to a "movie file", which is a sequence of commands to be sent to a spawned instance of the provided executable. Each line of a movie file must be one of the following commands:

* `Wait <time>`, where `time` is the number of seconds to wait before issuing the next command.

* `Key <code> <state>`, where `code` is the [virtual-key code](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes) of a key to be pressed or released, and `state` is a value of 0 or 1 indicating a key release or key press respectively.
