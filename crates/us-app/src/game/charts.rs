//! Line and area charts for the monthly history. Solid lines and flat fills only: no gradients,
//! no glow.

use crate::theme::{ACCENT, EDGE, INK, LINE, PANEL_DARK, pixel};
use egui::epaint::Mesh;
use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, vec2};

/// How a value is written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fmt {
    Count,
    Money,
    /// A percent number such as 7.5.
    Percent,
    /// A fraction from 0 to 1, shown as a percent.
    Fraction,
}

impl Fmt {
    /// Full text for the hover read-out.
    pub fn full(self, v: f64) -> String {
        match self {
            Fmt::Count => group(v.round() as i64),
            Fmt::Money => {
                let r = v.round() as i64;
                format!("{}${}", if r < 0 { "-" } else { "" }, group(r.abs()))
            }
            Fmt::Percent => format!("{v:.1}%"),
            Fmt::Fraction => format!("{:.0}%", v * 100.0),
        }
    }

    /// Short text for axis labels.
    fn short(self, v: f64) -> String {
        match self {
            Fmt::Count => compact(v),
            Fmt::Money => {
                let c = compact(v.abs());
                format!("{}${c}", if v < 0.0 { "-" } else { "" })
            }
            Fmt::Percent => format!("{v:.0}%"),
            Fmt::Fraction => format!("{:.0}%", v * 100.0),
        }
    }
}

fn group(v: i64) -> String {
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let out: String = out.chars().rev().collect();
    if v < 0 { format!("-{out}") } else { out }
}

fn compact(v: f64) -> String {
    let a = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if a >= 1.0e6 {
        format!("{sign}{:.1}M", a / 1.0e6)
    } else if a >= 1.0e4 {
        format!("{sign}{:.0}K", a / 1.0e3)
    } else if a >= 1.0e3 {
        format!("{sign}{:.1}K", a / 1.0e3)
    } else {
        format!("{sign}{a:.0}")
    }
}

/// One drawn series.
pub struct Series {
    pub name: String,
    pub color: Color32,
    pub vals: Vec<f64>,
}

/// A step that gives round tick labels, at most about `n` ticks over `span`.
fn nice_step(span: f64, n: f64) -> f64 {
    let raw = (span / n).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    let m = if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    };
    m * mag
}

/// Everything a chart needs besides the drawing area.
pub struct Chart<'a> {
    pub series: &'a [Series],
    pub visible: &'a [bool],
    /// Game day of every sample.
    pub days: &'a [i32],
    /// Month (1 to 12) and year of every sample, for the axis.
    pub dates: &'a [(i32, u32)],
    pub stacked: bool,
    pub fmt: Fmt,
}

const MARGIN_L: f32 = 78.0;

