# Substrate Monitor tool Prometheus

## 소개

프로메테우스는, 다른 체인의 생태계에서 매우 유용하게 사용되고있다. 코어에 포함 되게 되면 좀 더 세밀한 metrics 구현이 가능하다. 여러가지 기능들이
있는데, 주로 노드헬스, 체인헬스에 대한 타임라인관리, 실시간알람, 그리고 그라파나를 이용한 시각화를 통해 노드를 철저하게 관리 할 수있다.

## 목차

Substrate Dev hack
 - Prometheus starter
 - CLI Config
 - Metrics Add

Metrics
 - List of available metrics

Start Prometheus
 - Install prometheus
 - Edit Prometheus config file
 - Start Prometheus

Start Grafana
 - Install Grafana

## Substrate Dev hack
### Prometheus starter

client/prometheus/src/lib.rs
```rust

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
use hyper::http::StatusCode;
use hyper::rt::Future;
use hyper::service::service_fn_ok;
use hyper::{Body, Request, Response, Server};
use prometheus::{ Encoder,  TextEncoder,Opts};
use std::{net::{ SocketAddr}};
pub use sr_primitives::traits::SaturatedConversion;
pub use prometheus::{Histogram, IntCounter, IntGauge, Result};

pub mod metrics;


/// Initializes the metrics context, and starts an HTTP server
/// to serve metrics.
pub fn init_prometheus(prometheus_addr: SocketAddr) {
    let addr = prometheus_addr;
    let server = Server::bind(&addr)
        .serve(|| {
            // This is the `Service` that will handle the connection.
            // `service_fn_ok` is a helper to convert a function that
            // returns a Response into a `Service`.
            service_fn_ok(move |req: Request<Body>| {
                
                if req.uri().path() == "/metrics" {
                    let metric_families = prometheus::gather();
                    let mut buffer = vec![];
                    let encoder = TextEncoder::new();
                    encoder.encode(&metric_families, &mut buffer).unwrap();
                
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", encoder.format_type())
                        .body(Body::from(buffer))
                        .expect("Sends OK(200) response with one or more data metrics")
                } else {
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Body::from("Not found."))
                        .expect("Sends NOT_FOUND(404) message with no data metric")
                }
            })
        })
        .map_err(|e| error!("server error: {}", e));

    info!("Exporting metrics at http://{}/metrics", addr);

    let mut rt = tokio::runtime::Builder::new()
        .core_threads(1) // one thread is sufficient
        .build()
        .expect("Builds one thread of tokio runtime exporter for prometheus");

    std::thread::spawn(move || {
        rt.spawn(server);
        rt.shutdown_on_idle().wait().unwrap();
    });
}

```

client/prometheus/Cargo.toml
```toml
[package]
name = "substrate-prometheus"
version = "2.0.0"
authors = ["Parity Technologies <admin@parity.io>"]
description = "prometheus utils"
edition = "2018"

[dependencies]
hyper = "0.12"
lazy_static = "1.0"
log = "0.4"
prometheus = { version = "0.7", features = ["nightly", "process"]}
tokio = "0.1"

[dev-dependencies]
reqwest = "0.9"
```
client/service/Cargo.toml
```toml
....
promet = { package = "substrate-prometheus", path="../../core/prometheus"}
....
```
client/service/src/builder.rs
```rust

				.....
				let _ = to_spawn_tx.unbounded_send(Box::new(future
				.select(exit.clone())
				.then(|_| Ok(()))));
			telemetry
		});
----------------
match config.prometheus_endpoint {
			None => (), 
			Some(x) => {let _prometheus = promet::init_prometheus(x);}	
		}
		
-------------------		
		Ok(NewService {
			client,
			network,
				.....
```
substrate/Cargo.toml
```toml
        ....
[workspace]
members = [
	"client/prometheus",
        ....
```
### CLI Config
client/cli/src/lib.rs
```rust
fn crate_run_node_config
...
}

	match cli.prometheus_endpoint {
		None => {config.prometheus_endpoint = None;},
		Some(x) => {
			config.prometheus_endpoint = Some(parse_address(&format!("{}:{}", x, 33333), cli.prometheus_port)?);
			}
	}
...
```

