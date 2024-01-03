use eframe::CreationContext;

use egui::epaint::CircleShape;
use egui::{
    emath, pos2, Color32, ColorImage, Pos2, Rect, Sense, Shape, Stroke, TextureHandle, TextureId,
    TextureOptions,
};
use egui::{epaint, DragValue};

use log::debug;
use reqwest::Client as ReqwestClient;

use shared::{ChangedLines, ClientID, DSRect, Line, StrokeX};
use shared::{Flag, Message};
use wasm_bindgen_futures::spawn_local;

use std::collections::HashMap;
use std::ops::Add;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use shared::Lines;
use shared::SPos2;

use anyhow::Result;

use async_recursion::async_recursion;

use lazy_static::lazy_static;

enum UserAgent {
    Chrome,
    Firefox,
    Edge,
    Other,
}

impl FromStr for UserAgent {
    type Err = ();

    fn from_str(user_agent: &str) -> Result<Self, Self::Err> {
        if user_agent.contains("Chrome") {
            Ok(UserAgent::Chrome)
        } else if user_agent.contains("Firefox") {
            Ok(UserAgent::Firefox)
        } else if user_agent.contains("Edg") {
            Ok(UserAgent::Edge)
        } else {
            Ok(UserAgent::Other)
        }
    }
}

pub struct App {
    #[allow(unused)]
    client_id: ClientID,
    host: String,
    is_dark: bool,
    texture_handles: HashMap<TextureId, TextureHandle>,
    current_background_id: TextureId,
    background_offset: Pos2,
    zoom: f32,
    lines: Arc<Mutex<Lines>>,
    changed_lines: Arc<Mutex<ChangedLines>>,
    current_line_id: Option<usize>,
    get_lines_timer: Option<f64>,
    stroke: Stroke,
    original_canvas_rect: Option<Rect>,
    user_agent: UserAgent,
    user_agent_position_correction: Pos2,
}

const IMAGES: &[(&str, &[u8])] = &include!(concat!("../../assets/", "/images.rs"));

impl App {
    pub fn new(cc: &CreationContext<'_>, host: String, client_id: String) -> Self {
        let user_agent = &cc.integration_info.web_info.user_agent;
        log::debug!("User agent: {}", user_agent);

        for (name, data) in IMAGES {
            log::trace!("File {} is {} bytes", name, data.len());
        }

        let user_agent = UserAgent::from_str(user_agent).unwrap();

        let user_agent_position_correction = match user_agent {
            UserAgent::Chrome => Pos2::new(0.5, 0.5),
            _ => Pos2::new(1.0, 1.0),
        };

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

        let first_background_id = texture_handles.iter().next().map(|(id, _)| *id).unwrap();

        let client_id = ClientID(
            client_id
                .trim()
                .parse::<u32>()
                .expect(format!("Failed to parse client id {}", client_id).as_str()),
        );

        Self {
            client_id: client_id,
            host: host.to_string(),
            is_dark: true,
            texture_handles,
            current_background_id: first_background_id,
            background_offset: Pos2::ZERO,
            zoom: 0.0,
            lines: Default::default(),
            changed_lines: Arc::new(Mutex::new(ChangedLines::default())),
            current_line_id: None,
            get_lines_timer: None,
            stroke: Stroke::new(5.0, Color32::RED),
            original_canvas_rect: None,
            user_agent,
            user_agent_position_correction,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.toggle_value(&mut self.is_dark, "🌓").changed().then(|| {
                    if self.is_dark {
                        ui.ctx().set_visuals(egui::Visuals::dark());
                    } else {
                        ui.ctx().set_visuals(egui::Visuals::light());
                    }
                });

                ui.horizontal(|ui| {
                    let epaint::Stroke { width, color } = &mut self.stroke;

                    ui.add(DragValue::new(width).speed(0.1).clamp_range(0.0..=1000.0))
                        .on_hover_text("Width");
                    ui.color_edit_button_srgba(color);
                    ui.label("Stroke");

                    let (_id, stroke_rect) = ui.allocate_space(ui.spacing().interact_size);
                    let left = stroke_rect.left_center();
                    let right = stroke_rect.right_center();
                    ui.painter().line_segment([left, right], (*width, *color));
                });

                let host = self.host.clone();

                if ui.button("Clear").clicked() {
                    spawn_local(async move {
                        match send_clear_lines_request(&host).await {
                            Ok(_) => (),
                            Err(e) => println!("Error sending clear lines request: {:?}", e),
                        };
                    });

                    let mut lines = self
                        .lines
                        .try_lock()
                        .expect(&format!("Failed to lock lines at line {}", line!()));

                    lines.clear();
                }

                let current_map_name = match self.texture_handles.get(&self.current_background_id) {
                    Some(texture) => texture.name(),
                    None => {
                        println!("Failed to get current map name");
                        return;
                    }
                };

                egui::ComboBox::from_label("Map")
                    .selected_text(format!("{}", current_map_name))
                    .show_ui(ui, |ui| {
                        for (id, texture) in self.texture_handles.iter() {
                            ui.selectable_value(
                                &mut self.current_background_id,
                                *id,
                                texture.name(),
                            );
                        }
                    });

                ui.label(format!("Zoom {:.2}", self.zoom));
                ui.button("➖").clicked().then(|| {
                    self.zoom = (self.zoom + 0.05).clamp(-2.0, 2.0);
                });
                ui.button("➕").clicked().then(|| {
                    self.zoom = (self.zoom - 0.05).clamp(-2.0, 2.0);
                });
            });

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