pub fn draw(ui: &mut Ui, size: egui::Vec2, c: &Chart) {
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let rect = resp.rect;
    painter.rect_filled(rect, 0.0, PANEL_DARK);
    painter.rect_stroke(rect, 0.0, Stroke::new(2.0, EDGE), egui::StrokeKind::Inside);
    let n = c.days.len();
    let shown: Vec<usize> = (0..c.series.len())
        .filter(|&i| c.visible.get(i).copied().unwrap_or(true))
        .collect();
    if n == 0 || shown.is_empty() {
        let msg = if n == 0 {
            "NO DATA YET: LET A MONTH PASS"
        } else {
            "ALL SERIES ARE OFF"
        };
        painter.text(rect.center(), Align2::CENTER_CENTER, msg, pixel(12.0), INK);
        return;
    }
    let plot = Rect::from_min_max(rect.min + vec2(MARGIN_L, 12.0), rect.max - vec2(14.0, 26.0));

    // value range
    let mut cum: Vec<Vec<f64>> = Vec::new(); // upper edge per shown series when stacked
    if c.stacked {
        let mut run = vec![0.0f64; n];
        for &s in &shown {
            for (r, v) in run.iter_mut().zip(&c.series[s].vals) {
                *r += v.max(0.0);
            }
            cum.push(run.clone());
        }
    }
    let (mut lo, mut hi) = (0.0f64, f64::MIN);
    if c.stacked {
        hi = cum
            .last()
            .map_or(1.0, |t| t.iter().copied().fold(0.0, f64::max));
    } else {
        for &s in &shown {
            for &v in &c.series[s].vals {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    if c.fmt == Fmt::Fraction {
        hi = hi.max(0.1);
    }
    if hi <= lo {
        hi = lo + 1.0;
    }
    let step = nice_step(hi - lo, 4.0);
    let top = (hi / step).ceil() * step;
    let bottom = if lo < 0.0 {
        (lo / step).floor() * step
    } else {
        0.0
    };
    let y_of = |v: f64| plot.bottom() - plot.height() * ((v - bottom) / (top - bottom)) as f32;
    let x_of = |i: usize| {
        if n == 1 {
            plot.center().x
        } else {
            plot.left() + plot.width() * i as f32 / (n - 1) as f32
        }
    };

    // grid and y labels
    let mut t = bottom;
    while t <= top + step * 0.01 {
        let y = y_of(t);
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(
                1.0,
                if t == 0.0 {
                    Color32::from_rgb(70, 96, 122)
                } else {
                    EDGE
                },
            ),
        );
        painter.text(
            Pos2::new(plot.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            c.fmt.short(t),
            pixel(12.0),
            INK,
        );
        t += step;
    }

    // x labels: a year at every January when the range is long, else month names
    const MON: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    let by_month = n <= 14;
    let stride = if by_month {
        ((n as f32 * 96.0 / plot.width()).ceil() as usize).max(1)
    } else if n > 72 {
        12 * ((n as f32 / 12.0 * 64.0 / plot.width()).ceil() as usize).max(1)
    } else {
        12
    };
    let mut last_label_x = f32::MIN;
    for i in 0..n {
        let (year, month) = c.dates[i];
        let want = if by_month {
            i % stride == 0
        } else {
            month == 1 && (year.rem_euclid(stride as i32 / 12)) == 0
        };
        if !want {
            continue;
        }
        let x = x_of(i);
        if x - last_label_x < 86.0 {
            continue;
        }
        last_label_x = x;
        painter.line_segment(
            [
                Pos2::new(x, plot.bottom()),
                Pos2::new(x, plot.bottom() + 4.0),
            ],
            Stroke::new(1.0, INK),
        );
        let text = if by_month {
            format!("{} {}", MON[month as usize - 1], year)
        } else {
            year.to_string()
        };
        painter.text(
            Pos2::new(x, plot.bottom() + 6.0),
            Align2::CENTER_TOP,
            text,
            pixel(12.0),
            INK,
        );
    }

    let hover = resp
        .hover_pos()
        .filter(|p| plot.expand2(vec2(2.0, 6.0)).contains(*p))
        .map(|p| {
            if n == 1 {
                0
            } else {
                (((p.x - plot.left()) / plot.width()) * (n - 1) as f32)
                    .round()
                    .clamp(0.0, (n - 1) as f32) as usize
            }
        });

    // data
    if c.stacked {
        for (k, &s) in shown.iter().enumerate() {
            let col = c.series[s].color;
            let upper = &cum[k];
            let zero = vec![0.0f64; n];
            let lower = if k == 0 { &zero } else { &cum[k - 1] };
            if n == 1 {
                let x = x_of(0);
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x - 4.0, y_of(upper[0])),
                        Pos2::new(x + 4.0, y_of(lower[0])),
                    ),
                    0.0,
                    col,
                );
                continue;
            }
            let mut mesh = Mesh::default();
            for i in 0..n - 1 {
                let q = [
                    Pos2::new(x_of(i), y_of(upper[i])),
                    Pos2::new(x_of(i + 1), y_of(upper[i + 1])),
                    Pos2::new(x_of(i + 1), y_of(lower[i + 1])),
                    Pos2::new(x_of(i), y_of(lower[i])),
                ];
                let base = mesh.vertices.len() as u32;
                for p in q {
                    mesh.colored_vertex(p, col);
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base, base + 2, base + 3);
            }
            painter.add(Shape::mesh(mesh));
            let top_line: Vec<Pos2> = (0..n).map(|i| Pos2::new(x_of(i), y_of(upper[i]))).collect();
            painter.add(Shape::line(top_line, Stroke::new(1.5, LINE)));
        }
    } else {
        for &s in &shown {
            let ser = &c.series[s];
            let pts: Vec<Pos2> = ser
                .vals
                .iter()
                .enumerate()
                .map(|(i, &v)| Pos2::new(x_of(i), y_of(v)))
                .collect();
            if pts.len() == 1 {
                painter.circle_filled(pts[0], 4.0, ser.color);
            } else {
                painter.add(Shape::line(pts, Stroke::new(2.5, ser.color)));
            }
        }
    }

    // hover read-out
    if let Some(i) = hover {
        let x = x_of(i);
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0, INK),
        );
        for (k, &s) in shown.iter().enumerate() {
            let v = if c.stacked {
                cum[k][i]
            } else {
                c.series[s].vals[i]
            };
            let p = Pos2::new(x, y_of(v));
            painter.circle_filled(p, 5.0, LINE);
            painter.circle_filled(p, 3.5, c.series[s].color);
        }
        let (year, month) = c.dates[i];
        let head = format!("{} {}", MON[month as usize - 1], year);
        let fmt = c.fmt;
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(head).font(pixel(12.0)).color(ACCENT));
            for &s in &shown {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, c.series[s].color);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  {}",
                            c.series[s].name,
                            fmt.full(c.series[s].vals[i])
                        ))
                        .font(pixel(12.0))
                        .color(INK),
                    );
                });
            }
        });
    }
}

/// A tiny line with no axes, for the top bar. Shows the last `keep` values.
pub fn spark(ui: &mut Ui, size: egui::Vec2, vals: &[f64], color: Color32, tip: &str) {
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let r = resp.rect;
    painter.rect_filled(r, 0.0, PANEL_DARK);
    painter.rect_stroke(r, 0.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    if vals.len() >= 2 {
        let lo = vals.iter().copied().fold(f64::MAX, f64::min);
        let hi = vals.iter().copied().fold(f64::MIN, f64::max);
        let span = (hi - lo).max(1e-9);
        let inner = r.shrink2(vec2(4.0, 4.0));
        let n = vals.len();
        let pts: Vec<Pos2> = vals
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                Pos2::new(
                    inner.left() + inner.width() * i as f32 / (n - 1) as f32,
                    inner.bottom() - inner.height() * ((v - lo) / span) as f32,
                )
            })
            .collect();
        let last = *pts.last().unwrap();
        painter.add(Shape::line(pts, Stroke::new(2.0, color)));
        painter.circle_filled(last, 2.5, color);
    }
    resp.on_hover_text(egui::RichText::new(tip).font(pixel(12.0)));
}
