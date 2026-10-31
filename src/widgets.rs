//! Material 3 widgets drawn on top of egui primitives.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

use crate::icons::{self, Icon};
use crate::theme::{alpha, blend, font, font_medium, Palette};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Filled,
    Tonal,
    Outlined,
    Text,
    Danger,
}

pub const CONTROL_HEIGHT: f32 = 40.0;

fn hover_t(ui: &Ui, response: &Response, open: bool) -> f32 {
    ui.ctx()
        .animate_bool_with_time(response.id.with("hover"), open, 0.12)
}

fn pressed_t(ui: &Ui, response: &Response) -> f32 {
    ui.ctx().animate_bool_with_time(
        response.id.with("press"),
        response.is_pointer_button_down_on(),
        0.06,
    )
}

pub fn button(
    ui: &mut Ui,
    p: &Palette,
    kind: ButtonKind,
    label: &str,
    icon: Option<Icon>,
    enabled: bool,
) -> Response {
    let f = font_medium(14.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), f.clone(), p.on_surface);
    let icon_w = if icon.is_some() { 18.0 } else { 0.0 };
    let gap = if icon.is_some() { 9.0 } else { 0.0 };
    let pad_x = if kind == ButtonKind::Text { 14.0 } else { 20.0 };
    let width = galley.size().x + icon_w + gap + pad_x * 2.0;
    let height = CONTROL_HEIGHT;

    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let (bg, fg, border) = match kind {
        ButtonKind::Filled => (p.primary, p.on_primary, None),
        ButtonKind::Tonal => (
            p.secondary_container,
            p.on_secondary_container,
            None,
        ),
        ButtonKind::Outlined => (Color32::TRANSPARENT, p.primary, Some(p.outline)),
        ButtonKind::Text => (Color32::TRANSPARENT, p.primary, None),
        ButtonKind::Danger => (p.error, p.on_error, None),
    };

    let (bg, fg, border) = if enabled {
        (bg, fg, border)
    } else {
        (
            match kind {
                ButtonKind::Filled | ButtonKind::Danger => p.disabled_container(),
                ButtonKind::Tonal => alpha(p.on_surface, 12),
                _ => Color32::TRANSPARENT,
            },
            p.disabled_on(),
            border.map(|_| alpha(p.on_surface, 12)),
        )
    };

    let hover = hover_t(ui, &response, response.hovered() && enabled);
    let pressed = pressed_t(ui, &response);
    let state = (hover * 0.08 + pressed * 0.10).min(0.18);

    let painter = ui.painter();
    let radius = CornerRadius::same((height / 2.0) as u8);
    if bg != Color32::TRANSPARENT || state > 0.0 {
        painter.rect_filled(rect, radius, bg);
    }
    if state > 0.0 {
        let layer = match kind {
            ButtonKind::Filled | ButtonKind::Danger => Color32::WHITE,
            _ => p.primary,
        };
        painter.rect_filled(rect, radius, alpha(layer, (state * 255.0) as u8));
    }
    if let Some(border) = border {
        painter.rect_stroke(rect, radius, Stroke::new(1.0_f32, border), StrokeKind::Inside);
    }

    let mut x = rect.center().x - (galley.size().x + icon_w + gap) / 2.0;
    if let Some(icon) = icon {
        let icon_rect = Rect::from_center_size(
            Pos2::new(x + icon_w / 2.0, rect.center().y),
            Vec2::splat(18.0),
        );
        icons::draw(painter, icon_rect, icon, fg, bg);
        x += icon_w + gap;
    }
    painter.galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );

    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

pub fn icon_button(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    tooltip: &str,
    size: f32,
    tint: Option<Color32>,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hover = hover_t(ui, &response, response.hovered());
    let pressed = pressed_t(ui, &response);
    let color = tint.unwrap_or(p.on_surface_variant);
    let painter = ui.painter();
    let layer = (hover * 0.10 + pressed * 0.12).min(0.20);
    if layer > 0.0 {
        painter.circle_filled(rect.center(), size / 2.0, alpha(color, (layer * 255.0) as u8));
    }
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(size * 0.52));
    icons::draw(painter, icon_rect, icon, color, p.surface);
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

