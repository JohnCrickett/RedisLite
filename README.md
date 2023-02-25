# RedisLite
This is a very lite version of a [Redis](https://redis.io/) server built in [Rust](https://www.rust-lang.org/) with [Tokio](https://tokio.rs/).

The intent of the project is to build a real-world application as part of learning Rust.

## Why Redis
learning Redis and learning Rust have both been on my todo list for a while so it seemed like a good idea to combine the two.

## Running
The Redis server can be run using:
```bash
cargo run
```

## Supported Commands
The following commands are supported:
* GET
* SET
* PING
* ECHO

There is no support for persistence.