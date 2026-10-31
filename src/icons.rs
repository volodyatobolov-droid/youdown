//! Hand-drawn Material-style icons, so the app does not depend on an icon font.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Download,
    Library,
    Settings,
    Search,
    Paste,
    Folder,
    Play,
    Close,
    Check,
    Sun,
    Moon,
    Refresh,
    Trash,
    Open,
    Clock,
    Alert,
    Link,
    Video,
    Music,
    ChevronDown,
    Copy,
}

pub fn draw(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32, background: Color32) {
    let s = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), egui::vec2(s, s));
    // Coordinates are authored on a 24x24 grid.
    let p = |x: f32, y: f32| -> Pos2 {
        Pos2::new(
            r.min.x + x / 24.0 * r.width(),
            r.min.y + y / 24.0 * r.height(),
        )
    };
    let w = (s / 24.0 * 2.0).max(1.4);
    let stroke = Stroke::new(w, color);

    macro_rules! poly {
        ($($x:expr, $y:expr);+ $(;)?) => {
            painter.add(Shape::line(vec![$(p($x as f32, $y as f32)),+], stroke))
        };
    }
    macro_rules! seg {
        ($x1:expr, $y1:expr, $x2:expr, $y2:expr) => {
            painter.line_segment([p($x1 as f32, $y1 as f32), p($x2 as f32, $y2 as f32)], stroke)
        };
    }

    match icon {
        Icon::Download => {
            seg!(12.0, 3.5, 12.0, 15.0);
            poly!(6.5, 9.5; 12.0, 15.0; 17.5, 9.5);
            poly!(4.5, 16.0; 4.5, 19.5; 19.5, 19.5; 19.5, 16.0);
        }
        Icon::Library => {
            painter.rect_stroke(
                Rect::from_min_max(p(3.0, 5.0), p(21.0, 19.0)),
                (s / 24.0 * 2.5) as u8,
                stroke,
                StrokeKind::Inside,
            );
            let tri = vec![p(10.0, 9.0), p(16.0, 12.0), p(10.0, 15.0)];
            painter.add(Shape::convex_polygon(tri, color, Stroke::NONE));
        }
        Icon::Settings => {
            painter.circle_stroke(r.center(), s * 0.19, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sa, ca) = a.sin_cos();
                painter.line_segment(
                    [
                        r.center() + egui::vec2(ca * s * 0.32, sa * s * 0.32),
                        r.center() + egui::vec2(ca * s * 0.43, sa * s * 0.43),
                    ],
                    stroke,
                );
            }
        }
        Icon::Search => {
            painter.circle_stroke(p(10.5, 10.5), s * 0.26, stroke);
            seg!(15.0, 15.0, 20.0, 20.0);
        }
        Icon::Paste => {
            painter.rect_stroke(
                Rect::from_min_max(p(6.0, 5.5), p(18.0, 20.0)),
                (s / 24.0 * 1.5) as u8,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_filled(
                Rect::from_min_max(p(9.0, 3.0), p(15.0, 6.5)),
                (s / 24.0) as u8,
                color,
            );
            seg!(9.0, 11.0, 15.0, 11.0);
            seg!(9.0, 15.0, 13.5, 15.0);
        }
        Icon::Folder => {
            let pts = vec![
                p(3.0, 6.0),
                p(9.5, 6.0),
                p(11.5, 8.5),
                p(21.0, 8.5),
                p(21.0, 19.0),
                p(3.0, 19.0),
            ];
            painter.add(Shape::closed_line(pts, stroke));
        }
        Icon::Play => {
            let tri = vec![p(8.0, 4.5), p(20.0, 12.0), p(8.0, 19.5)];
            painter.add(Shape::convex_polygon(tri, color, Stroke::NONE));
        }
        Icon::Close => {
            seg!(6.5, 6.5, 17.5, 17.5);
            seg!(17.5, 6.5, 6.5, 17.5);
        }
        Icon::Check => {
            poly!(5.0, 12.5; 9.5, 17.0; 19.0, 6.5);
        }
        Icon::Sun => {
            painter.circle_stroke(r.center(), s * 0.17, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sa, ca) = a.sin_cos();
                painter.line_segment(
                    [
                        r.center() + egui::vec2(ca * s * 0.28, sa * s * 0.28),
                        r.center() + egui::vec2(ca * s * 0.40, sa * s * 0.40),
                    ],
                    stroke,
                );
            }
        }
        Icon::Moon => {
            painter.circle_filled(r.center(), s * 0.31, color);
            painter.circle_filled(
                r.center() + egui::vec2(s * 0.15, -s * 0.13),
                s * 0.27,
                background,
            );
        }
        Icon::Refresh => {
            let pts: Vec<Pos2> = (0..=28)
                .map(|i| {
                    let a = 0.5 + i as f32 / 28.0 * 1.55 * std::f32::consts::TAU;
                    r.center() + egui::vec2(a.cos() * s * 0.31, a.sin() * s * 0.31)
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
            let arrow = vec![p(14.5, 3.0), p(19.5, 6.2), p(14.0, 8.0)];
            painter.add(Shape::closed_line(arrow, stroke));
        }
        Icon::Trash => {
            seg!(4.0, 7.0, 20.0, 7.0);
            poly!(9.0, 7.0; 9.0, 4.0; 15.0, 4.0; 15.0, 7.0);
            poly!(6.0, 7.0; 7.0, 20.0; 17.0, 20.0; 18.0, 7.0);
            seg!(10.0, 10.5, 10.0, 16.5);
            seg!(14.0, 10.5, 14.0, 16.5);
        }
        Icon::Open => {
            painter.rect_stroke(
                Rect::from_min_max(p(3.5, 8.0), p(16.0, 20.5)),
                (s / 24.0 * 1.5) as u8,
                stroke,
                StrokeKind::Inside,
            );
            poly!(13.0, 3.5; 20.5, 3.5; 20.5, 11.0);
            seg!(20.0, 4.0, 11.5, 12.5);
        }
        Icon::Clock => {
            painter.circle_stroke(r.center(), s * 0.36, stroke);
            poly!(12.0, 6.5; 12.0, 12.3; 16.0, 14.5);
        }
        Icon::Alert => {
            let tri = vec![p(12.0, 3.0), p(22.0, 20.5), p(2.0, 20.5)];
            painter.add(Shape::closed_line(tri, stroke));
            seg!(12.0, 9.0, 12.0, 14.5);
            painter.circle_filled(p(12.0, 17.5), (s * 0.045).max(1.0), color);
        }
        Icon::Link => {
            let rad = (s / 24.0 * 4.0) as u8;
            painter.rect_stroke(
                Rect::from_min_max(p(2.5, 9.0), p(13.5, 15.0)),
                rad,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(10.5, 9.0), p(21.5, 15.0)),
                rad,
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Video => {
            painter.rect_stroke(
                Rect::from_min_max(p(2.5, 6.0), p(17.5, 18.0)),
                (s / 24.0 * 2.0) as u8,
                stroke,
                StrokeKind::Inside,
            );
            let tri = vec![p(19.5, 9.0), p(22.0, 7.5), p(22.0, 16.5), p(19.5, 15.0)];
            painter.add(Shape::convex_polygon(tri, color, Stroke::NONE));
        }
        Icon::Music => {
            poly!(9.0, 17.5; 9.0, 5.0; 19.5, 3.0; 19.5, 15.5);
            painter.circle_filled(p(6.5, 18.0), s * 0.12, color);
            painter.circle_filled(p(17.0, 16.0), s * 0.12, color);
            seg!(9.0, 8.0, 19.5, 6.0);
        }
        Icon::ChevronDown => {
            poly!(6.5, 9.5; 12.0, 15.0; 17.5, 9.5);
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(p(8.0, 8.0), p(20.0, 20.0)),
                (s / 24.0 * 1.5) as u8,
                stroke,
                StrokeKind::Inside,
            );
            poly!(16.0, 5.5; 4.0, 5.5; 4.0, 17.0);
        }
    }
}
