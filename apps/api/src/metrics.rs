use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use serde::Serialize;

#[derive(Debug, Default)]
pub struct MetricsRegistry {
    in_flight_requests: AtomicU64,
    total_requests: AtomicU64,
    error_responses: AtomicU64,
    total_latency_ms: AtomicU64,
    audit_queries: AtomicU64,
    health_checks: AtomicU64,
}

impl MetricsRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn request_started(&self) {
        self.in_flight_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn request_finished(&self, status_code: u16, latency_ms: u64) {
        self.in_flight_requests.fetch_sub(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);
        if status_code >= 500 {
            self.error_responses.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn audit_query(&self) {
        self.audit_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn health_check(&self) {
        self.health_checks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let total_latency_ms = self.total_latency_ms.load(Ordering::Relaxed);

        MetricsSnapshot {
            in_flight_requests: self.in_flight_requests.load(Ordering::Relaxed),
            total_requests,
            error_responses: self.error_responses.load(Ordering::Relaxed),
            audit_queries: self.audit_queries.load(Ordering::Relaxed),
            health_checks: self.health_checks.load(Ordering::Relaxed),
            average_latency_ms: if total_requests == 0 {
                0.0
            } else {
                total_latency_ms as f64 / total_requests as f64
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub in_flight_requests: u64,
    pub total_requests: u64,
    pub error_responses: u64,
    pub audit_queries: u64,
    pub health_checks: u64,
    pub average_latency_ms: f64,
}
