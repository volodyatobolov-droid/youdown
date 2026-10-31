//! Application data model, persistence and formatting helpers.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::theme::ThemeMode;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeMode,
    pub seed: [u8; 3],
    pub download_dir: String,
    pub concurrency: usize,
    pub open_folder_after: bool,
    pub cookies_browser: String,
    pub history: Vec<HistoryItem>,
}

fn default_seed() -> [u8; 3] {
    [103, 80, 164]
}

fn default_dir() -> String {
    dirs::video_dir()
        .or_else(dirs::download_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .into_owned()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            seed: default_seed(),
            download_dir: default_dir(),
            concurrency: 2,
            open_folder_after: false,
            cookies_browser: String::new(),
            history: Vec::new(),
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("youdown")
    }

    pub fn path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&text) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let dir = Self::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), text);
        }
    }

    pub fn ensure_download_dir(&self) -> PathBuf {
        let path = PathBuf::from(&self.download_dir);
        let _ = std::fs::create_dir_all(&path);
        path
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct HistoryItem {
    pub title: String,
    pub path: String,
    pub url: String,
    pub channel: String,
    pub kind: String,
    pub finished_at: u64,
    pub size: Option<u64>,
}

impl HistoryItem {
    pub fn when(&self) -> String {
        format_time_ago(self.finished_at)
    }
}

// ---------------------------------------------------------------------------
// yt-dlp metadata
// ---------------------------------------------------------------------------

#[derive(Clone, Deserialize, Default)]
pub struct RawVideo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub view_count: Option<u64>,
    #[serde(default)]
    pub like_count: Option<u64>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub webpage_url: Option<String>,
    #[serde(default)]
    pub formats: Vec<RawFormat>,
}

#[derive(Clone, Deserialize, Default)]
pub struct RawFormat {
    #[serde(default)]
    pub format_id: String,
    #[serde(default)]
    pub ext: Option<String>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default)]
    pub vcodec: Option<String>,
    #[serde(default)]
    pub acodec: Option<String>,
    #[serde(default)]
    pub filesize: Option<u64>,
    #[serde(default)]
    pub filesize_approx: Option<u64>,
    #[serde(default)]
    pub tbr: Option<f64>,
    #[serde(default)]
    pub abr: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: Option<f64>,
    pub views: Option<u64>,
    pub likes: Option<u64>,
    pub thumbnail: Option<String>,
    pub url: String,
    pub qualities: Vec<Quality>,
    pub audio_size: Option<u64>,
    pub best_size: Option<u64>,
    pub max_height: u32,
}

#[derive(Clone, Debug)]
pub struct Quality {
    pub height: u32,
    pub fps: Option<u32>,
    pub size: Option<u64>,
    pub total_size: Option<u64>,
}

impl Quality {
    pub fn label(&self) -> String {
        if let Some(fps) = self.fps {
            if fps >= 48 {
                return format!("{}p{}", self.height, fps);
            }
        }
        format!("{}p", self.height)
    }
}

impl VideoInfo {
    pub fn from_raw(raw: RawVideo) -> Self {
        let duration = raw.duration;
        let mut qualities: Vec<Quality> = Vec::new();

        let mut heights: Vec<u32> = raw
            .formats
            .iter()
            .filter(|f| f.height.unwrap_or(0) > 0 && is_video(f))
            .filter_map(|f| f.height)
            .collect();
        heights.sort_unstable();
        heights.dedup();
        heights.reverse();

        for height in heights {
            let mut best: Option<&RawFormat> = None;
            for f in raw.formats.iter().filter(|f| f.height == Some(height) && is_video(f)) {
                match best {
                    None => best = Some(f),
                    Some(b) => {
                        if f.tbr.unwrap_or(0.0) > b.tbr.unwrap_or(0.0)
                            || (f.tbr.is_none() && b.tbr.is_none() && size_of(f) > size_of(b))
                        {
                            best = Some(f);
                        }
                    }
                }
            }
            if let Some(f) = best {
                let fps = f.fps.map(|v| v.round() as u32);
                let video_size = estimate_size(f, duration);
                let audio_size = best_audio_size(&raw.formats, duration);
                let total = match (video_size, audio_size) {
                    (Some(v), Some(a)) => Some(v + a),
                    (Some(v), None) => Some(v),
                    _ => None,
                };
                qualities.push(Quality {
                    height,
                    fps,
                    size: total,
                    total_size: total,
                });
            }
        }

        let max_height = qualities.first().map(|q| q.height).unwrap_or(0);
        let best_size = qualities.first().and_then(|q| q.total_size);

        let title = if raw.title.is_empty() {
            "Без названия".to_owned()
        } else {
            raw.title
        };

        Self {
            id: raw.id,
            title,
            channel: raw
                .channel
                .or(raw.uploader)
                .unwrap_or_else(|| "Неизвестный канал".to_owned()),
            duration,
            views: raw.view_count,
            likes: raw.like_count,
            thumbnail: raw.thumbnail,
            url: raw.webpage_url.unwrap_or_default(),
            qualities,
            audio_size: best_audio_size(&raw.formats, duration),
            best_size,
            max_height,
        }
    }
}

fn is_video(f: &RawFormat) -> bool {
    f.vcodec.as_deref().map(|c| c != "none").unwrap_or(false)
}

fn size_of(f: &RawFormat) -> u64 {
    f.filesize.or(f.filesize_approx).unwrap_or(0)
}

