use std::{
    fs,
    io::Write,
    process::{self, Stdio},
};

use backoff::ExponentialBackoff;
use redis::IntoConnectionInfo;
use redis_sentinel::SentinelClient;
use tempfile::TempDir;

pub struct Sentinel {
    pub process: process::Child,
    _tempdir: Option<TempDir>,
    tcp_host: String,
    pub tcp_port: u16,
}

impl Sentinel {
    pub fn new() -> anyhow::Result<Sentinel> {
        // this is technically a race but we can't do better with
        // the tools that redis gives us :(
        let listener = net2::TcpBuilder::new_v4()
            .unwrap()
            .reuse_address(true)
            .unwrap()
            .bind("127.0.0.1:0")
            .unwrap()
            .listen(1)
            .unwrap();
        let port = listener.local_addr().unwrap().port();

        let tempdir = tempfile::Builder::new()
            .prefix("redis-sentinel-")
            .tempdir()?;
        let config_file_path = tempdir.path().join("sentinel.conf");

        let mut config_file = fs::File::create(&config_file_path).unwrap();
        writeln!(
            config_file,
            r#"
                daemonize no
                sentinel deny-scripts-reconfig yes
            "#
        )?;

        let process = process::Command::new("redis-server")
            .current_dir(&tempdir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg(config_file_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--bind")
            .arg("127.0.0.1")
            .arg("--sentinel")
            .spawn()?;

        Ok(Sentinel {
            process,
            _tempdir: Some(tempdir),
            tcp_host: "127.0.0.1".into(),
            tcp_port: port,
        })
    }

    pub fn wait_for_startup(&mut self) -> anyhow::Result<SentinelClient> {
        let client = SentinelClient::open(
            format!("redis://{}:{}", self.tcp_host, self.tcp_port).into_connection_info()?,
        )?;
        let backoff = ExponentialBackoff::default();
        let op = || client.get_connection().map_err(backoff::Error::Transient);
        backoff::retry(backoff, op)?;
        Ok(client)
    }
}

impl Drop for Sentinel {
    fn drop(&mut self) {
        self.process.kill().unwrap();
        self.process.wait().unwrap();
    }
}
