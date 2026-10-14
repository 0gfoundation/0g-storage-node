use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Instant,
};

use jsonrpsee::core::{middleware::Middleware, server::rpc_module::Methods};
use metrics::{register_meter_with_group, Histogram, Meter, Sample};
use parking_lot::Mutex;

struct RpcMetric {
    qps: Arc<dyn Meter>,
    latency: Arc<dyn Histogram>,
}

impl RpcMetric {
    fn new(method_name: &str) -> Self {
        let group = format!("rpc_{}", method_name);

        Self {
            qps: register_meter_with_group(group.as_str(), "qps"),
            latency: Sample::ExpDecay(0.015).register_with_group(group.as_str(), "latency", 1024),
        }
    }

    /// Returns the metrics of the given method, registering them only once per process, since
    /// the metrics registry is global and panics on duplicate registration.
    fn get_or_register(method_name: &'static str) -> Arc<Self> {
        static METRICS_BY_METHOD: Mutex<BTreeMap<&str, Arc<RpcMetric>>> =
            Mutex::new(BTreeMap::new());

        METRICS_BY_METHOD
            .lock()
            .entry(method_name)
            .or_insert_with(|| Arc::new(Self::new(method_name)))
            .clone()
    }
}

/// Records metrics of the methods served by an RPC server, and ignores calls of other methods,
/// so that clients cannot register metrics with arbitrary method names.
#[derive(Clone)]
pub struct Metrics {
    metrics_by_method: Arc<HashMap<&'static str, Arc<RpcMetric>>>,
}

impl Metrics {
    /// Must be called after `metrics::initialize`. Metrics registered before it are no-ops, and
    /// `get_or_register` would keep returning them for the rest of the process.
    pub fn new(methods: &Methods) -> Self {
        let metrics_by_method = methods
            .method_names()
            .map(|name| (name, RpcMetric::get_or_register(name)))
            .collect();

        Self {
            metrics_by_method: Arc::new(metrics_by_method),
        }
    }
}

impl Middleware for Metrics {
    type Instant = Instant;

    fn on_request(&self) -> Self::Instant {
        Instant::now()
    }

    fn on_call(&self, name: &str) {
        if let Some(metric) = self.metrics_by_method.get(name) {
            metric.qps.mark(1);
        }
    }

    fn on_result(&self, name: &str, _success: bool, started_at: Self::Instant) {
        if let Some(metric) = self.metrics_by_method.get(name) {
            metric.latency.update_since(started_at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Metrics;
    use jsonrpsee::{core::middleware::Middleware, RpcModule};
    use metrics::{MetricsConfiguration, DEFAULT_GROUPING_REGISTRY};

    fn enable_metrics() {
        metrics::initialize(MetricsConfiguration {
            enabled: true,
            ..Default::default()
        });
    }

    fn server_metrics(method_names: &[&'static str]) -> Metrics {
        let mut module = RpcModule::new(());
        for name in method_names {
            module.register_method(name, |_, _| Ok(())).unwrap();
        }
        Metrics::new(&module)
    }

    fn call(metrics: &Metrics, method_name: &str) {
        let started_at = metrics.on_request();
        metrics.on_call(method_name);
        metrics.on_result(method_name, true, started_at);
    }

    /// Returns the metrics registered for the given method with their counts, e.g. "qps: 1".
    fn registered(method_name: &str) -> Vec<String> {
        let registry = DEFAULT_GROUPING_REGISTRY.read();
        let mut metrics: Vec<String> = registry
            .get_all()
            .get(&format!("rpc_{}", method_name))
            .into_iter()
            .flatten()
            .map(|(name, metric)| {
                // formatted as "{count: 1, ...}"
                let value = metric.get_value();
                let count = value.trim_start_matches("{count: ").split(',').next();
                format!("{}: {}", name, count.unwrap())
            })
            .collect();
        metrics.sort();
        metrics
    }

    #[test]
    fn test_same_method_on_public_and_admin_servers() {
        enable_metrics();

        // As in run_server_public_private, where each server serves its own methods.
        let public = server_metrics(&["zgs_testA", "zgs_testB"]);
        let admin = server_metrics(&["admin_testA", "admin_testB"]);

        // Clients can send any method name to either server, in any order.
        for name in ["zgs_testA", "admin_testA", "eth_testA"] {
            call(&public, name);
            call(&admin, name);
        }
        for name in ["zgs_testB", "admin_testB", "eth_testB"] {
            call(&admin, name);
            call(&public, name);
        }

        // Only the server serving a method records its calls.
        for name in ["zgs_testA", "zgs_testB", "admin_testA", "admin_testB"] {
            assert_eq!(registered(name), ["latency: 1", "qps: 1"], "{}", name);
        }
        for name in ["eth_testA", "eth_testB"] {
            assert!(registered(name).is_empty(), "{}", name);
        }
    }

    #[test]
    fn test_unknown_methods_not_registered() {
        enable_metrics();

        let metrics = server_metrics(&["zgs_testServed"]);
        for i in 0..100 {
            call(&metrics, &format!("zgs_testUnknown{}", i));
        }
        call(&metrics, "zgs_testServed");

        let unknown_groups = DEFAULT_GROUPING_REGISTRY
            .read()
            .get_all()
            .keys()
            .filter(|group| group.starts_with("rpc_zgs_testUnknown"))
            .count();
        assert_eq!(unknown_groups, 0);
        assert_eq!(registered("zgs_testServed"), ["latency: 1", "qps: 1"]);
    }

    #[test]
    fn test_servers_sharing_methods_concurrently() {
        enable_metrics();

        // Servers serving the same method share its metrics, even if built concurrently.
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| call(&server_metrics(&["zgs_testShared"]), "zgs_testShared"));
            }
        });

        assert_eq!(registered("zgs_testShared"), ["latency: 8", "qps: 8"]);
    }
}