/// Navigation rail destination: pill behind icon + label below.
pub fn nav_item(ui: &mut Ui, p: &Palette, icon: Icon, label: &str, active: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(80.0, 58.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hover = hover_t(ui, &response, response.hovered());
    let painter = ui.painter();

    let pill = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.min.y + 16.0),
        Vec2::new(64.0, 32.0),
    );
    if active {
        painter.rect_filled(pill, CornerRadius::same(16), p.secondary_container);
    } else if hover > 0.0 {
        painter.rect_filled(
            pill,
            CornerRadius::same(16),
            alpha(p.on_surface, (hover * 22.0) as u8),
        );
    }
    let fg = if active {
        p.on_secondary_container
    } else {
        p.on_surface_variant
    };
    icons::draw(
        painter,
        Rect::from_center_size(pill.center(), Vec2::splat(22.0)),
        icon,
        fg,
        p.surface_container_low,
    );

    let font = if active { font_medium(11.5) } else { font(11.5) };
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 43.0),
        Align2::CENTER_CENTER,
        label,
        font,
        fg,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn chip(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let f = if selected {
        font_medium(13.0)
    } else {
        font(13.0)
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), f, p.on_surface);
    let check_w = if selected { 17.0 } else { 0.0 };
    let width = galley.size().x + check_w + 28.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hover = hover_t(ui, &response, response.hovered() && enabled);
    let painter = ui.painter();
    let radius = CornerRadius::same(10);

    let (bg, fg, border) = if selected {
        (p.secondary_container, p.on_secondary_container, None)
    } else {
        (Color32::TRANSPARENT, p.on_surface_variant, Some(p.outline))
    };
    let (bg, fg, border) = if enabled {
        (bg, fg, border)
    } else {
        (bg, p.disabled_on(), border.map(|_| alpha(p.on_surface, 20)))
    };

    if selected {
        painter.rect_filled(rect, radius, bg);
    } else if hover > 0.0 {
        painter.rect_filled(rect, radius, alpha(p.on_surface, (hover * 20.0) as u8));
    }
    if let Some(border) = border {
        painter.rect_stroke(rect, radius, Stroke::new(1.0_f32, border), StrokeKind::Inside);
    }

    let mut x = rect.center().x - (galley.size().x + check_w) / 2.0;
    if selected {
        icons::draw(
            painter,
            Rect::from_center_size(Pos2::new(x + 7.0, rect.center().y), Vec2::splat(15.0)),
            Icon::Check,
            fg,
            bg,
        );
        x += check_w;
    }
    painter.galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

pub fn switch(ui: &mut Ui, p: &Palette, on: &mut bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(52.0, 32.0), Sense::click());
    if response.clicked() {
        *on = !*on;
    }
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("switch"), *on, 0.16);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let track = Rect::from_min_max(rect.min + Vec2::new(0.0, 2.0), rect.max - Vec2::new(0.0, 2.0));
    let radius = CornerRadius::same(14);
    let fill = blend(p.surface_container_highest, p.primary, t);
    painter.rect_filled(track, radius, fill);
    if t < 0.5 {
        let outline = blend(p.outline, Color32::TRANSPARENT, t);
        painter.rect_stroke(track, radius, Stroke::new(1.5_f32, outline), StrokeKind::Inside);
    }

    let cx = egui::lerp((track.min.x + 14.0)..=(track.max.x - 14.0), t);
    let handle_r = egui::lerp(9.0..=11.0, t);
    painter.circle_filled(Pos2::new(cx, track.center().y), handle_r, if *on { p.on_primary } else { p.outline });
    if t > 0.35 {
        let color = p.primary;
        let check_rect = Rect::from_center_size(
            Pos2::new(cx, track.center().y),
            Vec2::splat(12.0),
        );
        icons::draw(painter, check_rect, Icon::Check, color, fill);
    }
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if *on { "Включено" } else { "Выключено" })
}

