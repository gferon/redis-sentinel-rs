mod support;

use futures::StreamExt;
use redis::AsyncCommands;
use redis_sentinel::SentinelMonitorCmd;
use support::{sentinel::Sentinel, server::Server};

/// Spawn a master node and 3 replicas
#[tokio::test]
async fn test_master_failover() -> anyhow::Result<()> {
    let master_server = Server::new()?;
    let _ = master_server.wait_for_startup()?;

    let replica_server = Server::with_config(&format!(
        "replicaof {} {}",
        master_server.tcp_host, master_server.tcp_port
    ))?;
    let _ = replica_server.wait_for_startup()?;

    // we use 3 sentinels
    let mut sentinel_servers = vec![];
    let mut sentinel_clients = vec![];
    let mut sentinel_connections = vec![];
    for _ in 0u8..3 {
        let mut sentinel_server = Sentinel::new()?;
        let sentinel_client = sentinel_server.wait_for_startup()?;
        let mut sentinel_connection = sentinel_client.get_async_connection().await?;

        sentinel_connection
            .monitor_master(SentinelMonitorCmd {
                master_name: "c3p0".to_string(),
                tcp_host: master_server.tcp_host.clone(),
                tcp_port: master_server.tcp_port,
                quorum: 2,
                down_after_milliseconds: 1000,
                failover_timeout: 1000,
                parallel_syncs: 1,
            })
            .await?;

        sentinel_servers.push(sentinel_server);
        sentinel_clients.push(sentinel_client);
        sentinel_connections.push(sentinel_connection);
    }

    // we subscribe to leader election events
    let mut sentinel_pubsub = sentinel_clients[0]
        .get_async_connection()
        .await?
        .into_pubsub();
    sentinel_pubsub.subscribe("+switch-master").await?;
    let mut sentinel_pubsub_stream = sentinel_pubsub.into_on_message();

    // we set a key hello > world
    sentinel_connections[0]
        .get_master_connection("c3p0")
        .await?
        .set("hello", "world")
        .await?;

    // TODO: wait for replication?

    // now we kill the master and see what happens
    drop(master_server);

    // the sentinel told us that the master has changed
    // we want to check that it's the replica that took over!
    let msg = sentinel_pubsub_stream.next().await.unwrap();
    let new_master: String = msg.get_payload()?;
    assert_eq!(
        new_master.split(" ").last().unwrap(),
        replica_server.tcp_port.to_string()
    );

    // without doing anything, we want to check that the connection
    // to the master will be recycled automatically by our sentinel
    let value: String = sentinel_connections[1]
        .get_master_connection("c3p0")
        .await?
        .get("hello")
        .await?;

    assert_eq!(value, "world");

    Ok(())
}
