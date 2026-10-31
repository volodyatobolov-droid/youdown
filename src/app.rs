//! YouDown — Material You YouTube downloader.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    Vec2,
};

use crate::icons::{self, Icon};
use crate::model::*;
use crate::theme::{self, alpha, font, font_bold, font_medium, Palette, ThemeMode};
use crate::widgets::{self, ButtonKind};
use crate::ytdlp;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Download,
    Library,
    Settings,
}

struct Toast {
    text: String,
    error: bool,
    at: Instant,
}

#[derive(Clone)]
enum RowAction {
    Cancel(u64),
    Retry(u64),
    Remove(u64),
    OpenFile(String),
    OpenFolder(String),
    OpenUrl(String),
}

pub struct YouDown {
    cfg: Config,
    palette: Palette,
    dark: bool,
    theme_seed: [u8; 3],
    last_system_check: Instant,
    page: Page,

    url: String,
    analyzing: bool,
    analyze_req: u64,
    video: Option<VideoInfo>,
    thumb: Option<egui::TextureHandle>,
    selected_height: Option<u32>,
    kind: DownloadKind,

    downloads: Vec<DownloadItem>,
    active: HashSet<u64>,
    queue: VecDeque<u64>,
    pids: HashMap<u64, u32>,
    next_id: u64,

    tx: Sender<Event>,
    rx: Receiver<Event>,

    toast: Option<Toast>,
    versions: Option<Result<Versions, String>>,
    versions_loading: bool,

    library_query: String,
}

impl YouDown {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_url: Option<String>) -> Self {
        theme::install_fonts(&cc.egui_ctx);

        let cfg = Config::load();
        let dark = resolve_dark(cfg.theme);
        let palette = Palette::from_seed(cfg.seed, dark);
        theme::apply_style(&cc.egui_ctx, &palette);
        let _ = cfg.ensure_download_dir();

        let (tx, rx) = channel();
        ytdlp::spawn_versions(tx.clone());

        let mut app = Self {
            theme_seed: cfg.seed,
            last_system_check: Instant::now(),
            cfg,
            palette,
            dark,
            page: Page::Download,
            url: String::new(),
            analyzing: false,
            analyze_req: 0,
            video: None,
            thumb: None,
            selected_height: None,
            kind: DownloadKind::Video,
            downloads: Vec::new(),
            active: HashSet::new(),
            queue: VecDeque::new(),
            pids: HashMap::new(),
            next_id: 0,
            tx,
            rx,
            toast: None,
            versions: None,
            versions_loading: true,
            library_query: String::new(),
        };