client/cli/src/params.rs
```rust
pub struct RunCmd{
/// Specify HTTP RPC server TCP port.
#[structopt(long = "prometheus-port", value_name = "PORT")]
	pub prometheus_port: Option<u16>,
#[structopt(long = "prometheus-addr", value_name = "Local IP address")]
	pub prometheus_endpoint: Option<String>,
}
```
client/service/src/config.rs
```rust
#[derive(Clone)]
pub struct Configuration<C, G, E = NoExtension> {
    ...
    pub prometheus_endpoint: Option<SocketAddr>,
    ...
}
impl<C, G, E> Configuration<C, G, E> where
	C: Default,
	G: RuntimeGenesis,
	E: Extension,
{
	/// Create default config for given chain spec.
	pub fn default_with_spec(chain_spec: ChainSpec<G, E>) -> Self {
		let mut configuration = Configuration {
            ...
            prometheus_endpoints: None,
            ...
		};
		configuration.network.boot_nodes = configuration.chain_spec.boot_nodes().to_vec();

		configuration.telemetry_endpoints = configuration.chain_spec.telemetry_endpoints().clone();

		configuration
	}
```



### Metrics Add 
ex) consensus_FINALITY_HEIGHT

client/prometheus/src/metrics.rs

```rust
pub use crate::*;

pub fn try_create_int_gauge(name: &str, help: &str) -> Result<IntGauge> {
    let opts = Opts::new(name, help);
    let gauge = IntGauge::with_opts(opts)?;
    prometheus::register(Box::new(gauge.clone()))?;
    Ok(gauge)
}

pub fn set_gauge(gauge: &Result<IntGauge>, value: u64) {
    if let Ok(gauge) = gauge {
        gauge.set(value as i64);
    }
}

lazy_static! {
    pub static ref FINALITY_HEIGHT: Result<IntGauge> = try_create_int_gauge(
        "consensus_finality_block_height_number",
        "block is finality HEIGHT"

    );
}
```
client/service/Cargo.toml
```rust
...
promet = { package = "substrate-prometheus", path="../prometheus"}
...
```
client/service/src/builder.rs
```rust
.....
use promet::{metrics};
.....
		let tel_task = state_rx.for_each(move |(net_status, _)| {
			let info = client_.info();
			let best_number = info.chain.best_number.saturated_into::<u64>();
			let best_hash = info.chain.best_hash;
			let num_peers = net_status.num_connected_peers;
			let txpool_status = transaction_pool_.status();
			let finalized_number: u64 = info.chain.finalized_number.saturated_into::<u64>();
			let bandwidth_download = net_status.average_download_per_sec;
			let bandwidth_upload = net_status.average_upload_per_sec;

			let used_state_cache_size = match info.used_state_cache_size {
				Some(size) => size,
				None => 0,
			};

			// get cpu usage and memory usage of this process
			let (cpu_usage, memory) = if let Some(self_pid) = self_pid {
				if sys.refresh_process(self_pid) {
					let proc = sys.get_process(self_pid)
						.expect("Above refresh_process succeeds, this should be Some(), qed");
					(proc.cpu_usage(), proc.memory())
				} else { (0.0, 0) }
			} else { (0.0, 0) };

			telemetry!(
				SUBSTRATE_INFO;
				"system.interval";
				"peers" => num_peers,
				"height" => best_number,
				"best" => ?best_hash,
				"txcount" => txpool_status.ready,
				"cpu" => cpu_usage,
				"memory" => memory,
				"finalized_height" => finalized_number,
				"finalized_hash" => ?info.chain.finalized_hash,
				"bandwidth_download" => bandwidth_download,
				"bandwidth_upload" => bandwidth_upload,
				"used_state_cache_size" => used_state_cache_size,
			);
            metrics::set_gauge(&metrics::FINALITY_HEIGHT, finalized_number as u64);
.....
```
## Metrics

substrate can report and serve the Prometheus metrics, which in their turn can be consumed by Prometheus collector(s).

This functionality is disabled by default.

To enable the Prometheus metrics, set in your cli command (--prometheus-addr,--prometheus-port ). 
Metrics will be served under /metrics on 33333 port by default.

