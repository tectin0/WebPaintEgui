use egui::Style;
use egui::{emath, vec2, Color32, Context, Frame, Pos2, Rect, Sense, Stroke, Ui, Window};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    lines: Vec<Vec<Pos2>>,
    stroke: Stroke,
}

impl Default for App {
    fn default() -> Self {
        Self {
            lines: Default::default(),
            stroke: Stroke::new(1.0, Color32::BLACK),
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            Frame::canvas(ui.style()).show(ui, |ui| {
                let (mut response, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::drag());

                let to_screen = emath::RectTransform::from_to(
                    Rect::from_min_size(Pos2::ZERO, response.rect.square_proportions()),
                    response.rect,
                );
                let from_screen = to_screen.inverse();

                if self.lines.is_empty() {
                    self.lines.push(vec![]);
                }

                let current_line = self.lines.last_mut().unwrap();

                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let canvas_pos = from_screen * pointer_pos;
                    if current_line.last() != Some(&canvas_pos) {
                        current_line.push(canvas_pos);
                        response.mark_changed();
                    }
                } else if !current_line.is_empty() {
                    self.lines.push(vec![]);
                    response.mark_changed();
                }

                let shapes = self
                    .lines
                    .iter()
                    .filter(|line| line.len() >= 2)
                    .map(|line| {
                        let points: Vec<Pos2> = line.iter().map(|p| to_screen * *p).collect();
                        egui::Shape::line(points, self.stroke)
                    });

                painter.extend(shapes);

                response
            });
        });
    }
}