        if let Some(url) = initial_url {
            let trimmed = url.trim().to_owned();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                app.url = trimmed;
                app.analyze();
            }
        }

        app
    }

    // -----------------------------------------------------------------------
    // State / theme
    // -----------------------------------------------------------------------

    fn sync_theme(&mut self, ctx: &egui::Context) {
        if self.cfg.theme == ThemeMode::System
            && self.last_system_check.elapsed() > Duration::from_secs(4)
        {
            self.last_system_check = Instant::now();
            let dark = resolve_dark(self.cfg.theme);
            if dark != self.dark {
                self.dark = dark;
                self.palette = Palette::from_seed(self.cfg.seed, dark);
                theme::apply_style(ctx, &self.palette);
            }
        }
        if self.cfg.seed != self.theme_seed || resolve_dark(self.cfg.theme) != self.dark {
            self.dark = resolve_dark(self.cfg.theme);
            self.theme_seed = self.cfg.seed;
            self.palette = Palette::from_seed(self.cfg.seed, self.dark);
            theme::apply_style(ctx, &self.palette);
        }
    }

    fn set_theme(&mut self, mode: ThemeMode) {
        self.cfg.theme = mode;
        self.dark = resolve_dark(mode);
        self.palette = Palette::from_seed(self.cfg.seed, self.dark);
        self.theme_seed = self.cfg.seed;
        self.cfg.save();
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast {
            text: text.into(),
            error,
            at: Instant::now(),
        });
    }

    fn analyze(&mut self) {
        let url = self.url.trim().to_owned();
        if url.is_empty() {
            return;
        }
        self.url = url.clone();
        self.analyze_req += 1;
        self.analyzing = true;
        self.video = None;
        self.thumb = None;
        self.selected_height = None;
        ytdlp::spawn_analyze(
            self.tx.clone(),
            self.analyze_req,
            url,
            self.cfg.cookies_browser.clone(),
        );
    }

    fn queue_download(&mut self) {
        let Some(video) = self.video.clone() else {
            return;
        };
        self.next_id += 1;
        let id = self.next_id;
        let height = if self.kind == DownloadKind::Video {
            self.selected_height
        } else {
            None
        };
        self.downloads.insert(
            0,
            DownloadItem {
                id,
                title: video.title.clone(),
                channel: video.channel.clone(),
                url: video.url.clone(),
                thumb: video.thumbnail.clone(),
                kind: self.kind,
                height,
                status: DownloadStatus::Queued,
                progress: 0.0,
                phase: "В очереди".to_owned(),
                speed: None,
                eta: None,
                path: None,
                error: None,
                started_at: now_unix(),
            },
        );
        self.queue.push_back(id);
    }

    fn schedule(&mut self) {
        while self.active.len() < self.cfg.concurrency.max(1) {
            let Some(id) = self.queue.pop_front() else {
                break;
            };
            let Some(item) = self.downloads.iter().find(|d| d.id == id) else {
                continue;
            };
            if item.status != DownloadStatus::Queued {
                continue;
            }
            let req = ytdlp::DownloadRequest {
                id,
                url: item.url.clone(),
                kind: item.kind,
                height: item.height,
                dir: self.cfg.ensure_download_dir(),
                cookies: self.cfg.cookies_browser.clone(),
                title: item.title.clone(),
                channel: item.channel.clone(),
                thumb: item.thumb.clone(),
            };
            if let Some(it) = self.downloads.iter_mut().find(|d| d.id == id) {
                it.status = DownloadStatus::Active;
                it.phase = "Подключение…".to_owned();
            }
            self.active.insert(id);
            ytdlp::spawn_download(self.tx.clone(), req);
        }
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Meta { req, result } => {
                    if req != self.analyze_req {
                        continue;
                    }
                    self.analyzing = false;
                    match result {
                        Ok(info) => {
                            self.selected_height = info.qualities.first().map(|q| q.height);
                            self.kind = DownloadKind::Video;
                            self.video = Some(info);
                        }
                        Err(err) => {
                            self.video = None;
                            self.toast(err, true);
                        }
                    }
                }
                Event::Thumb { req, result } => {
                    if req != self.analyze_req {
                        continue;
                    }
                    if let Ok(data) = result {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [data.width as usize, data.height as usize],
                            &data.rgba,
                        );
                        self.thumb = Some(ctx.load_texture(
                            format!("thumb-{req}"),
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                Event::Started { id, pid } => {
                    let cancelled = self
                        .downloads
                        .iter()
                        .find(|d| d.id == id)
                        .map(|d| d.status == DownloadStatus::Cancelled)
                        .unwrap_or(false);
                    if cancelled {
                        ytdlp::cancel_process(pid);
                    } else {
                        self.pids.insert(id, pid);
                    }
                }
                Event::Progress { id, update } => {
                    if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                        if item.status == DownloadStatus::Active {
                            item.progress = update.fraction;
                            item.phase = update.phase;
                            item.speed = update.speed;
                            item.eta = update.eta;
                        }
                    }
                }
                Event::Done { id, path, size } => {
                    self.active.remove(&id);
                    self.pids.remove(&id);
                    let mut history_item = None;
                    if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                        if item.status == DownloadStatus::Cancelled {
                            continue;
                        }
                        item.status = DownloadStatus::Done;
                        item.progress = 1.0;
                        item.phase = "Готово".to_owned();
                        item.speed = None;
                        item.eta = None;
                        item.path = Some(path.clone());
                        history_item = Some(HistoryItem {
                            title: item.title.clone(),
                            path: path.clone(),
                            url: item.url.clone(),
                            channel: item.channel.clone(),
                            kind: item.quality_label(),
                            finished_at: now_unix(),
                            size,
                        });
                    }
                    if let Some(history_item) = history_item {
                        self.cfg.history.insert(0, history_item);
                        self.cfg.history.truncate(250);
                        self.cfg.save();
                        if self.cfg.open_folder_after {
                            let parent = PathBuf::from(&path)
                                .parent()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_else(|| self.cfg.download_dir.clone());
                            ytdlp::open_path(&parent);
                        }
                        self.toast("Загрузка завершена", false);
                    }
                }
                Event::Failed { id, error } => {
                    self.active.remove(&id);
                    self.pids.remove(&id);
                    let mut cancelled = false;
                    if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                        if item.status != DownloadStatus::Cancelled {
                            item.status = DownloadStatus::Failed;
                            item.phase = "Ошибка".to_owned();
                            item.error = Some(error.clone());
                            item.speed = None;
                            item.eta = None;
                        } else {
                            cancelled = true;
                        }
                    }
                    if !cancelled {
                        self.toast(error, true);
                    }
                }
                Event::Cancelled { id } => {
                    self.active.remove(&id);
                    self.pids.remove(&id);
                }
                Event::Versions(result) => {
                    self.versions_loading = false;
                    self.versions = Some(result);
                }
                Event::FolderPicked(Some(dir)) => {
                    self.cfg.download_dir = dir;
                    let _ = self.cfg.ensure_download_dir();
                    self.cfg.save();
                    self.toast("Папка загрузок обновлена", false);
                }
                Event::FolderPicked(None) => {}
                Event::Toast { text, error } => self.toast(text, error),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Layout helpers
    // -----------------------------------------------------------------------

    fn page_header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        let p = self.palette;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(font_bold(34.0))
                        .color(p.on_surface),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(subtitle)
                        .font(font(14.0))
                        .color(p.on_surface_variant),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let icon = if self.dark { Icon::Sun } else { Icon::Moon };
                if widgets::icon_button(
                    ui,
                    &p,
                    icon,
                    if self.dark {
                        "Светлая тема"
                    } else {
                        "Тёмная тема"
                    },
                    44.0,
                    None,
                )
                .clicked()
                {
                    self.set_theme(if self.dark {
                        ThemeMode::Light
                    } else {
                        ThemeMode::Dark
                    });
                }
                if widgets::icon_button(ui, &p, Icon::Folder, "Открыть папку загрузок", 44.0, None)
                    .clicked()
                {
                    let dir = self.cfg.ensure_download_dir();
                    ytdlp::open_path(&dir.to_string_lossy());
                }
            });
        });
    }

    fn centered(ui: &mut egui::Ui, max_width: f32, add: impl FnOnce(&mut egui::Ui)) {
        let avail = ui.available_width();
        let width = avail.min(max_width).max(320.0);
        let pad = ((avail - width) * 0.5).max(0.0);
        ui.horizontal_top(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(width);
                add(ui);
            });
        });
    }

    fn section_title(&self, ui: &mut egui::Ui, title: &str, right: Option<&str>) -> bool {
        let p = self.palette;
        let mut action = false;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .font(font_medium(20.0))
                    .color(p.on_surface),
            );
            if let Some(right) = right {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, &p, ButtonKind::Text, right, None, true).clicked() {
                        action = true;
                    }
                });
            }
        });
        action
    }

    fn badge(ui: &mut egui::Ui, p: &Palette, icon: Icon, text: &str) {
        let font_id = font_medium(12.5);
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font_id, p.on_secondary_container);
        let width = galley.size().x + 46.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(15), p.secondary_container);
        icons::draw(
            painter,
            Rect::from_center_size(
                Pos2::new(rect.min.x + 17.0, rect.center().y),
                Vec2::splat(15.0),
            ),
            icon,
            p.on_secondary_container,
            p.secondary_container,
        );
        painter.galley(
            Pos2::new(
                rect.min.x + 30.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            p.on_secondary_container,
        );
    }

    // -----------------------------------------------------------------------
    // Download page
    // -----------------------------------------------------------------------

    fn page_download(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui, "Загрузки", "Скачивайте видео и музыку с YouTube в пару кликов");
        ui.add_space(22.0);
        self.url_card(ui);
        ui.add_space(20.0);

        if self.analyzing {
            self.loading_card(ui);
        } else if let Some(video) = self.video.clone() {
            self.video_card(ui, &video);
        } else {
            self.hero_card(ui);
        }

        ui.add_space(26.0);
        self.downloads_section(ui);
    }

    fn url_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let mut analyze = false;
        let mut paste = false;

        widgets::card(ui, p.surface_container_low, 28, 18, |ui| {
            ui.horizontal(|ui| {
                let total = ui.available_width();
                let side = 48.0 + 10.0 + 178.0 + 10.0;
                let field_w = (total - side).max(180.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(field_w, 56.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        let response = widgets::text_field(
                            ui,
                            &p,
                            "url-field",
                            &mut self.url,
                            "https://www.youtube.com/watch?v=…",
                            Some(Icon::Link),
                        );
                        if response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !self.url.trim().is_empty()
                        {
                            analyze = true;
                        }
                    },
                );
                ui.add_space(4.0);
                if widgets::icon_button(ui, &p, Icon::Paste, "Вставить из буфера", 48.0, None)
                    .clicked()
                {
                    paste = true;
                }
                ui.add_space(2.0);
                let label = if self.analyzing {
                    "Анализируем…"
                } else {
                    "Анализировать"
                };
                if widgets::button(
                    ui,
                    &p,
                    ButtonKind::Filled,
                    label,
                    Some(Icon::Search),
                    !self.analyzing && !self.url.trim().is_empty(),
                )
                .clicked()
                {
                    analyze = true;
                }
                if self.analyzing {
                    ui.add(egui::Spinner::new().size(18.0).color(p.primary));
                }
            });
        });

        if paste {
            if let Some(text) = ytdlp::paste_clipboard() {
                self.url = text.trim().to_owned();
            } else {
                self.toast("Не удалось прочитать буфер обмена", true);
            }
        }
        if analyze && !self.analyzing {
            self.analyze();
        }
    }

    fn loading_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        widgets::card(ui, p.surface_container_low, 28, 32, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(28.0).color(p.primary));
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Получаем информацию о видео")
                            .font(font_medium(18.0))
                            .color(p.on_surface),
                    );
                    ui.label(
                        RichText::new("yt-dlp опрашивает доступные форматы и дорожки…")
                            .font(font(13.5))
                            .color(p.on_surface_variant),
                    );
                });
            });
            ui.add_space(16.0);
            // Skeleton lines.
            for (i, w) in [0.55f32, 0.9, 0.7].iter().enumerate() {
                let width = ui.available_width() * w;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 12.0), Sense::hover());
                let t = ui.input(|i| i.time) as f32;
                let pulse = 0.35 + 0.25 * ((t * 1.6 + i as f32).sin() * 0.5 + 0.5);
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(6),
                    alpha(p.on_surface, (pulse * 255.0) as u8),
                );
                ui.add_space(8.0);
            }
        });
    }

    fn hero_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        widgets::card(ui, p.surface_container_low, 28, 32, |ui| {
            let wide = ui.available_width() > 760.0;
            let body = |ui: &mut egui::Ui| {
                ui.label(
                    RichText::new("MATERIAL YOU DOWNLOADER")
                        .font(font_medium(11.5))
                        .color(p.primary),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(
                        RichText::new("Скачивайте. Слушайте. Смотрите.")
                            .font(font_bold(32.0))
                            .color(p.on_surface),
                    )
                    .wrap(),
                );
                ui.add_space(10.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Вставьте ссылку на видео — YouDown найдёт лучшие форматы, скачает видео и аудио дорожки и аккуратно склеит их через ffmpeg.",
                        )
                        .font(font(15.0))
                        .color(p.on_surface_variant),
                    )
                    .wrap(),
                );
                ui.add_space(18.0);
                ui.horizontal_wrapped(|ui| {
                    Self::badge(ui, &p, Icon::Video, "До 4K / 60 fps");
                    Self::badge(ui, &p, Icon::Music, "MP3 и M4A");
                    Self::badge(ui, &p, Icon::Download, "Без лишних шагов");
                });
            };

            if wide {
                ui.horizontal(|ui| {
                    let art_w = 250.0;
                    let text_w = (ui.available_width() - art_w - 24.0).max(300.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(text_w, 280.0),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            ui.add_space(18.0);
                            body(ui);
                        },
                    );
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        Self::hero_art(ui, &p, art_w);
                    });
                });
            } else {
                Self::hero_art(ui, &p, 180.0);
                ui.add_space(8.0);
                body(ui);
            }
        });
    }

    fn hero_art(ui: &mut egui::Ui, p: &Palette, size: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let painter = ui.painter();
        let c = rect.center();
        let s = size;

        painter.circle_filled(c, s * 0.42, p.primary_container);
        painter.circle_filled(
            c + Vec2::new(s * 0.20, -s * 0.24),
            s * 0.115,
            p.tertiary_container,
        );
        painter.circle_filled(
            c + Vec2::new(-s * 0.28, s * 0.22),
            s * 0.075,
            p.secondary_container,
        );

        // “Device” with a play button.
        let screen = Rect::from_center_size(c + Vec2::new(0.0, -s * 0.02), Vec2::new(s * 0.52, s * 0.31));
        painter.rect_filled(screen, CornerRadius::same((s * 0.045) as u8), Color32::from_rgb(27, 22, 34));
        let tri = vec![
            screen.center() + Vec2::new(-s * 0.045, -s * 0.065),
            screen.center() + Vec2::new(s * 0.075, 0.0),
            screen.center() + Vec2::new(-s * 0.045, s * 0.065),
        ];
        painter.add(egui::Shape::convex_polygon(
            tri,
            p.primary_container,
            Stroke::NONE,
        ));

        // Download bubble.
        let bubble = c + Vec2::new(s * 0.24, s * 0.24);
        painter.circle_filled(bubble, s * 0.12, p.primary);
        icons::draw(
            painter,
            Rect::from_center_size(bubble, Vec2::splat(s * 0.13)),
            Icon::Download,
            p.on_primary,
            p.primary,
        );

        // Decorative ring.
        painter.circle_stroke(
            c + Vec2::new(-s * 0.14, -s * 0.20),
            s * 0.16,
            Stroke::new(2.0_f32, alpha(p.primary, 90)),
        );
    }

    fn video_card(&mut self, ui: &mut egui::Ui, video: &VideoInfo) {
        let p = self.palette;
        let mut download_now = false;
        let mut copy_link = false;
        let mut open_youtube = false;

        widgets::card(ui, p.surface_container_low, 28, 24, |ui| {
            let wide = ui.available_width() > 780.0;

            if wide {
                ui.horizontal_top(|ui| {
                    let thumb_w = (ui.available_width() * 0.37).clamp(280.0, 430.0);
                    self.thumbnail(ui, video, thumb_w);
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        self.video_details(
                            ui,
                            video,
                            &mut download_now,
                            &mut copy_link,
                            &mut open_youtube,
                        );
                    });
                });
            } else {
                let thumb_w = ui.available_width();
                self.thumbnail(ui, video, thumb_w);
                ui.add_space(18.0);
                self.video_details(
                    ui,
                    video,
                    &mut download_now,
                    &mut copy_link,
                    &mut open_youtube,
                );
            }
        });

        if open_youtube {
            let target = if video.url.is_empty() {
                self.url.clone()
            } else {
                video.url.clone()
            };
            ytdlp::open_path(&target);
        }
        if copy_link {
            let target = if video.url.is_empty() {
                self.url.clone()
            } else {
                video.url.clone()
            };
            ui.ctx().copy_text(target);
            self.toast("Ссылка скопирована", false);
        }
        if download_now {
            self.queue_download();
        }
    }

    fn thumbnail(&self, ui: &mut egui::Ui, video: &VideoInfo, width: f32) {
        let p = self.palette;
        let height = width * 9.0 / 16.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(20), p.surface_container_highest);

        if let Some(tex) = &self.thumb {
            let size = tex.size_vec2();
            let image_aspect = size.x / size.y.max(1.0);
            let target_aspect = rect.width() / rect.height().max(1.0);
            let (u, v) = if image_aspect > target_aspect {
                let u = target_aspect / image_aspect;
                (u, 1.0)
            } else {
                let v = image_aspect / target_aspect;
                (1.0, v)
            };
            let uv = Rect::from_min_max(
                Pos2::new((1.0 - u) / 2.0, (1.0 - v) / 2.0),
                Pos2::new(1.0 - (1.0 - u) / 2.0, 1.0 - (1.0 - v) / 2.0),
            );
            painter.image(tex.id(), rect, uv, Color32::WHITE);
        } else {
            painter.circle_filled(rect.center(), height * 0.18, alpha(p.primary, 40));
            icons::draw(
                painter,
                Rect::from_center_size(rect.center(), Vec2::splat(height * 0.2)),
                Icon::Play,
                p.primary,
                p.surface_container_highest,
            );
        }

        if let Some(duration) = video.duration {
            let text = human_duration(Some(duration));
            let galley = painter.layout_no_wrap(
                text,
                font_medium(12.0),
                Color32::from_rgb(255, 255, 255),
            );
            let badge = Rect::from_min_size(
                Pos2::new(
                    rect.max.x - galley.size().x - 20.0,
                    rect.max.y - galley.size().y - 16.0,
                ),
                galley.size() + Vec2::new(12.0, 6.0),
            );
            painter.rect_filled(
                badge.translate(Vec2::new(-4.0, -2.0)),
                CornerRadius::same(6),
                Color32::from_black_alpha(190),
            );
            painter.galley(badge.min + Vec2::new(2.0, 3.0), galley, Color32::WHITE);
        }
    }

    fn video_details(
        &mut self,
        ui: &mut egui::Ui,
        video: &VideoInfo,
        download_now: &mut bool,
        copy_link: &mut bool,
        open_youtube: &mut bool,
    ) {
        let p = self.palette;
        ui.horizontal(|ui| {
            let initial = video
                .channel
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| "?".to_owned());
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 12.0, p.tertiary_container);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                initial,
                font_medium(11.0),
                p.on_tertiary_container,
            );
            ui.label(
                RichText::new(widgets::ellipsize(&video.channel, 42))
                    .font(font_medium(13.0))
                    .color(p.on_surface_variant),
            );
        });
        ui.add_space(6.0);
        ui.add(
            egui::Label::new(
                RichText::new(&video.title)
                    .font(font_bold(23.0))
                    .color(p.on_surface),
            )
            .wrap(),
        );
        ui.add_space(8.0);

        let mut stats = Vec::new();
        if let Some(d) = video.duration {
            stats.push(human_duration(Some(d)));
        }
        if let Some(views) = video.views {
            stats.push(human_count(views, "просмотр", "просмотра", "просмотров"));
        }
        if let Some(likes) = video.likes {
            stats.push(format!(
                "{} {}",
                human_number(likes),
                plural(likes, "лайк", "лайка", "лайков")
            ));
        }
        ui.label(
            RichText::new(stats.join("  ·  "))
                .font(font(13.0))
                .color(p.on_surface_variant),
        );

        ui.add_space(18.0);

        if self.kind == DownloadKind::Video {
            widgets::section_label(ui, &p, "КАЧЕСТВО");
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let best = widgets::chip(ui, &p, "Лучшее", self.selected_height.is_none(), true);
                if best.clicked() {
                    self.selected_height = None;
                }
                for q in &video.qualities {
                    let selected = self.selected_height == Some(q.height);
                    if widgets::chip(ui, &p, &q.label(), selected, true).clicked() {
                        self.selected_height = Some(q.height);
                    }
                }
            });
            ui.add_space(12.0);
        }

        widgets::section_label(ui, &p, "ФОРМАТ");
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for kind in [
                DownloadKind::Video,
                DownloadKind::AudioM4a,
                DownloadKind::AudioMp3,
            ] {
                if widgets::chip(ui, &p, kind.label(), self.kind == kind, true).clicked() {
                    self.kind = kind;
                }
            }
        });

        if self.kind != DownloadKind::Video {
            ui.add_space(6.0);
            ui.label(
                RichText::new("Будет загружена лучшая доступная аудиодорожка.")
                    .font(font(12.0))
                    .color(p.on_surface_variant),
            );
        }

        let size = match self.kind {
            DownloadKind::Video => match self.selected_height {
                Some(h) => video
                    .qualities
                    .iter()
                    .find(|q| q.height == h)
                    .and_then(|q| q.total_size),
                None => video.best_size,
            },
            DownloadKind::AudioM4a => video.audio_size,
            DownloadKind::AudioMp3 => video.audio_size.map(|s| (s as f64 * 1.35) as u64),
        };

        ui.add_space(20.0);
        ui.horizontal(|ui| {
            let label = match size {
                Some(size) => format!("Скачать · {}", human_size(size)),
                None => "Скачать".to_owned(),
            };
            if widgets::button(ui, &p, ButtonKind::Filled, &label, Some(Icon::Download), true)
                .clicked()
            {
                *download_now = true;
            }
            if widgets::button(
                ui,
                &p,
                ButtonKind::Tonal,
                "На YouTube",
                Some(Icon::Open),
                true,
            )
            .clicked()
            {
                *open_youtube = true;
            }
            if widgets::icon_button(ui, &p, Icon::Copy, "Скопировать ссылку", 40.0, None).clicked() {
                *copy_link = true;
            }
        });
    }

    fn downloads_section(&mut self, ui: &mut egui::Ui) {
        if self.downloads.is_empty() {
            return;
        }
        let p = self.palette;
        let mut open_dir = false;
        let mut clear_finished = false;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Загрузки")
                    .font(font_medium(20.0))
                    .color(p.on_surface),
            );
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            let _ = rect;
            let count = self.downloads.len();
            let text = count.to_string();
            let galley = ui
                .painter()
                .layout_no_wrap(text, font_medium(11.5), p.on_secondary_container);
            let (badge, _) = ui.allocate_exact_size(
                Vec2::new(galley.size().x + 16.0, 22.0),
                Sense::hover(),
            );
            ui.painter()
                .rect_filled(badge, CornerRadius::same(11), p.secondary_container);
            ui.painter().galley(
                badge.center() - galley.size() / 2.0,
                galley,
                p.on_secondary_container,
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, &p, ButtonKind::Text, "Открыть папку", None, true).clicked()
                {
                    open_dir = true;
                }
                if self.downloads.iter().any(|d| d.status.is_finished())
                    && widgets::button(ui, &p, ButtonKind::Text, "Убрать завершённые", None, true)
                        .clicked()
                {
                    clear_finished = true;
                }
            });
        });
        ui.add_space(10.0);

        let items: Vec<DownloadItem> = self.downloads.clone();
        let mut actions: Vec<RowAction> = Vec::new();

        for item in &items {
            widgets::card(ui, p.surface_container_low, 20, 16, |ui| {
                ui.horizontal(|ui| {
                    let status_color = match item.status {
                        DownloadStatus::Done => p.tertiary_container,
                        DownloadStatus::Failed => p.error_container,
                        DownloadStatus::Cancelled => p.surface_container_highest,
                        _ => p.secondary_container,
                    };
                    let status_fg = match item.status {
                        DownloadStatus::Done => p.on_tertiary_container,
                        DownloadStatus::Failed => p.on_error_container,
                        DownloadStatus::Cancelled => p.on_surface_variant,
                        _ => p.on_secondary_container,
                    };
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 23.0, status_color);
                    let icon = match item.status {
                        DownloadStatus::Done => Icon::Check,
                        DownloadStatus::Failed => Icon::Alert,
                        DownloadStatus::Cancelled => Icon::Close,
                        _ => item.kind.icon(),
                    };
                    icons::draw(
                        ui.painter(),
                        Rect::from_center_size(rect.center(), Vec2::splat(22.0)),
                        icon,
                        status_fg,
                        status_color,
                    );

                    let reserved = 190.0;
                    let text_w = (ui.available_width() - reserved).max(160.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(text_w, 44.0),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            ui.label(
                                RichText::new(widgets::ellipsize(&item.title, 64))
                                    .font(font_medium(15.0))
                                    .color(p.on_surface),
                            );
                            let mut meta = item.quality_label();
                            match item.status {
                                DownloadStatus::Active => {
                                    meta.push_str(" · ");
                                    meta.push_str(&item.phase);
                                    if let Some(speed) = &item.speed {
                                        meta.push_str(" · ");
                                        meta.push_str(speed);
                                    }
                                    if let Some(eta) = &item.eta {
                                        meta.push_str(" · осталось ");
                                        meta.push_str(eta);
                                    }
                                }
                                DownloadStatus::Queued => meta.push_str(" · в очереди"),
                                DownloadStatus::Done => {
                                    if let Some(size) = item
                                        .path
                                        .as_ref()
                                        .and_then(|p| std::fs::metadata(p).ok())
                                        .map(|m| m.len())
                                    {
                                        meta.push_str(" · ");
                                        meta.push_str(&human_size(size));
                                    }
                                }
                                DownloadStatus::Failed => {
                                    meta.push_str(" · ");
                                    meta.push_str(
                                        item.error
                                            .as_deref()
                                            .map(|e| widgets::ellipsize(e, 52))
                                            .unwrap_or_else(|| "ошибка".to_owned())
                                            .as_str(),
                                    );
                                }
                                DownloadStatus::Cancelled => meta.push_str(" · отменено"),
                            }
                            ui.label(
                                RichText::new(meta)
                                    .font(font(12.0))
                                    .color(if item.status == DownloadStatus::Failed {
                                        p.error
                                    } else {
                                        p.on_surface_variant
                                    }),
                            );
                        },
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        match item.status {
                            DownloadStatus::Active | DownloadStatus::Queued => {
                                if widgets::icon_button(
                                    ui,
                                    &p,
                                    Icon::Close,
                                    "Отменить",
                                    36.0,
                                    Some(p.error),
                                )
                                .clicked()
                                {
                                    actions.push(RowAction::Cancel(item.id));
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "{}%",
                                        (item.progress * 100.0).round() as u32
                                    ))
                                    .font(font_medium(14.0))
                                    .color(p.primary),
                                );
                            }
                            DownloadStatus::Done => {
                                if let Some(path) = &item.path {
                                    if widgets::icon_button(ui, &p, Icon::Open, "Открыть файл", 36.0, None)
                                        .clicked()
                                    {
                                        actions.push(RowAction::OpenFile(path.clone()));
                                    }
                                    if widgets::icon_button(ui, &p, Icon::Folder, "Папка", 36.0, None)
                                        .clicked()
                                    {
                                        actions.push(RowAction::OpenFolder(path.clone()));
                                    }
                                }
                                if widgets::icon_button(ui, &p, Icon::Trash, "Убрать", 36.0, None)
                                    .clicked()
                                {
                                    actions.push(RowAction::Remove(item.id));
                                }
                            }
                            DownloadStatus::Failed | DownloadStatus::Cancelled => {
                                if widgets::icon_button(
                                    ui,
                                    &p,
                                    Icon::Refresh,
                                    "Повторить",
                                    36.0,
                                    Some(p.primary),
                                )
                                .clicked()
                                {
                                    actions.push(RowAction::Retry(item.id));
                                }
                                if widgets::icon_button(ui, &p, Icon::Trash, "Убрать", 36.0, None)
                                    .clicked()
                                {
                                    actions.push(RowAction::Remove(item.id));
                                }
                            }
                        }
                    });
                });

                if item.status == DownloadStatus::Active {
                    ui.add_space(10.0);
                    widgets::progress(ui, &p, item.progress);
                }
            });
            ui.add_space(8.0);
        }

        if open_dir {
            let dir = self.cfg.ensure_download_dir();
            ytdlp::open_path(&dir.to_string_lossy());
        }
        if clear_finished {
            self.downloads.retain(|d| !d.status.is_finished());
        }
        for action in actions {
            match action {
                RowAction::Cancel(id) => self.cancel_download(id),
                RowAction::Retry(id) => self.retry_download(id),
                RowAction::Remove(id) => self.downloads.retain(|d| d.id != id),
                RowAction::OpenFile(path) => ytdlp::open_path(&path),
                RowAction::OpenFolder(path) => {
                    let parent = PathBuf::from(path)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| self.cfg.download_dir.clone());
                    ytdlp::open_path(&parent);
                }
                RowAction::OpenUrl(url) => ytdlp::open_path(&url),
            }
        }
    }

    fn cancel_download(&mut self, id: u64) {
        if let Some(pid) = self.pids.remove(&id) {
            ytdlp::cancel_process(pid);
        }
        self.active.remove(&id);
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = DownloadStatus::Cancelled;
            item.phase = "Отменено".to_owned();
            item.speed = None;
            item.eta = None;
        }
        self.toast("Загрузка отменена", false);
    }

    fn retry_download(&mut self, id: u64) {
        let mut queued = false;
        if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
            item.status = DownloadStatus::Queued;
            item.progress = 0.0;
            item.phase = "В очереди".to_owned();
            item.error = None;
            item.path = None;
            item.speed = None;
            item.eta = None;
            queued = true;
        }
        if queued {
            self.queue.push_back(id);
        }
    }

    // -----------------------------------------------------------------------
    // Library page
    // -----------------------------------------------------------------------

    fn page_library(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let total = self.cfg.history.len();
        self.page_header(
            ui,
            "Библиотека",
            &format!(
                "{} {}",
                total,
                plural(total as u64, "загруженный файл", "загруженных файла", "загруженных файлов")
            ),
        );
        ui.add_space(22.0);

        let mut clear_all = false;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 140.0).max(220.0);
            ui.allocate_ui_with_layout(
                Vec2::new(width, 56.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    widgets::text_field(
                        ui,
                        &p,
                        "library-search",
                        &mut self.library_query,
                        "Поиск по названию или каналу",
                        Some(Icon::Search),
                    );
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.cfg.history.is_empty()
                    && widgets::button(ui, &p, ButtonKind::Text, "Очистить", Some(Icon::Trash), true)
                        .clicked()
                {
                    clear_all = true;
                }
            });
        });
        ui.add_space(18.0);

        if clear_all {
            self.cfg.history.clear();
            self.cfg.save();
        }

        let query = self.library_query.to_lowercase();
        let items: Vec<HistoryItem> = self
            .cfg
            .history
            .iter()
            .filter(|h| {
                query.is_empty()
                    || h.title.to_lowercase().contains(&query)
                    || h.channel.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        if items.is_empty() {
            self.library_empty(ui, !self.cfg.history.is_empty());
            return;
        }

        let mut actions: Vec<RowAction> = Vec::new();
        let mut remove: Option<String> = None;

        for item in &items {
            widgets::card(ui, p.surface_container_low, 20, 16, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 23.0, p.secondary_container);
                    let icon = if item.kind.starts_with("Аудио") {
                        Icon::Music
                    } else {
                        Icon::Video
                    };
                    icons::draw(
                        ui.painter(),
                        Rect::from_center_size(rect.center(), Vec2::splat(22.0)),
                        icon,
                        p.on_secondary_container,
                        p.secondary_container,
                    );
                    let reserved = 220.0;
                    let text_w = (ui.available_width() - reserved).max(160.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(text_w, 44.0),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            ui.label(
                                RichText::new(widgets::ellipsize(&item.title, 70))
                                    .font(font_medium(15.0))
                                    .color(p.on_surface),
                            );
                            let mut meta = item.channel.clone();
                            if !item.kind.is_empty() {
                                meta.push_str("  ·  ");
                                meta.push_str(&item.kind);
                            }
                            meta.push_str("  ·  ");
                            meta.push_str(&item.when());
                            ui.label(
                                RichText::new(meta)
                                    .font(font(12.0))
                                    .color(p.on_surface_variant),
                            );
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, &p, Icon::Trash, "Убрать из истории", 36.0, None)
                            .clicked()
                        {
                            remove = Some(item.path.clone());
                        }
                        if widgets::icon_button(ui, &p, Icon::Folder, "Открыть папку", 36.0, None)
                            .clicked()
                        {
                            actions.push(RowAction::OpenFolder(item.path.clone()));
                        }
                        if widgets::icon_button(ui, &p, Icon::Open, "Открыть файл", 36.0, None)
                            .clicked()
                        {
                            actions.push(RowAction::OpenFile(item.path.clone()));
                        }
                        if let Some(size) = item.size {
                            ui.label(
                                RichText::new(human_size(size))
                                    .font(font_medium(12.0))
                                    .color(p.on_surface_variant),
                            );
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }

        if let Some(path) = remove {
            self.cfg.history.retain(|h| h.path != path);
            self.cfg.save();
        }
        for action in actions {
            match action {
                RowAction::OpenFile(path) => ytdlp::open_path(&path),
                RowAction::OpenFolder(path) => {
                    let parent = PathBuf::from(path)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| self.cfg.download_dir.clone());
                    ytdlp::open_path(&parent);
                }
                RowAction::OpenUrl(url) => ytdlp::open_path(&url),
                _ => {}
            }
        }
    }

    fn library_empty(&mut self, ui: &mut egui::Ui, filtered: bool) {
        let p = self.palette;
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(120.0), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 60.0, p.surface_container_high);
            icons::draw(
                ui.painter(),
                Rect::from_center_size(rect.center(), Vec2::splat(48.0)),
                if filtered { Icon::Search } else { Icon::Library },
                p.on_surface_variant,
                p.surface_container_high,
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(if filtered {
                    "Ничего не найдено"
                } else {
                    "Библиотека пока пуста"
                })
                .font(font_medium(20.0))
                .color(p.on_surface),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(if filtered {
                    "Попробуйте изменить запрос"
                } else {
                    "Скачанные видео и аудио появятся здесь"
                })
                .font(font(13.5))
                .color(p.on_surface_variant),
            );
            if !filtered {
                ui.add_space(18.0);
                if widgets::button(
                    ui,
                    &p,
                    ButtonKind::Tonal,
                    "Перейти к загрузкам",
                    Some(Icon::Download),
                    true,
                )
                .clicked()
                {
                    self.page = Page::Download;
                }
            }
        });
    }

    // -----------------------------------------------------------------------
    // Settings page
    // -----------------------------------------------------------------------

    fn page_settings(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        self.page_header(ui, "Настройки", "Внешний вид, папка загрузок и инструменты");
        ui.add_space(22.0);

        let mut changed = false;
        let mut pick_folder = false;
        let mut check_versions = false;
        let mut update_ytdlp = false;
        let mut open_gist = false;

        // Appearance -------------------------------------------------------
        widgets::card(ui, p.surface_container_low, 24, 22, |ui| {
            ui.label(
                RichText::new("Внешний вид")
                    .font(font_medium(18.0))
                    .color(p.on_surface),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Material You строит палитру из выбранного цвета.")
                    .font(font(13.0))
                    .color(p.on_surface_variant),
            );
            ui.add_space(14.0);

            ui.label(
                RichText::new("Тема")
                    .font(font_medium(12.0))
                    .color(p.primary),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for mode in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
                    if widgets::chip(ui, &p, mode.label(), self.cfg.theme == mode, true).clicked() {
                        self.cfg.theme = mode;
                        self.dark = resolve_dark(mode);
                        self.palette = Palette::from_seed(self.cfg.seed, self.dark);
                        self.theme_seed = self.cfg.seed;
                        changed = true;
                    }
                }
            });

            ui.add_space(16.0);
            ui.label(
                RichText::new("Цвет-источник")
                    .font(font_medium(12.0))
                    .color(p.primary),
            );
            ui.add_space(8.0);
            const SEEDS: [[u8; 3]; 8] = [
                [103, 80, 164],
                [255, 0, 51],
                [255, 109, 0],
                [255, 193, 7],
                [46, 125, 50],
                [0, 150, 136],
                [25, 118, 210],
                [194, 24, 91],
            ];
            ui.horizontal_wrapped(|ui| {
                for seed in SEEDS {
                    let (rect, response) = ui.allocate_exact_size(Vec2::splat(42.0), Sense::click());
                    let color = Color32::from_rgb(seed[0], seed[1], seed[2]);
                    let painter = ui.painter();
                    if response.hovered() {
                        painter.circle_filled(rect.center(), 21.0, alpha(color, 50));
                    }
                    painter.circle_filled(rect.center(), 16.0, color);
                    if self.cfg.seed == seed {
                        painter.circle_stroke(
                            rect.center(),
                            19.5,
                            Stroke::new(2.5_f32, p.on_surface),
                        );
                        icons::draw(
                            painter,
                            Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
                            Icon::Check,
                            p.on_surface,
                            color,
                        );
                    }
                    if response.clicked() {
                        self.cfg.seed = seed;
                        self.dark = resolve_dark(self.cfg.theme);
                        self.palette = Palette::from_seed(seed, self.dark);
                        self.theme_seed = seed;
                        changed = true;
                    }
                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
            });

            ui.add_space(16.0);
            let colors = [
                (p.primary, "primary"),
                (p.secondary, "secondary"),
                (p.tertiary, "tertiary"),
                (p.error, "error"),
                (p.surface_container_highest, "surface"),
            ];
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
            let painter = ui.painter();
            let gap = 6.0;
            let w = (rect.width() - gap * (colors.len() as f32 - 1.0)) / colors.len() as f32;
            for (i, (color, _)) in colors.iter().enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(rect.min.x + i as f32 * (w + gap), rect.min.y),
                    Vec2::new(w, 18.0),
                );
                painter.rect_filled(r, CornerRadius::same(9), *color);
            }
        });

        ui.add_space(14.0);

        // Downloads --------------------------------------------------------
        widgets::card(ui, p.surface_container_low, 24, 22, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Загрузки")
                        .font(font_medium(18.0))
                        .color(p.on_surface),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(
                        ui,
                        &p,
                        ButtonKind::Tonal,
                        "Выбрать папку",
                        Some(Icon::Folder),
                        true,
                    )
                    .clicked()
                    {
                        pick_folder = true;
                    }
                });
            });
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 56.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    let response = widgets::text_field(
                        ui,
                        &p,
                        "download-dir",
                        &mut self.cfg.download_dir,
                        "Папка для сохранения",
                        Some(Icon::Folder),
                    );
                    if response.lost_focus() {
                        let _ = self.cfg.ensure_download_dir();
                        changed = true;
                    }
                },
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (label, dir) in [
                    ("Видео", dirs::video_dir()),
                    ("Загрузки", dirs::download_dir()),
                    ("Музыка", dirs::audio_dir()),
                    ("Документы", dirs::document_dir()),
                ] {
                    if let Some(dir) = dir {
                        if widgets::chip(ui, &p, label, false, true).clicked() {
                            self.cfg.download_dir = dir.to_string_lossy().into_owned();
                            let _ = self.cfg.ensure_download_dir();
                            changed = true;
                        }
                    }
                }
            });

            ui.add_space(16.0);
            ui.label(
                RichText::new("Одновременных загрузок")
                    .font(font_medium(12.0))
                    .color(p.primary),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for n in 1..=4usize {
                    if widgets::chip(ui, &p, &n.to_string(), self.cfg.concurrency == n, true)
                        .clicked()
                    {
                        self.cfg.concurrency = n;
                        changed = true;
                    }
                }
            });

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Открывать папку после загрузки")
                            .font(font_medium(14.0))
                            .color(p.on_surface),
                    );
                    ui.label(
                        RichText::new("Проводник откроется автоматически")
                            .font(font(12.0))
                            .color(p.on_surface_variant),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::switch(ui, &p, &mut self.cfg.open_folder_after).changed() {
                        changed = true;
                    }
                });
            });

            ui.add_space(16.0);
            ui.label(
                RichText::new("Cookies из браузера")
                    .font(font_medium(12.0))
                    .color(p.primary),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new("Нужно для возрастных или приватных видео")
                    .font(font(12.0))
                    .color(p.on_surface_variant),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (label, value) in [
                    ("Не использовать", ""),
                    ("Firefox", "firefox"),
                    ("Chrome", "chrome"),
                    ("Chromium", "chromium"),
                    ("Brave", "brave"),
                ] {
                    if widgets::chip(
                        ui,
                        &p,
                        label,
                        self.cfg.cookies_browser == value,
                        true,
                    )
                    .clicked()
                    {
                        self.cfg.cookies_browser = value.to_owned();
                        changed = true;
                    }
                }
            });
        });

        ui.add_space(14.0);

        // Tools ------------------------------------------------------------
        widgets::card(ui, p.surface_container_low, 24, 22, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Инструменты")
                        .font(font_medium(18.0))
                        .color(p.on_surface),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, &p, ButtonKind::Text, "Обновить yt-dlp", None, true)
                        .clicked()
                    {
                        update_ytdlp = true;
                    }
                    if widgets::icon_button(
                        ui,
                        &p,
                        Icon::Refresh,
                        "Проверить снова",
                        40.0,
                        None,
                    )
                    .clicked()
                    {
                        check_versions = true;
                    }
                });
            });
            ui.add_space(12.0);

            match &self.versions {
                None => {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(16.0).color(p.primary));
                        ui.label(
                            RichText::new("Проверяем yt-dlp и ffmpeg…")
                                .font(font(13.0))
                                .color(p.on_surface_variant),
                        );
                    });
                }
                Some(Err(err)) => {
                    tool_row(ui, &p, "yt-dlp", err, false);
                    ui.add_space(6.0);
                    tool_row(ui, &p, "ffmpeg", "не найден", false);
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Установка: sudo pacman -S yt-dlp ffmpeg")
                            .font(font(12.5))
                            .color(p.on_surface_variant),
                    );
                }
                Some(Ok(versions)) => {
                    tool_row(ui, &p, "yt-dlp", &versions.ytdlp, true);
                    ui.add_space(6.0);
                    let ffmpeg_ok = versions.ffmpeg != "не найден";
                    tool_row(ui, &p, "ffmpeg", &versions.ffmpeg, ffmpeg_ok);
                }
            }

            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "YouDown вызывает yt-dlp для анализа ссылок и загрузки, а ffmpeg — для склейки видео и аудио.",
                )
                .font(font(12.0))
                .color(p.on_surface_variant),
            );
        });

        ui.add_space(14.0);

        // About ------------------------------------------------------------
        widgets::card(ui, p.surface_container_low, 24, 22, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                ui.painter()
                    .circle_filled(rect.center(), 28.0, p.primary);
                icons::draw(
                    ui.painter(),
                    Rect::from_center_size(rect.center(), Vec2::splat(26.0)),
                    Icon::Play,
                    p.on_primary,
                    p.primary,
                );
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("YouDown 1.0.0")
                            .font(font_medium(17.0))
                            .color(p.on_surface),
                    );
                    ui.label(
                        RichText::new("Material You · Rust · yt-dlp")
                            .font(font(12.5))
                            .color(p.on_surface_variant),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, &p, ButtonKind::Text, "Инструкция", Some(Icon::Open), true)
                        .clicked()
                    {
                        open_gist = true;
                    }
                });
            });
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Скачивайте только тот контент, на который у вас есть права. Приложение не обходит ограничения YouTube.",
                )
                .font(font(12.0))
                .color(p.on_surface_variant),
            );
        });

        if changed {
            self.cfg.save();
        }
        if pick_folder {
            ytdlp::pick_folder(self.tx.clone(), self.cfg.download_dir.clone());
        }
        if check_versions {
            self.versions = None;
            self.versions_loading = true;
            ytdlp::spawn_versions(self.tx.clone());
        }
        let _ = self.versions_loading;
        if update_ytdlp {
            ytdlp::update_ytdlp(self.tx.clone());
        }
        if open_gist {
            ytdlp::open_path("https://gist.github.com/vladimir-vg/792d273f3e1dbc22d2cb5d941537a785");
        }
    }

    // -----------------------------------------------------------------------
    // Snackbar
    // -----------------------------------------------------------------------

    fn snackbar(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else {
            return;
        };
        if toast.at.elapsed() > Duration::from_secs(5) {
            self.toast = None;
            return;
        }
        let p = self.palette;
        let (bg, fg, icon) = if toast.error {
            (
                p.error_container,
                p.on_error_container,
                Icon::Alert,
            )
        } else {
            (p.inverse_surface, p.inverse_on_surface, Icon::Check)
        };

        egui::Area::new(egui::Id::new("youdown-snackbar"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -24.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::symmetric(18, 14))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(90),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            icons::draw(
                                ui.painter(),
                                Rect::from_center_size(
                                    ui.next_widget_position() + Vec2::new(9.0, 9.0),
                                    Vec2::splat(18.0),
                                ),
                                icon,
                                fg,
                                bg,
                            );
                            ui.add_space(30.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(widgets::ellipsize(&toast.text, 110))
                                        .font(font_medium(13.5))
                                        .color(fg),
                                )
                                .wrap(),
                            );
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(150));
    }

    // -----------------------------------------------------------------------
    // Frame
    // -----------------------------------------------------------------------

    fn nav_rail(&mut self, ctx: &egui::Context) {
        let p = self.palette;
        egui::SidePanel::left("youdown-nav")
            .exact_width(96.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(p.surface_container_low)
                    .inner_margin(Margin::symmetric(8, 18)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 27.0, p.primary);
                    icons::draw(
                        ui.painter(),
                        Rect::from_center_size(
                            rect.center() + Vec2::new(1.5, 0.0),
                            Vec2::splat(25.0),
                        ),
                        Icon::Play,
                        p.on_primary,
                        p.primary,
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("YouDown")
                            .font(font_medium(10.5))
                            .color(p.on_surface_variant),
                    );
                });
                ui.add_space(22.0);

                ui.vertical_centered(|ui| {
                    if widgets::nav_item(ui, &p, Icon::Download, "Загрузки", self.page == Page::Download)
                        .clicked()
                    {
                        self.page = Page::Download;
                    }
                    ui.add_space(4.0);
                    if widgets::nav_item(ui, &p, Icon::Library, "Библиотека", self.page == Page::Library)
                        .clicked()
                    {
                        self.page = Page::Library;
                    }
                    ui.add_space(4.0);
                    if widgets::nav_item(ui, &p, Icon::Settings, "Настройки", self.page == Page::Settings)
                        .clicked()
                    {
                        self.page = Page::Settings;
                    }
                });

                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    let icon = if self.dark { Icon::Sun } else { Icon::Moon };
                    if widgets::icon_button(
                        ui,
                        &p,
                        icon,
                        if self.dark {
                            "Светлая тема"
                        } else {
                            "Тёмная тема"
                        },
                        44.0,
                        None,
                    )
                    .clicked()
                    {
                        self.set_theme(if self.dark {
                            ThemeMode::Light
                        } else {
                            ThemeMode::Dark
                        });
                    }
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("v1.0")
                            .font(font(10.0))
                            .color(alpha(p.on_surface_variant, 150)),
                    );
                });
            });
    }
}

