#![deny(missing_docs)]

//! redis-sentinel-rs is a plugin for the [Redis rust client library](https://github.com/mitsuhiko/redis-rs)
//! which adds a wrapper around `redis::Client` which first queries a sentinel server.

mod client;

pub use client::ClusterClient;
