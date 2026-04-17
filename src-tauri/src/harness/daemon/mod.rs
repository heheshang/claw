//! Background task daemon - inspired by zeroclaw's daemon module
//!
//! Provides resilient background task execution with:
//! - Automatic restart with backoff on failure
//! - Health monitoring and status tracking
//! - Signal handling (ignores SIGHUP, responds to SIGINT/SIGTERM)

use log::{error, info, warn};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Minimum interval for status flush
#[allow(dead_code)]
const STATUS_FLUSH_SECONDS: u64 = 5;

/// Component health status
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub last_error: Option<String>,
    pub restart_count: u64,
    pub last_restart_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Error,
    Starting,
}

impl Default for ComponentHealth {
    fn default() -> Self {
        Self {
            name: String::new(),
            status: HealthStatus::Starting,
            last_error: None,
            restart_count: 0,
            last_restart_at: None,
        }
    }
}

/// Shared daemon state
pub struct DaemonState {
    pub components: RwLock<Vec<ComponentHealth>>,
    pub shutdown_requested: Arc<AtomicU64>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            components: RwLock::new(Vec::new()),
            shutdown_requested: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn mark_ok(&self, name: &str) {
        let mut components = self.components.write().await;
        if let Some(c) = components.iter_mut().find(|c| c.name == name) {
            c.status = HealthStatus::Ok;
            c.last_error = None;
        } else {
            components.push(ComponentHealth {
                name: name.to_string(),
                status: HealthStatus::Ok,
                last_error: None,
                restart_count: 0,
                last_restart_at: None,
            });
        }
    }

    pub async fn mark_error(&self, name: &str, error: &str) {
        let mut components = self.components.write().await;
        if let Some(c) = components.iter_mut().find(|c| c.name == name) {
            c.status = HealthStatus::Error;
            c.last_error = Some(error.to_string());
        }
    }

    pub async fn bump_restart(&self, name: &str) {
        let mut components = self.components.write().await;
        if let Some(c) = components.iter_mut().find(|c| c.name == name) {
            c.restart_count += 1;
            c.last_restart_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    pub async fn snapshot(&self) -> Vec<ComponentHealth> {
        self.components.read().await.clone()
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

/// Wait for shutdown signal (SIGINT or SIGTERM).
/// SIGHUP is explicitly ignored so the daemon survives terminal/SSH disconnects.
pub async fn wait_for_shutdown_signal(shutdown_flag: Arc<AtomicU64>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
        let mut sighup = signal(SignalKind::hangup()).expect("Failed to register SIGHUP handler");

        tokio::select! {
            _ = sigint.recv() => {
                info!("[DAEMON] Received SIGINT, shutting down...");
                shutdown_flag.fetch_add(1, Ordering::SeqCst);
            }
            _ = sigterm.recv() => {
                info!("[DAEMON] Received SIGTERM, shutting down...");
                shutdown_flag.fetch_add(1, Ordering::SeqCst);
            }
            _ = sighup.recv() => {
                info!("[DAEMON] Received SIGHUP, ignoring (daemon stays running)");
            }
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.expect("Failed to register Ctrl+C handler");
        info!("[DAEMON] Received Ctrl+C, shutting down...");
        shutdown_flag.fetch_add(1, Ordering::SeqCst);
    }
}

/// Spawn a component supervisor that will automatically restart on failure with backoff.
pub fn spawn_component_supervisor<F, Fut>(
    name: &'static str,
    daemon_state: Arc<DaemonState>,
    initial_backoff_secs: u64,
    max_backoff_secs: u64,
    mut run_component: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    tokio::spawn(async move {
        let mut backoff = initial_backoff_secs.max(1);
        let max_backoff = max_backoff_secs.max(initial_backoff_secs);

        loop {
            // Check shutdown flag
            if daemon_state.shutdown_requested.load(Ordering::SeqCst) > 0 {
                info!("[DAEMON] Component '{}' shutting down due to shutdown signal", name);
                break;
            }

            daemon_state.mark_ok(name).await;

            match run_component().await {
                Ok(()) => {
                    daemon_state.mark_error(name, "component exited unexpectedly").await;
                    warn!("[DAEMON] Component '{}' exited unexpectedly", name);
                    // Reset backoff since the component ran successfully
                    backoff = initial_backoff_secs.max(1);
                }
                Err(e) => {
                    daemon_state.mark_error(name, &e).await;
                    error!("[DAEMON] Component '{}' failed: {}", name, e);
                }
            }

            daemon_state.bump_restart(name).await;

            // Wait before restarting with exponential backoff
            tokio::time::sleep(tokio::time::Duration::from_secs(backoff)).await;

            // Double backoff AFTER sleeping so first error uses initial_backoff
            backoff = backoff.saturating_mul(2).min(max_backoff);
        }
    })
}

/// Spawn a simple background task that runs until shutdown.
pub fn spawn_background_task<F, Fut>(name: &'static str, daemon_state: Arc<DaemonState>, future: F) -> JoinHandle<()>
where
    F: Future<Output = Result<(), String>> + Send + 'static,
{
    tokio::spawn(async move {
        daemon_state.mark_ok(name).await;

        if let Err(e) = future.await {
            daemon_state.mark_error(name, &e).await;
            error!("[DAEMON] Background task '{}' failed: {}", name, e);
        }

        daemon_state.mark_ok(name).await;
    })
}

/// Spawn a periodic task that runs at fixed intervals.
pub fn spawn_periodic_task<F, Fut>(
    name: &'static str,
    daemon_state: Arc<DaemonState>,
    interval_secs: u64,
    mut task: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            if daemon_state.shutdown_requested.load(Ordering::SeqCst) > 0 {
                info!("[DAEMON] Periodic task '{}' shutting down", name);
                break;
            }

            interval.tick().await;
            daemon_state.mark_ok(name).await;

            if let Err(e) = task().await {
                daemon_state.mark_error(name, &e).await;
                warn!("[DAEMON] Periodic task '{}' iteration failed: {}", name, e);
            }
        }
    })
}
