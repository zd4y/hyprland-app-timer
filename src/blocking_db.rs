use chrono::{DateTime, Local, Utc};
use tokio::runtime::Runtime;

use crate::{AppUsage, AppUsageDay, Window};

#[derive(Debug)]
pub struct BlockingSqliteDB {
    inner: crate::SqliteDB,
    rt: Runtime,
}

impl BlockingSqliteDB {
    pub fn new() -> anyhow::Result<BlockingSqliteDB> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let inner = rt.block_on(crate::SqliteDB::new())?;
        Ok(BlockingSqliteDB { inner, rt })
    }

    pub fn get_apps_usage(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AppUsage>, sqlx::Error> {
        self.rt.block_on(self.inner.get_apps_usage(from, to))
    }

    pub fn get_daily_app_usage(
        &self,
        app: &str,
        from: DateTime<Local>,
        to: DateTime<Local>,
    ) -> anyhow::Result<Vec<AppUsageDay>> {
        self.rt
            .block_on(self.inner.get_daily_app_usage(app, from, to))
    }

    pub fn get_app_windows_between(
        &self,
        app: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Window>, sqlx::Error> {
        self.rt
            .block_on(self.inner.get_app_windows_between(app, from, to))
    }
}
