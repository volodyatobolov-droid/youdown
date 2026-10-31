#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod icons;
mod model;
mod theme;
mod widgets;
mod ytdlp;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("YouDown {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--doctor") {
        doctor();
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 820.0])
        .with_min_inner_size([900.0, 640.0])
        .with_title("YouDown — Material You downloader");
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let initial_url = args.into_iter().find(|a| !a.starts_with('-'));

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "YouDown",
        options,
        Box::new(move |cc| Ok(Box::new(app::YouDown::new(cc, initial_url)))),
    )
}

fn load_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/icon-256.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn doctor() {
    println!("YouDown {}", env!("CARGO_PKG_VERSION"));
    println!("config: {}", model::Config::path().display());
    match model::Config::load() {
        cfg => {
            println!("download dir: {}", cfg.download_dir);
            println!("theme: {:?}, seed: {:?}", cfg.theme, cfg.seed);
        }
    }
    match ytdlp::check_versions() {
        Ok(v) => {
            println!("yt-dlp: {}", v.ytdlp);
            println!("ffmpeg: {}", v.ffmpeg);
        }
        Err(e) => println!("dependencies: {e}"),
    }
    for tool in ["zenity", "wl-paste", "gsettings", "xdg-open"] {
        #[cfg(not(windows))]
        {
            let found = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("command -v {tool}"))
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            println!("{tool}: {}", if found { "ok" } else { "missing" });
        }
        #[cfg(windows)]
        let _ = tool;
    }
}
