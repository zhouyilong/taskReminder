//! 节假日数据在线更新：从项目仓库拉取 `holidays-cn.json`（先 jsDelivr 镜像，国内通常更容易
//! 访问；再 GitHub raw），经 `holidays::apply_remote` 校验后写入数据目录缓存。每年国务院公布
//! 次年安排、仓库更新数据后，用户无需升级应用即可获得。
//!
//! 启动 30 秒后检查一次，之后每 24 小时一次；设置中可关闭（本机设置），也可以手动“立即检查”。
//! 网络失败只记录日志，不影响其他功能。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::holidays;
use crate::state::AppState;

const SOURCES: &[&str] = &[
    "https://cdn.jsdelivr.net/gh/zhouyilong/taskReminder@main/src-tauri/data/holidays-cn.json",
    "https://raw.githubusercontent.com/zhouyilong/taskReminder/main/src-tauri/data/holidays-cn.json",
];
/// 用于测试或自建镜像：设置后只从这个地址获取。
const SOURCE_ENV: &str = "TASKREMINDER_HOLIDAY_URL";
/// 数据文件只有几 KB，超过这个大小的响应一律拒绝。
const MAX_BYTES: u64 = 256 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const STARTUP_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 一次检查的结果，返回给设置界面。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HolidayCheckResult {
    /// 生效的节假日数据是否因此改变。
    pub changed: bool,
    /// 检查后已有数据的年份。
    pub years: Vec<i32>,
}

fn sources() -> Vec<String> {
    match std::env::var(SOURCE_ENV) {
        Ok(url) if !url.trim().is_empty() => vec![url.trim().to_string()],
        _ => SOURCES.iter().map(|url| url.to_string()).collect(),
    }
}

/// 下载数据文件：只接受 2xx、不超过 `MAX_BYTES` 的 UTF-8 文本。
fn fetch(url: &str) -> Result<String, String> {
    let client = crate::http::blocking_client_builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("TaskReminder/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(url).send().map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    if response.content_length().is_some_and(|len| len > MAX_BYTES) {
        return Err("响应过大".to_string());
    }
    let mut body = Vec::new();
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    if body.len() as u64 > MAX_BYTES {
        return Err("响应过大".to_string());
    }
    String::from_utf8(body).map_err(|_| "响应不是 UTF-8 文本".to_string())
}

/// 依次尝试各个地址，第一个下载并通过校验的为准；全部失败时返回各地址的原因。
fn update_from_sources(data_dir: &Path, urls: &[String]) -> Result<bool, String> {
    let mut errors = Vec::new();
    for url in urls {
        match fetch(url).and_then(|raw| holidays::apply_remote(data_dir, &raw)) {
            Ok(changed) => return Ok(changed),
            Err(err) => errors.push(format!("{}：{}", source_label(url), err)),
        }
    }
    Err(errors.join("；"))
}

fn source_label(url: &str) -> &str {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(url)
}

/// 数据变化后：重新计算法定工作日提醒，并通知界面刷新“节假日待更新”等提示。
fn on_changed(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(err) = state.scheduler.reschedule_workday_tasks() {
            eprintln!("[holidays] 重新计算法定工作日提醒失败: {}", err);
        }
    }
    let _ = app.emit("holidays-updated", holidays::covered_years());
}

/// 立即检查一次（阻塞：网络请求）。设置界面的“立即检查”与后台定时检查共用。
pub fn check_now(app: &AppHandle, data_dir: &Path) -> Result<HolidayCheckResult, String> {
    let changed = update_from_sources(data_dir, &sources())?;
    if changed {
        on_changed(app);
    }
    Ok(HolidayCheckResult {
        changed,
        years: holidays::covered_years(),
    })
}

fn auto_update_enabled(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| state.db.holiday_auto_update_enabled().ok())
        .unwrap_or(false)
}

/// 启动后台检查线程（普通线程：网络请求是阻塞的，不能放进异步运行时）。
pub fn start(app: AppHandle, data_dir: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(STARTUP_DELAY);
        loop {
            if auto_update_enabled(&app) {
                match check_now(&app, &data_dir) {
                    Ok(result) if result.changed => {
                        eprintln!("[holidays] 节假日数据已更新，覆盖年份 {:?}", result.years)
                    }
                    Ok(_) => {}
                    Err(err) => eprintln!("[holidays] 检查节假日数据失败: {}", err),
                }
            }
            std::thread::sleep(CHECK_INTERVAL);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    /// 在本地起一个只应答一次的 HTTP 服务，返回它的地址。
    fn serve_once(status: &str, body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let status = status.to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = stream.read(&mut buffer);
                let header = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                    status,
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        format!("http://{}/holidays-cn.json", addr)
    }

    #[test]
    fn fetch_accepts_small_success_responses_only() {
        let ok = serve_once("200 OK", br#"{"2025": {"off": []}}"#.to_vec());
        assert_eq!(fetch(&ok).unwrap(), r#"{"2025": {"off": []}}"#);

        let missing = serve_once("404 Not Found", b"not found".to_vec());
        assert_eq!(fetch(&missing).unwrap_err(), "HTTP 404");

        let huge = serve_once("200 OK", vec![b' '; (MAX_BYTES + 10) as usize]);
        assert_eq!(fetch(&huge).unwrap_err(), "响应过大");
    }

    #[test]
    fn falls_back_to_next_source_and_reports_every_failure() {
        let dir = std::env::temp_dir().join(format!("holiday-update-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // 第一个地址返回无效内容，第二个返回与内置相同的数据（没有新年份，不算变化）。
        let bad = serve_once("200 OK", b"<html>oops</html>".to_vec());
        let same = serve_once(
            "200 OK",
            include_str!("../data/holidays-cn.json").as_bytes().to_vec(),
        );
        assert!(!update_from_sources(&dir, &[bad, same]).unwrap());

        let down = serve_once("503 Service Unavailable", Vec::new());
        let err = update_from_sources(&dir, &[down]).unwrap_err();
        assert!(
            err.contains("127.0.0.1") && err.contains("HTTP 503"),
            "{}",
            err
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn source_label_is_the_host() {
        assert_eq!(source_label(SOURCES[0]), "cdn.jsdelivr.net");
        assert_eq!(source_label(SOURCES[1]), "raw.githubusercontent.com");
    }
}
