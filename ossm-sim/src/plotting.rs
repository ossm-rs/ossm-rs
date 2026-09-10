use egui_plot::PlotPoint;

pub struct PlotMessage {
    pub name: &'static str,
    pub plot_point: PlotPoint,
}

impl PlotMessage {
    pub fn new(name: &'static str, x: f64, y: f64) -> Self {
        Self {
            name,
            plot_point: PlotPoint { x, y },
        }
    }
}
