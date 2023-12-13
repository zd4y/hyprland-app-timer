use chrono::{DateTime, Local, Utc};
use tokio::runtime::Runtime;

#[derive(Debug)]
pub struct BlockingClient {
    inner: crate::Client,
    rt: Runtime,
}

impl BlockingClient {
    pub fn new() -> anyhow::Result<BlockingClient> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let inner = rt.block_on(crate::Client::new())?;
        Ok(BlockingClient { inner, rt })
    }

    pub fn get_apps_usage(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<crate::AppUsage>, sqlx::Error> {
        self.rt.block_on(self.inner.get_apps_usage(from, to))
    }

    pub async fn get_daily_app_usage(
        &self,
        app: &str,
        from: DateTime<Local>,
        to: DateTime<Local>,
    ) -> anyhow::Result<Vec<crate::AppUsageDay>> {
        self.rt
            .block_on(self.inner.get_daily_app_usage(app, from, to))
    }

    pub async fn get_app_windows_between(
        &self,
        app: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<crate::Window>, sqlx::Error> {
        self.rt
            .block_on(self.inner.get_app_windows_between(app, from, to))
    }
}
