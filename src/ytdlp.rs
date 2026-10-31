//! Integration with yt-dlp / ffmpeg: analysis, downloads, thumbnails, helpers.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::model::{
    parse_speed, DownloadKind, Event, ProgressUpdate, RawVideo, ThumbData, Versions, VideoInfo,
};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// Locate a helper executable. On Windows the installer ships `yt-dlp.exe` and
/// `ffmpeg.exe` next to `youdown.exe`, so prefer that directory over PATH.
fn tool(name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join(format!("{name}.exe"));
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
        PathBuf::from(format!("{name}.exe"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(name)
    }
}

#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

#[cfg(windows)]
fn ffmpeg_dir() -> Option<PathBuf> {
    let ffmpeg = tool("ffmpeg");
    if ffmpeg.is_file() {
        ffmpeg.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn base_command() -> Command {
    let mut cmd = Command::new(tool("yt-dlp"));
    cmd.arg("--no-colors").arg("--no-warnings");
    #[cfg(windows)]
    if let Some(dir) = ffmpeg_dir() {
        cmd.arg("--ffmpeg-location").arg(dir);
    }
    hide_console(&mut cmd);
    cmd
}

fn add_cookies(cmd: &mut Command, browser: &str) {
    if !browser.is_empty() {
        cmd.arg("--cookies-from-browser").arg(browser);
    }
}

pub fn check_versions() -> Result<Versions, String> {
    let mut ytdlp_cmd = Command::new(tool("yt-dlp"));
    ytdlp_cmd.arg("--version");
    hide_console(&mut ytdlp_cmd);
    let ytdlp = ytdlp_cmd
        .output()
        .map_err(|_| missing_ytdlp())?;
    if !ytdlp.status.success() {
        return Err("Не удалось запустить yt-dlp".to_owned());
    }
    let ytdlp_version = String::from_utf8_lossy(&ytdlp.stdout).trim().to_owned();

    let mut ffmpeg_cmd = Command::new(tool("ffmpeg"));
    ffmpeg_cmd.arg("-version");
    hide_console(&mut ffmpeg_cmd);
    let ffmpeg = ffmpeg_cmd.output();
    let ffmpeg_version = match ffmpeg {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(2))
                .unwrap_or("установлен")
                .to_owned()
        }
        _ => "не найден".to_owned(),
    };

    Ok(Versions {
        ytdlp: ytdlp_version,
        ffmpeg: ffmpeg_version,
    })
}

fn missing_ytdlp() -> String {
    #[cfg(windows)]
    {
        "yt-dlp.exe не найден рядом с YouDown".to_owned()
    }
    #[cfg(not(windows))]
    {
        "yt-dlp не найден в PATH. Установите его командой: sudo pacman -S yt-dlp".to_owned()
    }
}

pub fn spawn_versions(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let _ = tx.send(Event::Versions(check_versions()));
    });
}

fn human_ytdlp_error(stderr: &str) -> String {
    let line = stderr
        .lines()
        .rev()
        .find(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("WARNING")
        })
        .unwrap_or("Не удалось получить данные о видео");
    let line = line.trim();
    let line = line.strip_prefix("ERROR: ").unwrap_or(line);
    line.to_owned()
}

