/// Shared database, mutation gate, and clipboard worker lifecycle.
pub struct AppState {
    pub(crate) pool: SqlitePool,
    pub(crate) gate: Mutex<()>,
    pub(crate) monitor: RwLock<MonitorStatus>,
    pub(crate) shutdown: AtomicBool,
}
