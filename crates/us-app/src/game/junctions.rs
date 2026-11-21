//! The traffic inspector reads the same approach geometry and clock as the street.

use super::Game;
use super::panels::{MapView, btn, lab, lab_c, wrapped};
use crate::theme::{ACCENT, BAD, GOOD, MUTED, TEXT};
use egui::Ui;
use us_gen::Pt;
use us_render::SignalAspect;

fn approach_label(bearing: f32, class: us_gen::RoadClass) -> String {
    let points = [
        "North",
        "Northeast",
        "East",
        "Southeast",
        "South",
        "Southwest",
        "West",
        "Northwest",
    ];
    let octant = ((bearing + 22.5) / 45.0).floor() as usize % points.len();
    format!("{} · {}", points[octant], class.name())
}

impl Game {
    pub(super) fn junction_details(&mut self, ui: &mut Ui, point: Pt) {
        let Some(junction) = self.live.scene.junction_report(point) else {
            wrapped(
                ui,
                "This junction has changed. Select it again on the map.",
                12.0,
                TEXT,
            );
            return;
        };
        let control = if junction.signalized {
            "Traffic signals"
        } else {
            "Approaches take turns"
        };
        lab_c(ui, control, 13.0, ACCENT);
        let traffic_map = self.ui.view == MapView::Traffic;
        let map_label = if traffic_map {
            "City view"
        } else {
            "Traffic map"
        };
        if btn(ui, map_label, false, 12.0).clicked() {
            self.ui.view = if traffic_map {
                MapView::Normal
            } else {
                MapView::Traffic
            };
        }
        ui.separator();
        for approach in &junction.approaches {
            lab(
                ui,
                approach_label(approach.bearing_deg, approach.class),
                13.0,
            );
            if let Some(signal) = approach.signal {
                let (phase, colour) = match signal {
                    SignalAspect::Green => ("Green".to_owned(), GOOD),
                    SignalAspect::Amber => ("Amber".to_owned(), ACCENT),
                    SignalAspect::Red => (
                        format!("Red · green in {:.0}s", approach.seconds_until_green.ceil()),
                        BAD,
                    ),
                };
                lab_c(ui, phase, 12.0, colour);
            }
            if let Some(peak) = approach.peak {
                let over = peak.rate > peak.capacity;
                let label = format!(
                    "{:.0} approaching · {:.0} capacity per hour",
                    peak.rate, peak.capacity
                );
                wrapped(ui, label, 12.0, if over { BAD } else { TEXT });
                if over {
                    lab_c(ui, "Likely rush-hour queue", 11.0, BAD);
                }
            } else {
                lab_c(ui, "Peak estimate unavailable", 11.0, MUTED);
            }
            ui.separator();
        }
    }
}
