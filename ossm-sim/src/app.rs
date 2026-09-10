use std::sync::mpsc::Receiver;

use egui::Color32;
use egui_plot::{Line, Plot, PlotPoint, PlotPoints};
use ossm_motion::{
    config::{
        MAX_MOVE_MM, MIN_MOVE_MM, MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS,
        MOTION_CONTROL_MAX_ACCELERATION, MOTION_CONTROL_MAX_VELOCITY, MOTION_CONTROL_MIN_VELOCITY,
    },
    motion::motion_state::{
        set_motion_depth_pct, set_motion_enabled, set_motion_length_pct, set_motion_pattern,
        set_motion_sensation_pct, set_motion_velocity_pct,
    },
    pattern::PatternExecutor,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::plotting::PlotMessage;

static NUM_POINTS: usize = 2000;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct OssmSim {
    #[serde(skip)]
    rx: Option<Receiver<PlotMessage>>,

    depth: u32,

    length: u32,

    velocity: u32,

    sensation: u32,

    motion_enabled: bool,

    #[serde(skip)]
    patterns: Vec<(String, u32)>,

    selected_pattern: usize,

    #[serde(skip)]
    position_points: Vec<PlotPoint>,

    #[serde(skip)]
    velocity_points: Vec<PlotPoint>,

    #[serde(skip)]
    acceleration_points: Vec<PlotPoint>,
}

impl Default for OssmSim {
    fn default() -> Self {
        Self {
            rx: None,
            depth: 0,
            length: 0,
            velocity: 0,
            sensation: 50,
            motion_enabled: false,
            patterns: vec![],
            selected_pattern: 0,
            position_points: vec![PlotPoint { x: 0.0, y: 0.0 }; NUM_POINTS],
            velocity_points: vec![PlotPoint { x: 0.0, y: 0.0 }; NUM_POINTS],
            acceleration_points: vec![PlotPoint { x: 0.0, y: 0.0 }; NUM_POINTS],
        }
    }
}

#[derive(Deserialize)]
struct Pattern {
    name: String,
    idx: u32,
}

impl OssmSim {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<PlotMessage>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut app = if let Some(storage) = cc.storage {
            let mut app: OssmSim = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.rx = Some(rx);
            app
        } else {
            let mut app: OssmSim = Default::default();
            app.rx = Some(rx);
            app
        };

        let patterns = PatternExecutor::new().get_all_patterns_json();
        let patterns: Vec<Pattern> =
            serde_json::from_str(&patterns).expect("Could not parse patterns");

        for pattern in patterns {
            app.patterns.push((pattern.name, pattern.idx));
        }

        log::info!("Patterns: {:?}", app.patterns);

        set_motion_depth_pct(app.depth);
        set_motion_length_pct(app.length);
        set_motion_velocity_pct(app.velocity);
        set_motion_sensation_pct(app.sensation);
        set_motion_pattern(app.patterns[app.selected_pattern].1);
        set_motion_enabled(app.motion_enabled);

        app
    }

    fn draw_plots(&mut self, ui: &mut egui::Ui) {
        let x_len = NUM_POINTS as f64 * (MOTION_CONTROL_LOOP_UPDATE_INTERVAL_MS as f64 / 1000.0);

        ui.vertical_centered(|ui| {
            ui.vertical(|ui| {
                ui.label("Position");
                Plot::new("position")
                    .height(200.0)
                    .y_grid_spacer(egui_plot::log_grid_spacer(5))
                    .default_x_bounds(self.position_points.first().unwrap().x, x_len)
                    .default_y_bounds(MIN_MOVE_MM - 5.0, MAX_MOVE_MM + 5.0)
                    .include_x(self.position_points.last().unwrap().x)
                    .show(ui, |plot_ui| {
                        plot_ui.add(
                            Line::new("position", self.position_points.as_slice())
                                .color(Color32::GREEN),
                        );
                    });
            });
            ui.vertical(|ui| {
                ui.label("Velocity");
                Plot::new("Velocity")
                    .height(200.0)
                    .y_grid_spacer(egui_plot::log_grid_spacer(5))
                    .default_x_bounds(self.velocity_points.first().unwrap().x, x_len)
                    .default_y_bounds(
                        -MOTION_CONTROL_MAX_VELOCITY - 5.0,
                        MOTION_CONTROL_MAX_VELOCITY + 5.0,
                    )
                    .include_x(self.velocity_points.last().unwrap().x)
                    .show(ui, |plot_ui| {
                        plot_ui.add(
                            Line::new("Velocity", self.velocity_points.as_slice())
                                .color(Color32::CYAN),
                        );
                    });
            });
            ui.vertical(|ui| {
                ui.label("Acceleration");
                Plot::new("Acceleration")
                    .height(200.0)
                    .y_grid_spacer(egui_plot::log_grid_spacer(5))
                    .default_x_bounds(self.acceleration_points.first().unwrap().x, x_len)
                    // .default_y_bounds(
                    //     -MOTION_CONTROL_MAX_ACCELERATION - 5.0,
                    //     MOTION_CONTROL_MAX_ACCELERATION + 5.0,
                    // )
                    .include_x(self.acceleration_points.last().unwrap().x)
                    .show(ui, |plot_ui| {
                        plot_ui.add(
                            Line::new("Acceleration", self.acceleration_points.as_slice())
                                .color(Color32::RED),
                        );
                    });
            });
        });
    }
}

impl eframe::App for OssmSim {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let rx = self.rx.as_mut().expect("Plot RX not present in the app");
        for msg in rx.try_iter() {
            let point_vec = match msg.name {
                "position" => &mut self.position_points,
                "velocity" => &mut self.velocity_points,
                "acceleration" => &mut self.acceleration_points,
                _ => panic!("Unknown plot"),
            };
            point_vec.push(msg.plot_point);
            if point_vec.len() > NUM_POINTS {
                point_vec.remove(0);
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("OSSM-SIM");

            // ui.add(egui::Slider::new(&mut self.value, 0.0..=10.0).text("value"));
            // if ui.button("Increment").clicked() {
            //     self.value += 1.0;
            // }

            ui.separator();

            ui.ctx().request_repaint();
            self.draw_plots(ui);

            let before = self.depth;
            ui.add(egui::Slider::new(&mut self.depth, 0..=100).text("Depth"));
            if before != self.depth {
                set_motion_depth_pct(self.depth);
            }

            let before = self.length;
            ui.add(egui::Slider::new(&mut self.length, 0..=100).text("Length"));
            if before != self.length {
                set_motion_length_pct(self.length);
            }

            let before = self.velocity;
            ui.add(egui::Slider::new(&mut self.velocity, 0..=100).text("Velocity"));
            if before != self.velocity {
                set_motion_velocity_pct(self.velocity);
            }

            let before = self.sensation;
            ui.add(egui::Slider::new(&mut self.sensation, 0..=100).text("Sensation"));
            if before != self.sensation {
                set_motion_sensation_pct(self.sensation);
            }

            let before = self.motion_enabled;
            ui.add(egui::Checkbox::new(
                &mut self.motion_enabled,
                "Motion Enabled",
            ));
            if before != self.motion_enabled {
                set_motion_enabled(self.motion_enabled);
            }

            let before = self.selected_pattern;
            egui::ComboBox::from_label("Pattern").show_index(
                ui,
                &mut self.selected_pattern,
                self.patterns.len(),
                |i| &self.patterns[i].0,
            );
            if before != self.selected_pattern {
                set_motion_pattern(self.patterns[self.selected_pattern].1);
            }
        });
    }
}