pub fn progress(ui: &mut Ui, p: &Palette, fraction: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), p.surface_container_highest);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let w = (rect.width() * f).max(8.0).min(rect.width());
        let fill = Rect::from_min_size(rect.min, Vec2::new(w, rect.height()));
        painter.rect_filled(fill, CornerRadius::same(4), p.primary);
    }
}

/// Filled text field with an optional leading icon and clear button.
pub fn text_field(
    ui: &mut Ui,
    p: &Palette,
    id_salt: &str,
    text: &mut String,
    hint: &str,
    leading: Option<Icon>,
) -> Response {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 56.0), Sense::hover());
    let painter = ui.painter().clone();

    let leading_w = if leading.is_some() { 26.0 } else { 0.0 };
    let trailing_w = if text.is_empty() { 0.0 } else { 30.0 };
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 18.0 + leading_w, rect.min.y),
        Pos2::new(rect.max.x - 12.0 - trailing_w, rect.max.y),
    )
    .shrink2(Vec2::new(0.0, 16.0));

    let response = ui
        .push_id(id_salt, |ui| {
            let edit = egui::TextEdit::singleline(text)
                .frame(false)
                .hint_text(hint)
                .font(font(16.0))
                .text_color(p.on_surface)
                .desired_width(text_rect.width());
            let r = ui.put(text_rect, edit);
            if !r.has_focus() && leading.is_some() {
                // nothing
            }
            r
        })
        .inner;

    let focused = response.has_focus();
    let focus_t = ui
        .ctx()
        .animate_bool_with_time(ui.id().with((id_salt, "field-focus")), focused, 0.14);
    let bg = blend(p.surface_container_highest, p.surface_container_low, focus_t * 0.35);
    painter.rect_filled(rect, CornerRadius::same(16), bg);
    let border = if focused {
        p.primary
    } else {
        blend(p.outline_variant, p.primary, focus_t * 0.3)
    };
    painter.rect_stroke(
        rect,
        CornerRadius::same(16),
        Stroke::new(if focused { 2.0_f32 } else { 1.0_f32 }, border),
        StrokeKind::Inside,
    );

    if let Some(icon) = leading {
        icons::draw(
            &painter,
            Rect::from_center_size(
                Pos2::new(rect.min.x + 18.0 + leading_w / 2.0, rect.center().y),
                Vec2::splat(20.0),
            ),
            icon,
            if focused { p.primary } else { p.on_surface_variant },
            bg,
        );
    }

    if !text.is_empty() {
        let clear = Rect::from_center_size(
            Pos2::new(rect.max.x - 24.0, rect.center().y),
            Vec2::splat(28.0),
        );
        let resp = ui.interact(clear, ui.id().with((id_salt, "clear")), Sense::click());
        if resp.hovered() {
            painter.circle_filled(clear.center(), 12.0, alpha(p.on_surface, 18));
        }
        icons::draw(
            &painter,
            Rect::from_center_size(clear.center(), Vec2::splat(16.0)),
            Icon::Close,
            p.on_surface_variant,
            bg,
        );
        if resp.clicked() {
            text.clear();
        }
    }

    response
}

pub fn card<R>(
    ui: &mut Ui,
    fill: Color32,
    radius: u8,
    margin: i8,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(egui::Margin::same(margin))
        .show(ui, add)
        .inner
}

pub fn section_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(font_medium(12.0))
            .color(p.primary),
    );
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0, p.outline_variant);
}

pub fn ellipsize(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

pub fn body_text(text: impl Into<String>, size: f32, color: Color32) -> egui::RichText {
    egui::RichText::new(text).font(FontId::new(size, egui::FontFamily::Proportional)).color(color)
}
