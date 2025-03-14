use std::{str::FromStr, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use sqlx::{
    prelude::FromRow,
    sqlite::{SqliteConnectOptions, SqliteRow},
    ConnectOptions, Row, SqlitePool,
};

#[cfg(feature = "db")]
#[derive(Debug)]
pub struct Window {
    pub datetime: DateTime<Utc>,
    pub title: String,
    pub class: String,
    pub duration: Duration,
}

#[cfg(feature = "db")]
#[derive(Debug)]
pub struct AppUsage {
    pub app: String,
    pub duration: Duration,
}

#[cfg(feature = "db")]
#[derive(Debug)]
pub struct AppUsageDay {
    pub date_local: NaiveDate,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct SqliteDB {
    pool: SqlitePool,
}

impl SqliteDB {
    pub async fn new() -> anyhow::Result<SqliteDB> {
        let database_url = SqliteDB::get_database_url()?;
        let mut options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
        options.disable_statement_logging();
        let pool = SqlitePool::connect_with(options).await?;
        log::debug!("Connected to sqlite pool {}", database_url);
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(SqliteDB { pool })
    }

    pub async fn get_apps_usage(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AppUsage>, sqlx::Error> {
        let from = from.to_rfc3339();
        let to = to.to_rfc3339();
        let db_apps = sqlx::query_as::<_, AppUsage>(
            r#"
            SELECT class, SUM(duration) as duration
            FROM windows_log WHERE datetime >= ?1 AND datetime < ?2
            GROUP BY class
            ORDER BY duration
            DESC
        "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        Ok(db_apps)
    }

    pub async fn get_daily_app_usage(
        &self,
        app: &str,
        from: DateTime<Local>,
        to: DateTime<Local>,
    ) -> anyhow::Result<Vec<AppUsageDay>> {
        let from_utc = to_utc(from).to_rfc3339();
        let to_utc = to_utc(to).to_rfc3339();
        let offset_seconds = from.offset().local_minus_utc();
        let db_windows = sqlx::query_as::<_, AppUsageDay>(
            &format!(
            r#"
            SELECT strftime('%Y-%m-%d', DATETIME(datetime, '{} seconds')) as date_local, SUM(duration) as duration
            FROM windows_log
            WHERE datetime >= ?1 AND datetime < ?2 AND class = ?3
            GROUP BY date_local
            ORDER BY date_local
            ASC
        "# , offset_seconds))
        .bind(from_utc)
        .bind(to_utc)
        .bind(app)
        .fetch_all(&self.pool)
        .await?;
        Ok(db_windows)
    }

    pub async fn get_app_windows_between(
        &self,
        app: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Window>, sqlx::Error> {
        let from = from.to_rfc3339();
        let to = to.to_rfc3339();
        let db_apps = sqlx::query_as::<_, Window>(
            r#"
                SELECT datetime, title, class, duration
                FROM windows_log WHERE datetime >= ?1 AND datetime < ?2 AND class = ?3
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(app)
        .fetch_all(&self.pool)
        .await?;
        Ok(db_apps)
    }

    #[cfg(feature = "server")]
    pub(crate) async fn save_windows(&self, windows: &[Window]) -> anyhow::Result<()> {
        if windows.is_empty() {
            return Ok(());
        }

        let mut to_save = Vec::new();

        for window in windows {
            let datetime = window.datetime.to_rfc3339();
            let duration = window.duration.as_secs_f64();
            to_save.push((datetime, &window.class, &window.title, duration));
        }

        let mut query =
            String::from("INSERT INTO windows_log (datetime, class, title, duration) VALUES");

        for (index, _) in to_save.iter().enumerate() {
            let i = index * 4;
            query += &format!("\n(?{}, ?{}, ?{}, ?{}),", i + 1, i + 2, i + 3, i + 4,);
        }

        // replace last `,` with `;`
        let mut query = query.chars();
        query.next_back();
        let query = format!("{};", query.as_str());

        let mut query = sqlx::query(&query);
        for window in to_save {
            query = query
                .bind(window.0)
                .bind(window.1)
                .bind(window.2)
                .bind(window.3)
        }
        query.execute(&self.pool).await?;

        Ok(())
    }

    fn get_database_url() -> anyhow::Result<String> {
        let path = crate::get_xdg_dirs()?.place_data_file("apps.db")?;
        let path_str = path.to_str().context("database path is not unicode")?;
        Ok(format!("sqlite:{path_str}"))
    }
}

impl FromRow<'_, SqliteRow> for Window {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let datetime = row.try_get("datetime")?;
        let datetime = DateTime::parse_from_rfc3339(datetime).unwrap().naive_utc();
        let datetime = Utc.from_utc_datetime(&datetime);
        let class = row.try_get("class")?;
        let title = row.try_get("title")?;
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self {
            datetime,
            title,
            class,
            duration,
        })
    }
}

impl FromRow<'_, SqliteRow> for AppUsage {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let app = row.try_get("class")?;
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self { app, duration })
    }
}

impl FromRow<'_, SqliteRow> for AppUsageDay {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let day = row.try_get("date_local")?;
        let date_local = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self {
            date_local,
            duration,
        })
    }
}

fn to_utc(datetime: DateTime<Local>) -> DateTime<Utc> {
    datetime.naive_utc().and_utc()
}