pub fn spawn_analyze(tx: Sender<Event>, req: u64, url: String, cookies: String) {
    std::thread::spawn(move || {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            let _ = tx.send(Event::Meta {
                req,
                result: Err("Ссылка должна начинаться с http:// или https://".to_owned()),
            });
            return;
        }

        let mut cmd = base_command();
        cmd.args(["--no-playlist", "--dump-single-json"]);
        add_cookies(&mut cmd, &cookies);
        cmd.arg(&url);

        let output = match cmd.output() {
            Ok(out) => out,
            Err(_) => {
                let _ = tx.send(Event::Meta {
                    req,
                    result: Err(missing_ytdlp()),
                });
                return;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = tx.send(Event::Meta {
                req,
                result: Err(human_ytdlp_error(&stderr)),
            });
            return;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let raw: RawVideo = match serde_json::from_str(&stdout) {
            Ok(raw) => raw,
            Err(err) => {
                let _ = tx.send(Event::Meta {
                    req,
                    result: Err(format!("Не удалось разобрать ответ yt-dlp: {err}")),
                });
                return;
            }
        };

        let info = VideoInfo::from_raw(raw);
        let thumb = info.thumbnail.clone();
        let _ = tx.send(Event::Meta {
            req,
            result: Ok(info),
        });

        if let Some(thumb_url) = thumb {
            if let Ok(data) = fetch_thumbnail(&thumb_url) {
                let _ = tx.send(Event::Thumb {
                    req,
                    result: Ok(data),
                });
            }
        }
    });
}

fn fetch_thumbnail(url: &str) -> Result<ThumbData, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent(UA)
        .build();
    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("миниатюра: {e}"))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(16 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;

    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let img = if img.width() > 720 {
        let new_w = 640u32;
        let new_h = (img.height() as f32 * new_w as f32 / img.width() as f32).round() as u32;
        img.resize_exact(new_w, new_h.max(1), image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    round_corners(&mut rgba, (w as f32 / 17.0).max(8.0));
    Ok(ThumbData {
        rgba: rgba.into_raw(),
        width: w,
        height: h,
    })
}

fn round_corners(img: &mut image::RgbaImage, radius: f32) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let r = radius.min(w as f32 / 2.0).min(h as f32 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let cx = if px < r {
                r
            } else if px > w as f32 - r {
                w as f32 - r
            } else {
                px
            };
            let cy = if py < r {
                r
            } else if py > h as f32 - r {
                h as f32 - r
            } else {
                py
            };
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if d > r - 0.5 {
                let alpha = ((r - d + 0.5) * 255.0).clamp(0.0, 255.0) as u8;
                let pixel = img.get_pixel_mut(x, y);
                pixel.0[3] = pixel.0[3].min(alpha);
            }
        }
    }
}

pub struct DownloadRequest {
    pub id: u64,
    pub url: String,
    pub kind: DownloadKind,
    pub height: Option<u32>,
    pub dir: PathBuf,
    pub cookies: String,
    pub title: String,
    pub channel: String,
    pub thumb: Option<String>,
}

