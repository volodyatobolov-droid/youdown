//! Material You (Material 3) dynamic color system.
//!
//! Generates tonal palettes from a seed color in the OKLab color space and maps
//! them to Material 3 color roles for light and dark themes.

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Система",
            Self::Light => "Светлая",
            Self::Dark => "Тёмная",
        }
    }
}

/// A full Material 3 color scheme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,

    pub secondary: Color32,
    pub on_secondary: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,

    pub tertiary: Color32,
    pub tertiary_container: Color32,
    pub on_tertiary_container: Color32,

    pub error: Color32,
    pub on_error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,

    pub surface: Color32,
    pub on_surface: Color32,
    pub surface_variant: Color32,
    pub on_surface_variant: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,

    pub outline: Color32,
    pub outline_variant: Color32,

    pub inverse_surface: Color32,
    pub inverse_on_surface: Color32,
    pub inverse_primary: Color32,

    pub scrim: Color32,
    pub dark: bool,
}

impl Palette {
    pub fn from_seed(seed: [u8; 3], dark: bool) -> Self {
        let (hue, chroma, _) = srgb_to_oklch(seed[0], seed[1], seed[2]);

        // Chroma factors per palette, following Material's tonal-palette idea.
        let primary = |tone: f32| tone_color(hue, chroma, 1.0, tone);
        let secondary = |tone: f32| tone_color(hue, chroma, 0.42, tone);
        let tertiary = |tone: f32| tone_color(hue + 60.0, chroma, 0.95, tone);
        let neutral = |tone: f32| tone_color(hue, chroma, 0.045, tone);
        let neutral_variant = |tone: f32| tone_color(hue, chroma, 0.085, tone);
        let error = |tone: f32| tone_color(25.0, 0.17, 1.0, tone);

        if dark {
            Self {
                primary: primary(80.0),
                on_primary: primary(20.0),
                primary_container: primary(30.0),
                on_primary_container: primary(90.0),

                secondary: secondary(80.0),
                on_secondary: secondary(20.0),
                secondary_container: secondary(30.0),
                on_secondary_container: secondary(90.0),

                tertiary: tertiary(80.0),
                tertiary_container: tertiary(30.0),
                on_tertiary_container: tertiary(90.0),

                error: error(80.0),
                on_error: error(20.0),
                error_container: error(30.0),
                on_error_container: error(90.0),

                surface: neutral(6.0),
                on_surface: neutral(90.0),
                surface_variant: neutral_variant(30.0),
                on_surface_variant: neutral_variant(80.0),
                surface_container_lowest: neutral(4.0),
                surface_container_low: neutral(10.0),
                surface_container: neutral(12.0),
                surface_container_high: neutral(17.0),
                surface_container_highest: neutral(22.0),

                outline: neutral_variant(60.0),
                outline_variant: neutral_variant(30.0),

                inverse_surface: neutral(90.0),
                inverse_on_surface: neutral(20.0),
                inverse_primary: primary(40.0),

                scrim: Color32::from_rgb(0, 0, 0),
                dark: true,
            }
        } else {
            Self {
                primary: primary(40.0),
                on_primary: primary(100.0),
                primary_container: primary(90.0),
                on_primary_container: primary(10.0),

                secondary: secondary(40.0),
                on_secondary: secondary(100.0),
                secondary_container: secondary(90.0),
                on_secondary_container: secondary(10.0),

                tertiary: tertiary(40.0),
                tertiary_container: tertiary(90.0),
                on_tertiary_container: tertiary(10.0),

                error: error(40.0),
                on_error: error(100.0),
                error_container: error(90.0),
                on_error_container: error(10.0),

                surface: neutral(98.0),
                on_surface: neutral(10.0),
                surface_variant: neutral_variant(90.0),
                on_surface_variant: neutral_variant(30.0),
                surface_container_lowest: neutral(100.0),
                surface_container_low: neutral(96.0),
                surface_container: neutral(94.0),
                surface_container_high: neutral(92.0),
                surface_container_highest: neutral(90.0),

                outline: neutral_variant(50.0),
                outline_variant: neutral_variant(80.0),

                inverse_surface: neutral(20.0),
                inverse_on_surface: neutral(95.0),
                inverse_primary: primary(80.0),

                scrim: Color32::from_rgb(0, 0, 0),
                dark: false,
            }
        }
    }