            let (mut response, painter) = ui.allocate_painter(canvas_size, Sense::drag());

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

            let from_screen =
                emath::RectTransform::from_to(background_rect, self.original_canvas_rect.unwrap());

            let to_screen = from_screen.inverse();

            let mut lines = self
                .lines
                .try_lock()
                .expect(&format!("Failed to lock lines at line {}", line!()));

            if lines.is_empty() {
                let current_line_id = rand::random::<usize>();
                self.current_line_id = Some(current_line_id);
                lines.0.insert(current_line_id, Line::new());
            }

            let current_line = match lines.0.get_mut(match &self.current_line_id {
                Some(current_line_id) => current_line_id,
                None => {
                    log::error!("Failed to get current line id");
                    return;
                }
            }) {
                Some(current_line) => current_line,
                None => {
                    log::error!("Failed to get current line");
                    return;
                }
            };

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

            let mut cursor_icon = None;

            let scroll_delta = response.ctx.input(|i| i.scroll_delta);

            let scroll_delta_y = scroll_delta.y * 1e-3;

            if scroll_delta_y != 0.0 {
                self.zoom = (self.zoom + scroll_delta_y).clamp(-2.0, 2.0);
            }

            match response.interact_pointer_pos() {
                Some(pointer_pos) => {
                    let canvas_pos = from_screen * pointer_pos;

                    match which_mouse_button_down {
                        MouseDown::Primary => {
                            if current_line.coordinates.last() != Some(&SPos2(canvas_pos)) {
                                current_line.coordinates.push(SPos2(canvas_pos));
                                current_line.stroke = StrokeX(self.stroke.clone());
                                response.mark_changed();
                            }
                        }
                        MouseDown::Secondary => {
                            cursor_icon = Some(get_eraser_on_pointer(pointer_pos));

                            let mut lines_to_remove: Vec<usize> = Vec::new();

                            const TOLERANCE: f32 = 10.0;

                            for (line_id, line) in lines.iter() {
                                for coordinate in line.coordinates.iter() {
                                    let distance = coordinate.0.distance(canvas_pos);

                                    if distance < TOLERANCE {
                                        lines_to_remove.push(*line_id);
                                        break;
                                    }
                                }
                            }

                            let mut changed_lines = self.changed_lines.try_lock().expect(&format!(
                                "Failed to lock changed lines at line {}",
                                line!()
                            ));

                            for line_id in lines_to_remove {
                                lines.0.remove(&line_id);
                                changed_lines.0.insert(line_id);

                                response.mark_changed();
                            }
                        }
                        MouseDown::Middle => {
                            let drag_delta = response.drag_delta();

                            self.background_offset = self.background_offset.add(drag_delta);
                        }
                        MouseDown::None => (),
                    }

                    drop(lines);
                }
                None => {
                    let are_coordinates_empty = current_line.coordinates.is_empty();

                    drop(lines);

                    if !are_coordinates_empty {
                        let lines = {
                            let unlocked = self
                                .lines
                                .try_lock()
                                .expect(&format!("Failed to lock lines at line {}", line!()));
                            unlocked.clone()
                        };

                        if let Some(last_line) = lines.iter().last() {
                            log::debug!("Last line: {:?}", last_line);
                        }

                        let host = self.host.clone();
                        let canvas_rect = Some(DSRect(self.original_canvas_rect.unwrap()));

                        spawn_local(async move {
                            let message = Message {
                                lines,
                                changed_lines: None,
                                flag: None,
                                canvas_rect,
                            };

                            match send_message(&host, message).await {
                                Ok(_) => (),
                                Err(e) => println!("Error: {:?} at Line: {}", e, line!()),
                            };
                        });

                        let mut lines = self
                            .lines
                            .try_lock()
                            .expect(&format!("Failed to lock lines at line {}", line!()));

                        let current_line_id = rand::random::<usize>();

                        self.current_line_id = Some(current_line_id);

                        lines.0.insert(current_line_id, Line::new());

                        response.mark_changed();
                    }

                    let mut changed_lines = self
                        .changed_lines
                        .try_lock()
                        .expect(&format!("Failed to lock changed lines at line {}", line!()));

                    if !changed_lines.0.is_empty() {
                        let changed_lines_cloned = changed_lines.clone();

                        let host = self.host.clone();

                        spawn_local(async move {
                            match send_deleted_lines(&host, changed_lines_cloned).await {
                                Ok(_) => (),
                                Err(e) => println!("Error: {:?} at Line: {}", e, line!()),
                            };
                        });

                        changed_lines.0.clear();
                    }
                }
            }

