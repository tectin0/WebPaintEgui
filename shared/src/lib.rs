pub mod config;

use std::{collections::HashMap, fmt::Display, ops::Deref};

use egui::{ahash::HashSet, Color32, Pos2, Rgba, Stroke};
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
    pub fn new() -> Self {
        Self(rand::random())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SPos2(pub Pos2);

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
    pub fn merge(&mut self, other: Self, changed_lines: &Option<ChangedLines>) {
        let are_changed_lines = changed_lines.is_some();

        let mut changed_lines = changed_lines.clone().unwrap_or_default();

        for (line_id, other_line) in other.iter() {
            match self.0.get_mut(line_id) {
                Some(line) => {
                    if are_changed_lines && changed_lines.0.remove(line_id) {
                        *line = other_line.clone();
                    }
                }
                None => {
                    self.0.insert(*line_id, other_line.clone());
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

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct Message {
    pub lines: Lines,
    pub changed_lines: Option<ChangedLines>,
    pub flag: Option<Flag>,
}