    /// Background of the main content area.
    pub fn background(&self) -> Color32 {
        if self.dark {
            self.surface
        } else {
            self.surface
        }
    }

    pub fn disabled_on(&self) -> Color32 {
        alpha(self.on_surface, if self.dark { 60 } else { 97 })
    }

    pub fn disabled_container(&self) -> Color32 {
        alpha(self.on_surface, if self.dark { 20 } else { 20 })
    }
}

/// Material 3 typography helpers.
pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn font_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}

pub fn font_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn install_fonts(ctx: &egui::Context) {
    use std::sync::Arc;

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "roboto".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Roboto-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "roboto-medium".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Roboto-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "roboto-bold".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Roboto-Bold.ttf"
        ))),
    );

    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let mut roboto = vec!["roboto".to_owned()];
    roboto.extend(proportional.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, roboto.clone());

    let mut medium = vec!["roboto-medium".to_owned()];
    medium.extend(proportional.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Name("medium".into()), medium);

    let mut bold = vec!["roboto-bold".to_owned()];
    bold.extend(proportional.iter().cloned());
    fonts.families.insert(FontFamily::Name("bold".into()), bold);

    ctx.set_fonts(fonts);
}

/// Default egui style used by built-in widgets (text edits, scrollbars…).
pub fn apply_style(ctx: &egui::Context, p: &Palette) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals(p);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 6.0,
        floating: true,
        ..Default::default()
    };
    style.text_styles = [
        (egui::TextStyle::Small, font(12.0)),
        (egui::TextStyle::Body, font(15.0)),
        (egui::TextStyle::Button, font_medium(15.0)),
        (egui::TextStyle::Heading, font_bold(24.0)),
        (egui::TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
    ctx.set_style(style);
}

fn visuals(p: &Palette) -> egui::Visuals {
    let mut v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.override_text_color = Some(p.on_surface);
    v.panel_fill = p.surface;
    v.window_fill = p.surface_container;
    v.extreme_bg_color = p.surface_container_highest;
    v.faint_bg_color = p.surface_container_low;
    v.window_stroke = Stroke::new(1.0_f32, p.outline_variant);
    v.selection.bg_fill = alpha(p.primary, 80);
    v.selection.stroke = Stroke::new(1.5_f32, p.on_surface);
    v.hyperlink_color = p.primary;
    v.warn_fg_color = p.tertiary;
    v.error_fg_color = p.error;
    v.widgets.noninteractive.bg_fill = p.surface_container;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    v.widgets.inactive.bg_fill = p.surface_container_high;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.on_surface_variant);
    v.widgets.hovered.bg_fill = p.surface_container_highest;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    v.widgets.active.bg_fill = p.surface_container_highest;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    v.widgets.open.bg_fill = p.surface_container_high;
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    v.widgets.inactive.weak_bg_fill = p.surface_container_high;
    v.widgets.inactive.corner_radius = egui::CornerRadius::same(12);
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(12);
    v.widgets.active.corner_radius = egui::CornerRadius::same(12);
    v.text_cursor.stroke = Stroke::new(2.0_f32, p.primary);
    v
}

pub fn alpha(color: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = color.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Blend `top` over `bottom` with the given opacity of `top`.
pub fn blend(bottom: Color32, top: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let b = bottom.to_array();
    let o = top.to_array();
    Color32::from_rgba_unmultiplied(
        (b[0] as f32 + (o[0] as f32 - b[0] as f32) * t).round() as u8,
        (b[1] as f32 + (o[1] as f32 - b[1] as f32) * t).round() as u8,
        (b[2] as f32 + (o[2] as f32 - b[2] as f32) * t).round() as u8,
        255,
    )
}

// ---------------------------------------------------------------------------
// Color math: sRGB <-> OKLab, with gamut mapping by chroma reduction.
// ---------------------------------------------------------------------------

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn linear_srgb_to_oklab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    (
        0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
        1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    )
}

fn oklab_to_linear_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    (
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    )
}

fn srgb_to_oklch(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (l, a, bb) = linear_srgb_to_oklab(
        srgb_to_linear(r as f32 / 255.0),
        srgb_to_linear(g as f32 / 255.0),
        srgb_to_linear(b as f32 / 255.0),
    );
    let chroma = (a * a + bb * bb).sqrt();
    let hue = bb.atan2(a).to_degrees().rem_euclid(360.0);
    (hue, chroma, l)
}

