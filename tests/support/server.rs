use std::process;

use backoff::ExponentialBackoff;
use redis::{IntoConnectionInfo, RedisError};
use tempfile::TempDir;

pub struct Server {
    pub process: process::Child,
    _tempdir: TempDir,
    pub tcp_host: String,
    pub tcp_port: u16,
}

impl Server {
    pub fn new() -> anyhow::Result<Server> {
        Self::with_config("")
    }

    pub fn with_config(config: &str) -> anyhow::Result<Server> {
        // get some available TCP port (we mark it as re-usable, Redis starts listening on it, and it's dropped later out of scope)
        let listener = net2::TcpBuilder::new_v4()
            .unwrap()
            .reuse_address(true)
            .unwrap()
            .bind("127.0.0.1:0")
            .unwrap()
            .listen(1)
            .unwrap();
        let redis_port = listener.local_addr().unwrap().port();
        let mut redis_cmd = process::Command::new("redis-server");

        let tempdir = tempfile::Builder::new()
            .prefix("redis-")
            .tempdir()
            .expect("failed to create temp dir");

        let config_file_path = tempdir.path().join("redis.conf");
        std::fs::write(&config_file_path, config)?;

        let cmd = redis_cmd
            .current_dir(&tempdir)
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .arg(config_file_path)
            .arg("--port")
            .arg(redis_port.to_string())
            .arg("--bind")
            .arg("127.0.0.1");

        Ok(Server {
            process: cmd.spawn()?,
            _tempdir: tempdir,
            tcp_host: "127.0.0.1".to_string(),
            tcp_port: redis_port,
        })
    }

    pub fn connection_info(&self) -> anyhow::Result<redis::ConnectionInfo> {
        Ok(format!("redis://{}:{}", self.tcp_host, self.tcp_port).into_connection_info()?)
    }

    pub fn wait_for_startup(&self) -> anyhow::Result<redis::Client> {
        let client = redis::Client::open(self.connection_info()?)?;
        let backoff = ExponentialBackoff::default();
        let retry_connection = || -> Result<(), backoff::Error<RedisError>> {
            client.get_connection().map_err(backoff::Error::Transient)?;
            Ok(())
        };
        backoff::retry(backoff, retry_connection)?;
        Ok(client)
    }

    pub fn stop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop()
    }
}