### List of available metrics


Consensus metrics, namespace: `substrate`

| **Name**                                | **Type**  | **Tags** | **Description**                                                 |
|-----------------------------------------|-----------|----------|-----------------------------------------------------------------|
| consensus_FINALITY_HEIGHT               | IntGauge  |          | finality Height of the chain                                    |
| consensus_BEST_HEIGHT                   | IntGauge  |          | best Height of the chain                                        |
| consensus_TARGET_HEIGHT                 | IntGauge  |          | syning Height target number                                     |
| consensus_validators_power(Add in the future)              | IntGauge  |          | Total voting power of all validators                            |
| consensus_missing_validators(Add in the future)            | Gauge     |          | Number of validators who did not sign                           |
| consensus_missing_validators_power(Add in the future)      | Gauge     |          | Total voting power of the missing validators                    |
| consensus_byzantine_validators(Add in the future)          | Gauge     |          | Number of validators who tried to double sign                   |
| consensus_byzantine_validators_power(Add in the future)    | Gauge     |          | Total voting power of the byzantine validators                  |
| consensus_block_interval_seconds(Add in the future)        | Histogram |          | Time between this and last block (Block.Header.Time) in seconds |
| consensus_rounds(Add in the future)                        | Gauge     |          | Number of rounds                                                |
| consensus_num_txs(Add in the future)                       | Gauge     |          | Number of transactions                                          |
| consensus_block_parts(Add in the future)                   | counter   |          | number of blockparts transmitted by peer                        |
| consensus_total_txs(Add in the future)                     | Gauge     |          | Total number of transactions committed                          |
| consensus_block_size_bytes(Add in the future)              | Gauge     |          | Block size in bytes                                             |
| p2p_peers_number                        | IntGauge  |          | Number of peers node's connected to                             |
| p2p_peer_receive_bytes_total            | IntGauge  |          | number of bytes received from a given peer                      |
| p2p_peer_send_bytes_total               | IntGauge  |          | number of bytes sent to a given peer                            |
| mempool_size(Add in the future)                            | Gauge     |          | Number of uncommitted transactions                              |
| mempool_tx_size_bytes(Add in the future)                   | histogram |          | transaction sizes in bytes                                      |
| mempool_failed_txs(Add in the future)                      | counter   |          | number of failed transactions                                   |
| mempool_recheck_times(Add in the future)                   | counter   |          | number of transactions rechecked in the mempool                 |
| state_block_processing_time(Add in the future)             | histogram |          | time between BeginBlock and EndBlock in ms                      |
| state_recheck_time(Add in the future)                      | histogram |          | time cost on recheck in ms                                      |
| state_app_hash_conflict(Add in the future)                 | count     |          | App hash conflict error                                         |



## Start Prometheus
### Install prometheus

https://prometheus.io/download/
```bash
wget <download URL>
tar -zxvf <prometheus tar file>
```

### Edit Prometheus config file

You can visit [prometheus.yml](https://github.com/prometheus/prometheus/blob/master/documentation/examples/prometheus.yml) to download default `prometheus.yml`.

Then edit `prometheus.yml` and add `jobs` :

```yaml
      - job_name: kusama
          static_configs:
          - targets: ['localhost:33333']
            labels:
              instance: local-validator
```

> Note：value of targets is ip:port which used by substrate monitor 

### Start Prometheus

```bash
cd <prometheus file>
./prometheus
```

> The above example, you can save `prometheus.yml` at `~/volumes/prometheus` on your host machine

You can visit `http://localhost:9090` to see prometheus data.



## Start Grafana
### Install Grafana
https://grafana.com/docs/installation/debian/

```bash
apt-get install -y software-properties-common
sudo add-apt-repository "deb https://packages.grafana.com/oss/deb stable main"
wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -
sudo apt-get update
sudo apt-get install grafana
sudo service grafana-server start
./prometheus
```

You can visit `http://localhost:3000/` to open grafana and create your own dashboard.

> Tips: The default username and password are both admin. We strongly recommend immediately changing your username & password after login

### Seting Grafana

Default ID:PW is admin.