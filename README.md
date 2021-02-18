# redis-sentinel-rs

Plugin for the Rust Redis client library [redis-rs](https://github.com/mitsuhiko/redis-rs) which adds a
wrapper around `redis::Client` which first queries a sentinel server.

**This is mostly incomplete and work-in-progress, but should work for the most basic use-case.**

Provides only async API and does not handle connection pooling.

Feel free to open an issue or a pull-request!

Links:
- [Redis sentinel documentation](https://redis.io/topics/sentinel)
- [Redis sentinel clients guidelines](https://redis.io/topics/sentinel-clients)
- [redis-rs documentation](https://docs.rs/redis/0.19.0/redis/)

TODO:

- [ ] Add tests
- [ ] Handle more errors if necessary
- [ ] Add ability to connect to a slave node