            let lines = {
                let unlocked = self
                    .lines
                    .try_lock()
                    .expect(&format!("Failed to lock lines at line {}", line!()));
                unlocked.clone()
            };

            let shapes = lines
                .iter()
                .filter(|(_line_id, line)| line.coordinates.len() >= 2)
                .map(|(_line_id, line)| {
                    let points: Vec<Pos2> =
                        line.coordinates.iter().map(|p| to_screen * **p).collect();
                    egui::Shape::line(points, *line.stroke)
                });

            painter.extend(shapes);

            match cursor_icon {
                Some(cursor_icon) => {
                    painter.add(cursor_icon);
                }
                None => (),
            }
        });

        let seconds_since = chrono::offset::Local::now().timestamp_millis() as f64 / 1000.0;

        if self.get_lines_timer.is_none() {
            self.get_lines_timer = Some(seconds_since);
        }

        if seconds_since - self.get_lines_timer.unwrap() > 0.5 {
            let lines = self.lines.clone();
            let changed_lines = self.changed_lines.clone();

            let host = self.host.clone();
            let canvas_rect = match self.original_canvas_rect {
                Some(original_canvas_rect) => DSRect(original_canvas_rect),
                None => {
                    log::error!("Failed to get canvas size before `get_message`");
                    return;
                }
            };

            spawn_local(async move {
                let get_lines = match get_message(&host).await {
                    Ok(get_lines) => get_lines,
                    Err(e) => {
                        println!("Error: {:?} at Line: {}", e, line!());
                        return;
                    }
                };

                let mut lines = lines
                    .try_lock()
                    .expect(&format!("Failed to lock lines at line {}", line!()));

                let flag = get_lines.flag;

                match flag {
                    Some(flag) => match flag {
                        Flag::Clear => lines.clear(),
                    },
                    None => {
                        let mut other_changed_lines = match get_lines.changed_lines {
                            Some(changed_lines) => changed_lines,
                            None => ChangedLines::default(),
                        };

                        let changed_lines = changed_lines
                            .try_lock()
                            .expect(&format!("Failed to lock changed lines at line {}", line!()));

                        other_changed_lines
                            .0
                            .extend(changed_lines.0.iter().cloned());

                        lines.merge(
                            get_lines.lines,
                            &Some(other_changed_lines),
                            &canvas_rect,
                            shared::MergeMode::ToCanvas,
                        );
                    }
                }
            });

            self.get_lines_timer = Some(seconds_since);
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

#[async_recursion(?Send)]
async fn send_message(host: &str, message: Message) -> Result<()> {
    let client = ReqwestClient::new();

    let body = serde_json::to_string(&message).unwrap() + "\r\n\r\n";

    match client
        .post("http://".to_string() + host + "/send_lines")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to send lines")),
    }
}

#[async_recursion(?Send)]
async fn get_message(host: &str) -> Result<Message> {
    let client = ReqwestClient::new();

    let response = client
        .get("http://".to_string() + host + "/get_lines")
        .send()
        .await
        .unwrap();

    let body = response.text().await.unwrap();

    Ok(serde_json::from_str::<Message>(&body).unwrap())
}

#[async_recursion(?Send)]
async fn send_deleted_lines(host: &str, changed_lines: ChangedLines) -> Result<()> {
    let client = ReqwestClient::new();

    let body = serde_json::to_string(&changed_lines).unwrap() + "\r\n\r\n";

    match client
        .post("http://".to_string() + host + "/delete_lines")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to send changed lines")),
    }
}

#[async_recursion(?Send)]
async fn send_clear_lines_request(host: &str) -> Result<()> {
    let client = ReqwestClient::new();

    let body = r#""#;

    match client
        .post("http://".to_string() + host + "/clear_lines")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to send clear lines")),
    }
}

pub enum MouseDown {
    None,
    Primary,
    Secondary,
    Middle,
}

lazy_static! {
    static ref CIRCLE: CircleShape = {
        let radius = 5.0;

        let center = Pos2::new(0.0, 0.0);

        let fill = Color32::TRANSPARENT;

        let stroke = Stroke::new(3.0, Color32::WHITE);

        CircleShape {
            center,
            radius,
            fill,
            stroke,
        }
    };
}

fn get_eraser_on_pointer(pointer_pos: Pos2) -> Shape {
    let mut circle_shape = CIRCLE.to_owned();
    circle_shape.center = pointer_pos;

    egui::Shape::Circle(circle_shape)
}
