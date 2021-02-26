# redis-sentinel-rs

![Build Status](https://github.com/gferon/redis-sentinel-rs/workflows/CI/badge.svg)

Plugin for the [Redis rust client library](https://github.com/mitsuhiko/redis-rs) which adds a
wrapper around `redis::Client` which first queries a sentinel server.

**This is mostly incomplete and work-in-progress, but should work for the most basic use-case.**

Feel free to open an issue or a pull-request!

Links:

- [Redis sentinel documentation](https://redis.io/topics/sentinel)
- [redis-rs documentation](https://docs.rs/redis/0.19.0/redis/)