pub fn spawn_download(tx: Sender<Event>, req: DownloadRequest) {
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&req.dir);

        let mut cmd = base_command();
        cmd.args(["--newline", "--no-playlist", "--no-simulate", "--continue", "--retries", "5"]);
        cmd.args([
            "--progress-delta",
            "0.3",
            "--progress-template",
            "download:PROG|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress.fragment_index)s|%(progress.fragment_count)s",
        ]);
        cmd.args(["--print", "after_move:FILE:%(filepath)s"]);
        let template = req
            .dir
            .join("%(title).180B [%(id)s].%(ext)s")
            .to_string_lossy()
            .into_owned();
        cmd.arg("-o").arg(template);

        match req.kind {
            DownloadKind::Video => {
                let filter = match req.height {
                    Some(h) => format!("bv*[height<={h}]+ba/b[height<={h}]/b"),
                    None => "bv*+ba/b".to_owned(),
                };
                cmd.args(["-f", &filter, "--merge-output-format", "mp4"]);
            }
            DownloadKind::AudioM4a => {
                cmd.args(["-f", "ba[ext=m4a]/ba/b"]);
            }
            DownloadKind::AudioMp3 => {
                cmd.args(["-f", "ba/b", "-x", "--audio-format", "mp3", "--audio-quality", "0"]);
            }
        }

        add_cookies(&mut cmd, &req.cookies);
        cmd.arg(&req.url);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = tx.send(Event::Failed {
                    id: req.id,
                    error: format!("Не удалось запустить yt-dlp: {err}"),
                });
                return;
            }
        };

        let _ = tx.send(Event::Started {
            id: req.id,
            pid: child.id(),
        });

        let stderr_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let mut buf = String::new();
                let _ = BufReader::new(stderr).read_to_string(&mut buf);
                buf
            })
        });

        let mut path: Option<String> = None;
        let mut phase = match req.kind {
            DownloadKind::Video => "Загрузка видео".to_owned(),
            _ => "Загрузка аудио".to_owned(),
        };

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("FILE:") {
                    path = Some(rest.trim().to_owned());
                    continue;
                }
                if let Some(rest) = line.strip_prefix("PROG|") {
                    let parts: Vec<&str> = rest.split('|').collect();
                    if parts.len() >= 5 {
                        let percent = parts[0]
                            .trim()
                            .trim_end_matches('%')
                            .trim()
                            .replace(',', ".")
                            .parse::<f32>()
                            .ok();
                        let speed_str = parts[1].trim();
                        let eta_str = parts[2].trim();
                        let frag_idx = parts[3].trim().parse::<u64>().ok();
                        let frag_count = parts[4].trim().parse::<u64>().ok();

                        let fraction = if let Some(frag) = frag_idx.filter(|f| *f > 0) {
                            if let Some(count) = frag_count.filter(|c| *c > 0) {
                                if percent.is_none() || percent == Some(0.0) {
                                    frag as f32 / count as f32
                                } else {
                                    percent.unwrap_or(0.0) / 100.0
                                }
                            } else {
                                percent.unwrap_or(0.0) / 100.0
                            }
                        } else {
                            percent.unwrap_or(0.0) / 100.0
                        };

                        let speed = if speed_str.is_empty() || speed_str == "NA" {
                            None
                        } else {
                            Some(speed_str.to_owned())
                        };
                        let eta = if eta_str.is_empty() || eta_str == "NA" {
                            None
                        } else {
                            Some(eta_str.to_owned())
                        };

                        let _ = tx.send(Event::Progress {
                            id: req.id,
                            update: ProgressUpdate {
                                fraction: fraction.clamp(0.0, 1.0),
                                phase: phase.clone(),
                                speed,
                                eta,
                            },
                        });
                    }
                    continue;
                }

                if let Some(dest) = line.strip_prefix("[download] Destination: ") {
                    let lower = dest.to_lowercase();
                    if lower.ends_with(".m4a") || lower.ends_with(".mp3") || lower.ends_with(".opus")
                    {
                        phase = "Загрузка аудио".to_owned();
                    } else if lower.ends_with(".mp4") || lower.ends_with(".webm") {
                        phase = "Загрузка видео".to_owned();
                    }
                } else if line.starts_with("[Merger]") || line.contains("Merging formats") {
                    phase = "Склейка дорожек".to_owned();
                } else if line.contains("[ExtractAudio]") {
                    phase = "Конвертация аудио".to_owned();
                } else if line.contains("[VideoConvertor]") || line.contains("[VideoRemuxer]") {
                    phase = "Обработка видео".to_owned();
                }
            }
        }

        let status = child.wait();
        let stderr_text = stderr_handle
            .and_then(|h| h.join().ok())
            .unwrap_or_default();

        match status {
            Ok(status) if status.success() => {
                let path = path.or_else(|| find_latest(&req.dir, &req.title)).unwrap_or_default();
                let size = std::fs::metadata(&path).ok().map(|m| m.len());
                let _ = tx.send(Event::Done {
                    id: req.id,
                    path,
                    size,
                });
            }
            _ => {
                let _ = tx.send(Event::Failed {
                    id: req.id,
                    error: human_ytdlp_error(&stderr_text),
                });
            }
        }
    });
}

fn find_latest(dir: &Path, title: &str) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    let short: String = title.chars().take(24).collect();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !short.is_empty() && !name.contains(&short) {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
        if let Some(modified) = modified {
            if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                best = Some((modified, path.to_string_lossy().into_owned()));
            }
        }
    }
    best.map(|(_, p)| p)
}

pub fn cancel_process(pid: u32) {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        hide_console(&mut cmd);
        let _ = cmd.status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
        let _ = Command::new("pkill")
            .args(["-TERM", "-P", &pid.to_string()])
            .status();
    }
}

