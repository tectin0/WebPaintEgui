pub mod config;

use std::{collections::HashMap, fmt::Display, ops::Deref};

use egui::{ahash::HashSet, Color32, Pos2, Rect, Rgba, Stroke, Vec2};
use serde::{Deserialize, Serialize};

use anyhow::Result;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct ChangedLines(pub HashSet<usize>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy, Eq, Hash)]
pub struct ClientID(pub u32);

impl Display for ClientID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for ClientID {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ClientID {
    pub fn new(number_of_connections: u32) -> Self {
        let mut random_number = rand::random::<u32>();
        random_number = (random_number / 100) * 100;
        random_number = random_number + number_of_connections;

        Self(random_number)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SPos2(pub Pos2);

impl SPos2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Pos2::new(x, y))
    }
}

impl From<Pos2> for SPos2 {
    fn from(pos: Pos2) -> Self {
        Self(pos)
    }
}

impl From<Vec2> for SPos2 {
    fn from(vec: Vec2) -> Self {
        Self(Pos2::new(vec.x, vec.y))
    }
}

impl Serialize for SPos2 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0.x, self.0.y).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SPos2 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (x, y) = Deserialize::deserialize(deserializer)?;

        Ok(Self(Pos2::new(x, y)))
    }
}

impl Deref for SPos2 {
    type Target = Pos2;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Flag {
    Clear,
}

#[derive(Debug, Clone)]
pub struct StrokeX(pub Stroke);

impl Default for StrokeX {
    fn default() -> Self {
        Self(Stroke {
            color: Color32::RED.into(),
            width: 5.0,
            ..Default::default()
        })
    }
}

impl Serialize for StrokeX {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let color = self.0.color;
        let width = self.0.width;

        (color.r(), color.g(), color.b(), color.a(), width).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StrokeX {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (r, g, b, a, width) = Deserialize::deserialize(deserializer)?;

        Ok(Self(Stroke {
            color: Rgba::from_rgba_premultiplied(r, g, b, a).into(),
            width,
            ..Default::default()
        }))
    }
}

impl Deref for StrokeX {
    type Target = Stroke;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone)]
pub struct Line {
    pub coordinates: Vec<SPos2>,
    pub stroke: StrokeX,
    pub flag: Option<Flag>,
}

impl Line {
    pub fn new() -> Self {
        Self {
            coordinates: Vec::new(),
            stroke: StrokeX::default(),
            flag: None,
        }
    }

    pub fn from_canvas(&mut self, canvas_rect: &DSRect) {
        for pos in self.coordinates.iter_mut() {
            pos.0.x = (pos.0.x - canvas_rect.min.x) / canvas_rect.width();
            pos.0.y = (pos.0.y - canvas_rect.min.y) / canvas_rect.height();
        }
    }

    pub fn to_canvas(&mut self, canvas_rect: &DSRect) {
        for pos in self.coordinates.iter_mut() {
            pos.0.x = pos.0.x * canvas_rect.width() + canvas_rect.min.x;
            pos.0.y = pos.0.y * canvas_rect.height() + canvas_rect.min.y;
        }
    }
}

pub enum MergeMode {
    ToCanvas,
    FromCanvas,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct Lines(pub HashMap<usize, Line>);

impl Deref for Lines {
    type Target = HashMap<usize, Line>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Lines {
    pub fn merge(
        &mut self,
        other: Self,
        changed_lines: &Option<ChangedLines>,
        canvas_rect: &DSRect,
        merge_mode: MergeMode,
    ) {
        let are_changed_lines = changed_lines.is_some();

        let mut changed_lines = changed_lines.clone().unwrap_or_default();

        for (line_id, other_line) in other.iter() {
            match self.0.get_mut(line_id) {
                Some(line) => {
                    if are_changed_lines && changed_lines.0.remove(line_id) {
                        *line = other_line.clone();

                        match merge_mode {
                            MergeMode::ToCanvas => {
                                line.to_canvas(canvas_rect);
                            }
                            MergeMode::FromCanvas => {
                                line.from_canvas(canvas_rect);
                            }
                        }
                    }
                }
                None => {
                    let mut line = other_line.clone();

                    match merge_mode {
                        MergeMode::ToCanvas => {
                            line.to_canvas(canvas_rect);
                        }
                        MergeMode::FromCanvas => {
                            line.from_canvas(canvas_rect);
                        }
                    }

                    self.0.insert(*line_id, line);
                }
            }
        }

        for line_id in changed_lines.0.drain() {
            self.0.remove(&line_id);
        }

        assert!(changed_lines.0.is_empty());
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Debug)]
pub struct Peer(pub String);

impl Peer {
    pub fn ip(&self) -> Result<&str> {
        match self.0.split_once(':') {
            Some((ip, _)) => Ok(ip),
            None => Err(anyhow::anyhow!("Failed to get ip from peer")),
        }
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.split_once(':') {
            Some((ip, port)) => write!(f, "{}:{}", ip, port),
            None => Err(std::fmt::Error),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DSRect(pub Rect);

impl Deref for DSRect {
    type Target = Rect;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for DSRect {
    fn default() -> Self {
        Self(Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)))
    }
}

impl Serialize for DSRect {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let min = self.0.min;
        let max = self.0.max;

        (min.x, min.y, max.x, max.y).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DSRect {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (min_x, min_y, max_x, max_y) = Deserialize::deserialize(deserializer)?;

        Ok(Self(Rect::from_min_max(
            Pos2::new(min_x, min_y),
            Pos2::new(max_x, max_y),
        )))
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct Message {
    pub lines: Lines,
    pub changed_lines: Option<ChangedLines>,
    pub flag: Option<Flag>,
    pub canvas_rect: Option<DSRect>,
}