fn oklch_to_color(l: f32, chroma: f32, hue_deg: f32) -> Color32 {
    let hue = hue_deg.to_radians();
    let (sin, cos) = hue.sin_cos();
    let mut c = chroma.max(0.0);
    for _ in 0..24 {
        let (r, g, b) = oklab_to_linear_srgb(l, c * cos, c * sin);
        if (-0.001..=1.001).contains(&r)
            && (-0.001..=1.001).contains(&g)
            && (-0.001..=1.001).contains(&b)
        {
            return Color32::from_rgb(
                (linear_to_srgb(r.clamp(0.0, 1.0)) * 255.0).round() as u8,
                (linear_to_srgb(g.clamp(0.0, 1.0)) * 255.0).round() as u8,
                (linear_to_srgb(b.clamp(0.0, 1.0)) * 255.0).round() as u8,
            );
        }
        c *= 0.93;
    }
    let (r, g, b) = oklab_to_linear_srgb(l, 0.0, 0.0);
    Color32::from_rgb(
        (linear_to_srgb(r.clamp(0.0, 1.0)) * 255.0).round() as u8,
        (linear_to_srgb(g.clamp(0.0, 1.0)) * 255.0).round() as u8,
        (linear_to_srgb(b.clamp(0.0, 1.0)) * 255.0).round() as u8,
    )
}

/// A single tone of a tonal palette. `chroma_factor` scales the seed chroma,
/// `tone` is a Material tone 0..=100 (CIE L*).
fn tone_color(hue: f32, seed_chroma: f32, chroma_factor: f32, tone: f32) -> Color32 {
    let t = tone.clamp(0.0, 100.0);
    let bell = 1.0 - ((t - 50.0) / 50.0).powi(2);
    let chroma = (seed_chroma * chroma_factor * (0.30 + 0.70 * bell)).min(0.19);
    // Material tones are CIE L* values. Convert L* to relative luminance and then
    // to OKLab lightness (OKLab L is the cube root of luminance for neutrals).
    let y = if t > 8.0 {
        ((t + 16.0) / 116.0).powi(3)
    } else {
        t / 903.3
    };
    let l = y.clamp(0.0, 1.0).cbrt();
    oklch_to_color(l, chroma, hue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_are_valid() {
        for dark in [true, false] {
            for seed in [[103, 80, 164], [255, 0, 51], [0, 128, 255], [255, 255, 255]] {
                let p = Palette::from_seed(seed, dark);
                assert_eq!(p.on_surface.to_array()[3], 255);
                assert_ne!(p.primary, p.surface);
            }
        }
    }

    #[test]
    fn contrast_between_primary_and_on_primary() {
        let p = Palette::from_seed([103, 80, 164], false);
        let a = p.primary.to_array();
        let b = p.on_primary.to_array();
        let lum = |c: [u8; 4]| {
            0.2126 * (c[0] as f32 / 255.0) + 0.7152 * (c[1] as f32 / 255.0) + 0.0722 * (c[2] as f32 / 255.0)
        };
        assert!((lum(a) - lum(b)).abs() > 0.3);
    }

    #[test]
    fn material_tones_have_expected_lightness() {
        let light = Palette::from_seed([103, 80, 164], false);
        let dark = Palette::from_seed([103, 80, 164], true);
        let seed = [103u8, 80, 164];
        let dist = |c: Color32| {
            let a = c.to_array();
            ((a[0] as i32 - seed[0] as i32).pow(2)
                + (a[1] as i32 - seed[1] as i32).pow(2)
                + (a[2] as i32 - seed[2] as i32).pow(2)) as f32
        };
        // Tone 40 of the seed's own hue is close to the seed itself.
        assert!(dist(light.primary).sqrt() < 40.0, "primary = {:?}", light.primary);
        // Dark surfaces are dark but not pure black (Material tone 6 ≈ #141218).
        let s = dark.surface.to_array();
        assert!(s[0] < 45 && s[0] > 8, "dark surface = {:?}", dark.surface);
        // Light surfaces are near-white.
        assert!(light.surface.to_array()[0] > 240);
        // On-dark text is light.
        assert!(dark.on_surface.to_array()[0] > 200);
    }
}
