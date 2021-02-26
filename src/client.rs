use redis::{ErrorKind, IntoConnectionInfo, Pipeline};

/// Enables connecting to a redis sentinel and get a connection to the
/// advertised master node in a group.
///
/// see: https://redis.io/topics/sentinel
#[derive(Debug, Clone)]
pub struct SentinelClient {
    sentinel_client: redis::Client,
}

pub struct SentinelConnection {
    connection: redis::Connection,
    client: Option<redis::Client>,
}

/// Helper struct you can use to add more master nodes to a sentinel.
/// This is mostly useful for testing right now.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct SentinelMonitorCmd {
    pub master_name: String,
    pub tcp_host: String,
    pub tcp_port: u16,
    pub quorum: u16,
    pub down_after_milliseconds: u32,
    pub failover_timeout: u32,
    pub parallel_syncs: u8,
}

impl SentinelClient {
    /// Connects to a redis-server in sentinel mode (used in redis clusters) and
    /// return a client pointing to the current master.
    /// This opens a short-lived connection to the sentinel server, then uses the
    /// regular `RedisClient`
    pub fn open<T: redis::IntoConnectionInfo>(params: T) -> redis::RedisResult<Self> {
        let connection_info = params.into_connection_info()?;
        Ok(Self {
            sentinel_client: redis::Client::open(connection_info)?,
        })
    }

    /// Returns a `redis::Connection` to the sentinel itself, e.g. for monitoring or
    /// admin operations
    pub fn get_connection(&self) -> redis::RedisResult<SentinelConnection> {
        Ok(SentinelConnection {
            connection: self.sentinel_client.get_connection()?,
            client: None,
        })
    }

    /// Returns a `redis::aio::Connection` to the sentinel itself, e.g. for monitoring or
    /// admin operations
    pub async fn get_async_connection(&self) -> redis::RedisResult<aio::SentinelConnection> {
        Ok(aio::SentinelConnection::new(
            self.sentinel_client.get_async_connection().await?,
        ))
    }
}

impl SentinelConnection {
    /// Queries the address of the current Redis master node
    ///
    /// see: https://redis.io/topics/sentinel#obtaining-the-address-of-the-current-master
    fn new_client(&mut self, master_group_name: &str) -> redis::RedisResult<redis::Client> {
        let master_addr: (String, u16) = redis::cmd("SENTINEL")
            .arg("get-master-addr-by-name")
            .arg(master_group_name)
            .query(&mut self.connection)?;

        // TODO: allow setting the DB, username amd password
        redis::Client::open(master_addr.into_connection_info()?)
    }

    /// Tells the sentinel to start monitoring a new master with some parameters
    pub fn monitor_master(&mut self, monitor: SentinelMonitorCmd) -> redis::RedisResult<()> {
        let pipeline: Pipeline = monitor.into();
        pipeline.query(&mut self.connection)?;
        Ok(())
    }

    /// Returns a `redis::Connection` that's known to work, or get the advertised
    /// master node from the sentinel and return a new connection to the elected master.
    pub fn get_master_connection(
        &mut self,
        master_group_name: &str,
    ) -> redis::RedisResult<redis::Connection> {
        if let Some(client) = &self.client {
            match client.get_connection() {
                // when we fail here, we try to reconnect to the advertised master node
                Err(e) if e.kind() == ErrorKind::IoError => {
                    let client = self.new_client(master_group_name)?;
                    let connection = client.get_connection()?;
                    self.client.replace(client);
                    Ok(connection)
                }
                r => r,
            }
        } else {
            let client = self.new_client(master_group_name)?;
            let connection = client.get_connection()?;
            self.client.replace(client);
            Ok(connection)
        }
    }

    /// Creates a PubSub instance for this connection.
    pub fn as_pubsub(&mut self) -> redis::PubSub {
        self.connection.as_pubsub()
    }
}

impl Into<redis::Pipeline> for SentinelMonitorCmd {
    fn into(self) -> redis::Pipeline {
        let mut pipeline = redis::pipe();
        pipeline
            .cmd("sentinel")
            .arg("monitor")
            .arg(&self.master_name)
            .arg(self.tcp_host)
            .arg(self.tcp_port)
            .arg(self.quorum)
            .ignore()
            .cmd("sentinel")
            .arg("set")
            .arg(&self.master_name)
            .arg("down-after-milliseconds")
            .arg(self.down_after_milliseconds)
            .ignore()
            .cmd("sentinel")
            .arg("set")
            .arg(&self.master_name)
            .arg("failover-timeout")
            .arg(self.failover_timeout)
            .ignore()
            .cmd("sentinel")
            .arg("set")
            .arg(self.master_name)
            .arg("parallel-syncs")
            .arg(self.parallel_syncs)
            .ignore();
        pipeline
    }
}

#[cfg(feature = "aio")]
pub mod aio {
    use redis::{ErrorKind, IntoConnectionInfo, Pipeline};

    use crate::SentinelMonitorCmd;

    pub struct SentinelConnection {
        connection: redis::aio::Connection,
        client: Option<redis::Client>,
    }

    impl SentinelConnection {
        pub(crate) fn new(connection: redis::aio::Connection) -> Self {
            Self {
                connection,
                client: None,
            }
        }

        /// Queries the address of the current Redis master node
        ///
        /// see: https://redis.io/topics/sentinel#obtaining-the-address-of-the-current-master
        async fn new_client(
            &mut self,
            master_group_name: &str,
        ) -> redis::RedisResult<redis::Client> {
            let master_addr: (String, u16) = redis::cmd("SENTINEL")
                .arg("get-master-addr-by-name")
                .arg(master_group_name)
                .query_async(&mut self.connection)
                .await?;

            // TODO: allow setting the DB, username amd password
            redis::Client::open(master_addr.into_connection_info()?)
        }

        /// Tells the sentinel to start monitoring a new master with some parameters
        pub async fn monitor_master(
            &mut self,
            monitor: SentinelMonitorCmd,
        ) -> redis::RedisResult<()> {
            let pipeline: Pipeline = monitor.into();
            pipeline.query_async(&mut self.connection).await?;
            Ok(())
        }

        /// Return a `redis::aio::Connection` that's known to work, or get the advertised
        /// master node from the sentinel and return a new connection to the elected master.
        pub async fn get_master_connection(
            &mut self,
            master_group_name: &str,
        ) -> Result<redis::aio::Connection, redis::RedisError> {
            if let Some(client) = &self.client {
                match client.get_async_connection().await {
                    // when we fail here, we try to reconnect to the new master node
                    Err(e) if e.kind() == ErrorKind::IoError => {
                        let client = self.new_client(master_group_name).await?;
                        let connection = client.get_async_connection().await?;
                        self.client.replace(client);
                        Ok(connection)
                    }
                    r => r,
                }
            } else {
                let client = self.new_client(master_group_name).await?;
                let connection = client.get_async_connection().await?;
                self.client.replace(client);
                Ok(connection)
            }
        }

        /// Creates a PubSub instance for this connection.
        pub fn into_pubsub(self) -> redis::aio::PubSub {
            self.connection.into_pubsub()
        }
    }
}
