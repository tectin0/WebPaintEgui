use core::num;
use std::collections::HashMap;

use egui::{
    emath, pos2, vec2, Color32, ColorImage, Context, Frame, Pos2, Rect, Sense, Stroke, TextureId,
    TextureOptions, Ui, Window,
};
use egui::{Style, TextureHandle};
use getrandom::getrandom;
use log::debug;
use shared::{Line, Lines};

use std::ops::Add;

use crate::requests::{execute, send_get_request, send_post_request};

const IMAGES: &[(&str, &[u8])] = &include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "\\..\\assets\\images.in"
));

const UPDATE_FREQUENCY: f64 = 5.0;

pub struct Channel<T> {
    sender: std::sync::mpsc::Sender<T>,
    receiver: std::sync::mpsc::Receiver<T>,
}

pub struct App {
    lines: Lines,
    stroke: Stroke,
    current_background_id: TextureId,
    background_offset: Pos2,
    zoom: f32,
    original_canvas_rect: Option<Rect>,
    texture_handles: HashMap<TextureId, TextureHandle>,
    new_lines_channel: Channel<Lines>,
    num_connections_channel: Channel<u64>,
    last_update: web_time::Instant,
    last_id: u64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
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

        let lines_channel = std::sync::mpsc::channel::<Lines>();

        let lines_channel = Channel {
            sender: lines_channel.0,
            receiver: lines_channel.1,
        };

        let num_connections_chanenl = std::sync::mpsc::channel::<u64>();

        let num_connections_channel = Channel {
            sender: num_connections_chanenl.0,
            receiver: num_connections_chanenl.1,
        };

        let sender = num_connections_channel.sender.clone();

        execute(async move {
            let result = send_get_request("http://localhost:8080/backend/num_connections").await;

            match result {
                Ok(num_connections) => {
                    sender
                        .send(num_connections.parse::<u64>().unwrap())
                        .unwrap();
                }
                Err(e) => {
                    log::error!("Error: {:?}", e);
                }
            }
        });

        Self {
            lines: Default::default(),
            stroke: Stroke {
                width: 3.0,
                color: Color32::BLACK,
            },
            current_background_id: texture_handles.keys().next().unwrap().to_owned(),
            texture_handles,
            background_offset: Pos2::ZERO,
            zoom: 0.0,
            original_canvas_rect: None,
            new_lines_channel: lines_channel,
            num_connections_channel,
            last_update: web_time::Instant::now(),
            last_id: 0,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_update.elapsed().as_secs_f64() > UPDATE_FREQUENCY {
            log::info!("Getting lines from backend");
            let sender = self.new_lines_channel.sender.clone();

            execute(async move {
                let lines = send_get_request("http://localhost:8080/backend/lines").await;

                match lines {
                    Ok(lines) => {
                        sender.send(lines.into()).unwrap();
                    }
                    Err(e) => {
                        log::error!("Error: {:?}", e);
                    }
                }
            });

            self.last_update = web_time::Instant::now();
        }

        if let Ok(new_lines) = self.new_lines_channel.receiver.try_recv() {
            log::info!("Updating lines with new lines");

            let last_line = self.lines.get(&self.last_id).unwrap().clone();

            self.lines = new_lines;

            if !self.lines.contains_key(&self.last_id) {
                self.lines.insert(self.last_id, last_line);
            }
        }

        if let Ok(num_connections) = self.num_connections_channel.receiver.try_recv() {
            log::debug!("Number of connections: {}", num_connections);

            self.stroke.color = match num_connections {
                0 => Color32::RED,
                1 => Color32::GREEN,
                2 => Color32::YELLOW,
                3 => Color32::ORANGE,
                4 => Color32::LIGHT_BLUE,
                5 => Color32::KHAKI,
                6 => Color32::GOLD,
                _ => Color32::BLUE,
            };
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);

                ui.add(&mut self.stroke);

                ui.button("Clear")
                    .on_hover_text("Clear the canvas")
                    .clicked()
                    .then(|| {
                        log::info!("Sending clear request");

                        self.lines.0.clear();

                        execute(async move {
                            match send_post_request("http://localhost:8080/backend/clear", "").await
                            {
                                Ok(_) => {
                                    log::debug!("Successfully cleared lines");
                                }
                                Err(e) => {
                                    log::error!("Error: {:?}", e);
                                }
                            }
                        });
                    })
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
                    self.zoom = (self.zoom - scroll_delta_y).clamp(-2.0, 2.0);
                }

                if self.lines.0.is_empty() {
                    let first_id = get_random_u64();

                    self.lines.0.insert(first_id, Line::new(self.stroke));

                    self.last_id = first_id;
                }

                let current_line = self.lines.0.get_mut(&self.last_id).unwrap();

                match response.interact_pointer_pos() {
                    Some(pointer_pos) => {
                        let canvas_pos = from_screen * pointer_pos;

                        match which_mouse_button_down {
                            MouseDown::Primary => {
                                if current_line.last() != Some(&canvas_pos) {
                                    current_line.push(canvas_pos);
                                    response.mark_changed();
                                }
                            }
                            MouseDown::Secondary => {
                                let mut lines_to_remove: Vec<u64> = Vec::new();

                                const TOLERANCE: f32 = 10.0;

                                for (line_id, line) in self.lines.iter() {
                                    for point in line.iter() {
                                        let distance = (*point - canvas_pos).length();

                                        if distance < TOLERANCE {
                                            lines_to_remove.push(*line_id);
                                            break;
                                        }
                                    }
                                }

                                if !lines_to_remove.is_empty() {
                                    log::info!("Removing lines: {:?}", lines_to_remove);

                                    for line_id in lines_to_remove.iter() {
                                        self.lines.remove(line_id);
                                    }

                                    execute(async move {
                                        match send_post_request(
                                            "http://localhost:8080/backend/remove_lines",
                                            &serde_json::to_string(&lines_to_remove).unwrap(),
                                        )
                                        .await
                                        {
                                            Ok(_) => {
                                                log::debug!("Successfully removed lines");
                                            }
                                            Err(e) => {
                                                log::error!("Error: {:?}", e);
                                            }
                                        }
                                    });

                                    response.mark_changed();
                                }
                            }
                            MouseDown::Middle => {
                                let drag_delta = response.drag_delta();

                                self.background_offset = self.background_offset.add(drag_delta);
                            }
                            MouseDown::None => (),
                        }
                    }
                    None => {
                        if !current_line.is_empty() {
                            let mut buffer = [0u8; 8];
                            getrandom(&mut buffer).unwrap();
                            let id = u64::from_ne_bytes(buffer);

                            let lines = self.lines.clone();

                            log::info!("Sending lines to backend");

                            execute(async move {
                                match send_post_request(
                                    "http://localhost:8080/backend/lines",
                                    &lines.to_string(),
                                )
                                .await
                                {
                                    Ok(_) => {
                                        log::debug!("Successfully sent lines to backend");
                                    }
                                    Err(e) => {
                                        log::error!("Error: {:?}", e);
                                    }
                                }
                            });

                            self.lines.insert(id, Line::new(self.stroke));
                            self.last_id = id;

                            response.mark_changed();
                        }
                    }
                }

                let shapes =
                    self.lines
                        .iter()
                        .filter(|(id, line)| line.len() >= 2)
                        .map(|(id, line)| {
                            let points: Vec<Pos2> = line.iter().map(|p| to_screen * *p).collect();
                            egui::Shape::line(points, line.stroke)
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

fn get_random_u64() -> u64 {
    let mut buffer = [0u8; 8];
    getrandom(&mut buffer).unwrap();
    u64::from_ne_bytes(buffer)
}