pub fn open_path(path: &str) {
    let target = Path::new(path);
    let arg = if target.is_file() {
        target.to_path_buf()
    } else if target.is_dir() {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };

    #[cfg(windows)]
    {
        let mut cmd = if path.starts_with("http://") || path.starts_with("https://") {
            let mut cmd = Command::new("rundll32");
            cmd.args(["url.dll,FileProtocolHandler", path]);
            cmd
        } else if target.is_file() {
            let mut cmd = Command::new("explorer");
            cmd.arg(format!("/select,{}", arg.display()));
            cmd
        } else {
            let mut cmd = Command::new("explorer");
            cmd.arg(arg.as_os_str());
            cmd
        };
        hide_console(&mut cmd);
        let _ = cmd.spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("xdg-open").arg(arg).spawn();
    }
}

#[cfg(windows)]
pub fn paste_clipboard() -> Option<String> {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-STA",
        "-Command",
        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; Get-Clipboard -Raw",
    ]);
    hide_console(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout)
        .trim_matches(|c: char| c == '\r' || c == '\n' || c == '\u{feff}')
        .trim()
        .to_owned();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(not(windows))]
pub fn paste_clipboard() -> Option<String> {
    let attempts: [(&str, &[&str]); 3] = [
        ("wl-paste", &["-n"]),
        ("xclip", &["-o", "-selection", "clipboard"]),
        ("xsel", &["-ob"]),
    ];
    for (bin, args) in attempts {
        if let Ok(output) = Command::new(bin).args(args).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

pub fn pick_folder(tx: Sender<Event>, current: String) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        let picked = {
            let escaped = current.replace('\'', "''");
            let script = format!(
                "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; \
                 Add-Type -AssemblyName System.Windows.Forms; \
                 $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
                 $d.Description = 'Папка для загрузок'; \
                 $d.SelectedPath = '{escaped}'; \
                 $f = New-Object System.Windows.Forms.Form; \
                 $f.TopMost = $true; \
                 if ($d.ShowDialog($f) -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $d.SelectedPath }}"
            );
            let mut cmd = Command::new("powershell");
            cmd.args(["-NoProfile", "-STA", "-Command", &script]);
            hide_console(&mut cmd);
            cmd.output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        };

        #[cfg(not(windows))]
        let picked = {
            let output = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--directory",
                    "--title=Папка для загрузок",
                    &format!("--filename={current}/"),
                ])
                .output();
            output
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        };

        let _ = tx.send(Event::FolderPicked(picked.filter(|s| !s.is_empty())));
    });
}

pub fn update_ytdlp(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let mut cmd = Command::new(tool("yt-dlp"));
        cmd.arg("-U");
        hide_console(&mut cmd);
        let output = cmd.output();
        let text = match output {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    text = if err.trim().is_empty() {
                        #[cfg(windows)]
                        {
                            "Не удалось обновить yt-dlp. Скачайте новый yt-dlp.exe вручную."
                                .to_owned()
                        }
                        #[cfg(not(windows))]
                        {
                            "Не удалось обновить yt-dlp автоматически. Попробуйте: sudo pacman -Syu yt-dlp"
                                .to_owned()
                        }
                    } else {
                        human_ytdlp_error(&err)
                    };
                    let _ = tx.send(Event::Toast { text, error: true });
                    return;
                }
                if text.is_empty() {
                    text = "yt-dlp обновлён".to_owned();
                } else if let Some(last) = text.lines().last() {
                    text = last.to_owned();
                }
                let _ = tx.send(Event::Toast {
                    text,
                    error: false,
                });
            }
            Err(_) => {
                let _ = tx.send(Event::Toast {
                    text: missing_ytdlp(),
                    error: true,
                });
            }
        };
    });
}

/// Simple helper used to estimate the remaining time as a human string.
pub fn eta_text(eta: &str) -> String {
    if eta.is_empty() || eta == "NA" {
        "—".to_owned()
    } else {
        eta.to_owned()
    }
}

#[allow(dead_code)]
pub fn speed_text(speed: Option<f64>) -> String {
    parse_speed(speed).unwrap_or_else(|| "—".to_owned())
}
