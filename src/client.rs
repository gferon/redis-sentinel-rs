use log::trace;
use redis::{ErrorKind, IntoConnectionInfo};

/// Enables connecting to a cluster of redis instance configured
/// in high-availability mode and monitored by sentinel
///
/// see: https://redis.io/topics/sentinel
#[derive(Debug, Clone)]
pub struct ClusterClient {
    /// connects to any available sentinel and fetches the IP of the master node
    sentinel_client: redis::Client,
    client: redis::Client,
    master_group_name: String,
    namespace: String,
}

impl ClusterClient {
    /// Connects to a redis-server in sentinel mode (used in redis clusters) and
    /// return a client pointing to the current master.
    /// This opens a short-lived connection to the sentinal server, then uses the
    /// regular `RedisClient`
    pub fn try_open_with_sentinel<T: redis::IntoConnectionInfo>(
        master_group_name: &str,
        params: T,
        namespace: &str,
    ) -> redis::RedisResult<Self> {
        let connection_info = params.into_connection_info()?;
        let mut sentinel_client = redis::Client::open(connection_info)?;
        let master_addr = Self::get_master_addr(&master_group_name, &mut sentinel_client)?;
        Ok(Self {
            master_group_name: master_group_name.to_owned(),
            sentinel_client,
            client: redis::Client::open(master_addr)?,
            namespace: namespace.into(),
        })
    }

    /// Queries the address of the current Redis master node
    ///
    /// see: https://redis.io/topics/sentinel#obtaining-the-address-of-the-current-master
    pub fn get_master_addr(
        master_group_name: &str,
        sentinel_client: &redis::Client,
    ) -> redis::RedisResult<redis::ConnectionInfo> {
        let mut sentinel_conn = sentinel_client.get_connection()?;
        
        let (master_addr, master_port): (String, u16) = redis::cmd("SENTINEL")
            .arg("get-master-addr-by-name")
            .arg(master_group_name)
            .query(&mut sentinel_conn)?;
        let master_addr = format!("redis://{}:{}", master_addr, master_port);
        trace!("got redis addr from sentinel: {}", master_addr);
        master_addr.into_connection_info()
    }

    /// Returns the current `redis::Connection` or tries to reconnect to the advertised
    /// master node.
    pub fn get_connection(&self) -> redis::RedisResult<redis::Connection> {
        match self.client.get_connection() {
            // when we fail here, we try to reconnect to the new master node
            Err(e) if e.kind() == ErrorKind::IoError => {
                let master_addr =
                    Self::get_master_addr(&self.master_group_name, &self.sentinel_client)?;
                redis::Client::open(master_addr)?.get_connection()
            }
            r => r,
        }
    }

    /// Returns the current `redis::aio::Connection` or tries to reconnected to
    /// the advertised master node.
    pub async fn get_connection_async(
        &self,
    ) -> Result<redis::aio::Connection, redis::RedisError> {
        match self.client.get_async_connection().await {
            // when we fail here, we try to reconnect
            Err(e) if e.kind() == ErrorKind::IoError => {
                let master_addr =
                    Self::get_master_addr(&self.master_group_name, &self.sentinel_client)?;
                redis::Client::open(master_addr)?
                    .get_async_connection()
                    .await
            }
            r => r,
        }
    }
}
