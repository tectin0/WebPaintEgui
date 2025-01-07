use std::collections::HashMap;

use egui::{
    emath, pos2, vec2, Color32, ColorImage, Context, Frame, Pos2, Rect, Sense, Stroke, TextureId,
    TextureOptions, Ui, Window,
};
use egui::{Style, TextureHandle};
use log::debug;

use std::ops::Add;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct State {
    lines: Vec<Vec<Pos2>>,
    stroke: Stroke,
}

impl Default for State {
    fn default() -> Self {
        Self {
            lines: Default::default(),
            stroke: Stroke::new(1.0, Color32::BLACK),
        }
    }
}

pub struct App {
    state: State,
    current_background_id: TextureId,
    background_offset: Pos2,
    zoom: f32,
    original_canvas_rect: Option<Rect>,
    texture_handles: HashMap<TextureId, TextureHandle>,
}

const IMAGES: &[(&str, &[u8])] =
    &include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/images.in"));

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut state = None;

        if let Some(storage) = cc.storage {
            state = Some(eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default());
        }

        let texture_handles: HashMap<TextureId, TextureHandle> = IMAGES
            .iter()
            .map(|(file_path, data)| {
                let file_name = match file_path.split('/').last() {
                    Some(file_name) => file_name,
                    None => "unknown_filename",
                };

                let name = file_name.to_string().replace(".png", "");

                let texture = cc.egui_ctx.load_texture(
                    name,
                    load_image_from_memory(data).unwrap(),
                    TextureOptions::default(),
                );

                (texture.id(), texture)
            })
            .collect();

        Self {
            current_background_id: texture_handles.keys().next().unwrap().to_owned(),
            texture_handles,
            background_offset: Pos2::ZERO,
            zoom: 1.0,
            original_canvas_rect: None,
            state: state.unwrap_or_default(),
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
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

                let original_background_size = self
                    .texture_handles
                    .get(&self.current_background_id)
                    .unwrap()
                    .size();

                let background_size = original_background_size
                    .iter()
                    .map(|x| (*x as f32))
                    .collect::<Vec<f32>>();

                let offset_x = self.background_offset.x;
                let offset_y = self.background_offset.y;

                let shrink_x = background_size[0] * self.zoom;
                let shrink_y = background_size[1] * self.zoom;

                let shrink = emath::vec2(shrink_x, shrink_y);

                let background_rect = Rect::from_min_size(
                    Pos2 {
                        x: offset_x,
                        y: offset_y,
                    },
                    emath::vec2(background_size[0], background_size[1]),
                )
                .shrink2(shrink);

                let canvas_size = ui.available_size_before_wrap();

                if self.original_canvas_rect.is_none() {
                    self.original_canvas_rect = Some(response.rect);

                    debug!("Original canvas rect: {:?}", self.original_canvas_rect);
                    debug!("Background rect: {:?}", background_rect);
                    debug!("Canvas size: {:?}", canvas_size);
                    debug!("Background size: {:?}", background_size);
                }

                painter.image(
                    self.current_background_id,
                    background_rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );

                let from_screen = emath::RectTransform::from_to(
                    background_rect,
                    self.original_canvas_rect.unwrap(),
                );

                let to_screen = from_screen.inverse();

                let which_mouse_button_down = response.ctx.input(|i| {
                    if i.pointer.primary_down() {
                        MouseDown::Primary
                    } else if i.pointer.secondary_down() {
                        MouseDown::Secondary
                    } else if i.pointer.middle_down() {
                        MouseDown::Middle
                    } else {
                        MouseDown::None
                    }
                });

                let scroll_delta = response.ctx.input(|i| i.raw_scroll_delta);

                let scroll_delta_y = scroll_delta.y * 1e-3;

                if scroll_delta_y != 0.0 {
                    self.zoom = (self.zoom + scroll_delta_y).clamp(-2.0, 2.0);
                }

                match response.interact_pointer_pos() {
                    Some(pointer_pos) => {
                        let canvas_pos = from_screen * pointer_pos;

                        match which_mouse_button_down {
                            MouseDown::Primary => {}
                            MouseDown::Secondary => {}
                            MouseDown::Middle => {
                                let drag_delta = response.drag_delta();

                                self.background_offset = self.background_offset.add(drag_delta);
                            }
                            MouseDown::None => (),
                        }
                    }
                    None => {}
                }

                if self.state.lines.is_empty() {
                    self.state.lines.push(vec![]);
                }

                let current_line = self.state.lines.last_mut().unwrap();

                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let canvas_pos = from_screen * pointer_pos;
                    if current_line.last() != Some(&canvas_pos) {
                        current_line.push(canvas_pos);
                        response.mark_changed();
                    }
                } else if !current_line.is_empty() {
                    self.state.lines.push(vec![]);
                    response.mark_changed();
                }

                let shapes = self
                    .state
                    .lines
                    .iter()
                    .filter(|line| line.len() >= 2)
                    .map(|line| {
                        let points: Vec<Pos2> = line.iter().map(|p| to_screen * *p).collect();
                        egui::Shape::line(points, self.state.stroke)
                    });

                painter.extend(shapes);

                response
            });
        });
    }
}

pub enum MouseDown {
    None,
    Primary,
    Secondary,
    Middle,
}

fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}
