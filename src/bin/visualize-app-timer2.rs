use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result};
use app_timer2::Window;
use chrono::{Local, NaiveDate, TimeZone, Utc};
use humantime::format_duration;
use prettytable::{format, row, Table};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = app_timer2::get_pool().await?;

    let mut args = std::env::args();
    args.next();
    let date_str = args.next();
    let local_from = match date_str {
        Some(date_str) => {
            let datetime = NaiveDate::parse_from_str(&date_str, "%d-%m-%Y")?
                .and_hms_opt(0, 0, 0)
                .unwrap();
            Local
                .from_local_datetime(&datetime)
                .single()
                .context("failed loading date")?
        }
        None => {
            let today = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
            Local.from_local_datetime(&today).single().unwrap()
        }
    };

    println!("Showing apps in date {}", local_from.format("%B %d, %Y"));

    let local_to = local_from + chrono::Duration::days(1);

    let from = local_from.with_timezone(&Utc);
    let to = local_to.with_timezone(&Utc);

    let windows = app_timer2::get_windows_between(&pool, from, to).await?;
    let apps = get_apps(&windows);
    let mut apps_vec: Vec<_> = apps.into_iter().collect();
    apps_vec.sort_by_key(|k| k.1);

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row!["App", "Duration"]);
    for (app, duration) in apps_vec {
        table.add_row(row![app, format_duration(duration)]);
    }
    table.printstd();

    Ok(())
}

fn get_apps(windows: &[Window]) -> HashMap<String, Duration> {
    windows.iter().fold(HashMap::new(), |mut apps, x| {
        let duration = apps.entry(x.class.1.clone()).or_default();
        *duration += x.duration;
        apps
    })
}
