//! Singleton settings repository.
use crate::{error::AppError, models::AppSettings};
use sqlx::SqlitePool;