fn estimate_size(f: &RawFormat, duration: Option<f64>) -> Option<u64> {
    if let Some(size) = f.filesize.or(f.filesize_approx) {
        return Some(size);
    }
    match (f.tbr, duration) {
        (Some(tbr), Some(dur)) if tbr > 0.0 && dur > 0.0 => Some((tbr * 1000.0 / 8.0 * dur) as u64),
        _ => None,
    }
}

fn best_audio_size(formats: &[RawFormat], duration: Option<f64>) -> Option<u64> {
    let mut best: Option<&RawFormat> = None;
    for f in formats.iter().filter(|f| {
        f.acodec.as_deref().map(|a| a != "none").unwrap_or(false)
            && f.vcodec.as_deref().map(|v| v == "none").unwrap_or(true)
    }) {
        match best {
            None => best = Some(f),
            Some(b) => {
                if f.abr.unwrap_or(0.0) > b.abr.unwrap_or(0.0) {
                    best = Some(f);
                }
            }
        }
    }
    best.and_then(|f| estimate_size(f, duration))
}

// ---------------------------------------------------------------------------
// Downloads
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadKind {
    Video,
    AudioM4a,
    AudioMp3,
}

impl Default for DownloadKind {
    fn default() -> Self {
        Self::Video
    }
}

impl DownloadKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Видео MP4",
            Self::AudioM4a => "Аудио M4A",
            Self::AudioMp3 => "Аудио MP3",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::AudioM4a => "audio-m4a",
            Self::AudioMp3 => "audio-mp3",
        }
    }

    pub fn icon(self) -> crate::icons::Icon {
        match self {
            Self::Video => crate::icons::Icon::Video,
            Self::AudioM4a | Self::AudioMp3 => crate::icons::Icon::Music,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    Active,
    Done,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug)]
pub struct DownloadItem {
    pub id: u64,
    pub title: String,
    pub channel: String,
    pub url: String,
    pub thumb: Option<String>,
    pub kind: DownloadKind,
    pub height: Option<u32>,
    pub status: DownloadStatus,
    pub progress: f32,
    pub phase: String,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub path: Option<String>,
    pub error: Option<String>,
    pub started_at: u64,
}

impl DownloadItem {
    pub fn quality_label(&self) -> String {
        match (self.kind, self.height) {
            (DownloadKind::Video, Some(h)) => format!("{h}p · MP4"),
            (DownloadKind::Video, None) => "Лучшее · MP4".to_owned(),
            (k, _) => k.label().to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProgressUpdate {
    pub fraction: f32,
    pub phase: String,
    pub speed: Option<String>,
    pub eta: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ThumbData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct Versions {
    pub ytdlp: String,
    pub ffmpeg: String,
}

#[derive(Debug)]
pub enum Event {
    Meta {
        req: u64,
        result: Result<VideoInfo, String>,
    },
    Thumb {
        req: u64,
        result: Result<ThumbData, String>,
    },
    Started {
        id: u64,
        pid: u32,
    },
    Progress {
        id: u64,
        update: ProgressUpdate,
    },
    Done {
        id: u64,
        path: String,
        size: Option<u64>,
    },
    Failed {
        id: u64,
        error: String,
    },
    Cancelled {
        id: u64,
    },
    Versions(Result<Versions, String>),
    FolderPicked(Option<String>),
    Toast {
        text: String,
        error: bool,
    },
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else if value >= 100.0 {
        format!("{:.0} {}", value, UNITS[unit])
    } else {
        format!("{:.1} {}", value, UNITS[unit]).replace('.', ",")
    }
}

pub fn human_optional_size(bytes: Option<u64>) -> String {
    bytes.map(human_size).unwrap_or_else(|| "—".to_owned())
}

pub fn human_duration(seconds: Option<f64>) -> String {
    let Some(secs) = seconds else {
        return "—".to_owned();
    };
    let total = secs.max(0.0).round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn human_count(n: u64, one: &str, few: &str, many: &str) -> String {
    let n_mod100 = n % 100;
    let n_mod10 = n % 10;
    let word = if (11..=14).contains(&n_mod100) {
        many
    } else if n_mod10 == 1 {
        one
    } else if (2..=4).contains(&n_mod10) {
        few
    } else {
        many
    };
    format!("{} {}", human_number(n), word)
}

pub fn human_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1} млрд", n as f64 / 1_000_000_000.0).replace('.', ",")
    } else if n >= 1_000_000 {
        format!("{:.1} млн", n as f64 / 1_000_000.0).replace('.', ",")
    } else if n >= 1_000 {
        format!("{:.0} тыс.", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_time_ago(unix: u64) -> String {
    let now = now_unix();
    let diff = now.saturating_sub(unix);
    if diff < 60 {
        "только что".to_owned()
    } else if diff < 3600 {
        human_count(diff / 60, "минуту назад", "минуты назад", "минут назад")
    } else if diff < 86_400 {
        human_count(diff / 3600, "час назад", "часа назад", "часов назад")
    } else if diff < 2_592_000 {
        human_count(diff / 86_400, "день назад", "дня назад", "дней назад")
    } else {
        let date = unix_to_date(unix);
        date
    }
}

fn unix_to_date(unix: u64) -> String {
    // Small civil-date conversion (Howard Hinnant's algorithm).
    let days = (unix / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{d:02}.{m:02}.{y}")
}

pub fn parse_speed(bytes_per_sec: Option<f64>) -> Option<String> {
    bytes_per_sec
        .filter(|v| *v > 0.0)
        .map(|v| format!("{}/с", human_size(v as u64)))
}