impl eframe::App for YouDown {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events(ctx);
        self.schedule();
        self.sync_theme(ctx);
        let p = self.palette;

        self.nav_rail(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(Margin {
                        left: 36,
                        right: 36,
                        top: 26,
                        bottom: 26,
                    }),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Self::centered(ui, 1060.0, |ui| match self.page {
                            Page::Download => self.page_download(ui),
                            Page::Library => self.page_library(ui),
                            Page::Settings => self.page_settings(ui),
                        });
                        ui.add_space(24.0);
                    });
            });

        self.snackbar(ctx);

        if self.analyzing || !self.active.is_empty() || !self.queue.is_empty() || self.toast.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }
}

impl Drop for YouDown {
    fn drop(&mut self) {
        self.cfg.save();
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

fn plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let mod100 = n % 100;
    let mod10 = n % 10;
    if (11..=14).contains(&mod100) {
        many
    } else if mod10 == 1 {
        one
    } else if (2..=4).contains(&mod10) {
        few
    } else {
        many
    }
}

fn tool_row(ui: &mut egui::Ui, p: &Palette, name: &str, version: &str, ok: bool) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
        let color = if ok { p.tertiary } else { p.error };
        ui.painter().circle_filled(rect.center(), 5.0, color);
        ui.add_space(6.0);
        ui.label(
            RichText::new(name)
                .font(font_medium(13.5))
                .color(p.on_surface),
        );
        ui.label(
            RichText::new(version)
                .font(font(13.0))
                .color(p.on_surface_variant),
        );
        if !ok {
            ui.label(
                RichText::new("нет в PATH")
                    .font(font(12.0))
                    .color(p.error),
            );
        }
    });
}

fn resolve_dark(mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => system_is_dark(),
    }
}

#[cfg(not(windows))]
fn system_is_dark() -> bool {
    use std::process::Command;

    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("prefer-dark") {
            return true;
        }
        if text.contains("prefer-light") {
            return false;
        }
    }
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        if text.contains("dark") {
            return true;
        }
    }
    if let Ok(out) = Command::new("kreadconfig6")
        .args(["--group", "General", "--key", "ColorScheme"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        if text.contains("dark") {
            return true;
        }
        if text.contains("light") {
            return false;
        }
    }
    true
}

#[cfg(windows)]
fn system_is_dark() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let mut cmd = Command::new("reg");
    cmd.args([
        "query",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "/v",
        "AppsUseLightTheme",
    ]);
    cmd.creation_flags(0x0800_0000);
    if let Ok(out) = cmd.output() {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = text.lines().find(|l| l.contains("AppsUseLightTheme")) {
            if line.contains("0x0") {
                return true;
            }
            if line.contains("0x1") {
                return false;
            }
        }
    }
    false
}
