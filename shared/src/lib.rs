use std::collections::BTreeMap;

use egui::{Pos2, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Line {
    pub points: Vec<Pos2>,
    pub stroke: Stroke,
}

impl Line {
    pub fn new(stroke: Stroke) -> Self {
        Self {
            points: Vec::new(),
            stroke,
        }
    }
}

impl std::ops::Deref for Line {
    type Target = Vec<Pos2>;

    fn deref(&self) -> &Self::Target {
        &self.points
    }
}

impl std::ops::DerefMut for Line {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.points
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Lines(pub BTreeMap<u64, Line>);

impl Lines {
    pub fn update_from_other(&mut self, other: Lines) {
        self.0.extend(other.0);
    }
}

impl std::ops::Deref for Lines {
    type Target = BTreeMap<u64, Line>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Lines {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ToString for Lines {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl From<String> for Lines {
    fn from(s: String) -> Self {
        serde_json::from_str(&s).unwrap()
    }
}
