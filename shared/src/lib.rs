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

    pub fn from_canvas(&mut self, canvas_rect: &egui::Rect) {
        for pos in self.points.iter_mut() {
            pos.x = (pos.x - canvas_rect.min.x) / canvas_rect.width();
            pos.y = (pos.y - canvas_rect.min.y) / canvas_rect.height();
        }
    }

    pub fn to_canvas(&mut self, canvas_rect: &egui::Rect) {
        for pos in self.points.iter_mut() {
            pos.x = pos.x * canvas_rect.width() + canvas_rect.min.x;
            pos.y = pos.y * canvas_rect.height() + canvas_rect.min.y;
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

    pub fn from_canvas(&mut self, canvas_rect: &egui::Rect) {
        for line in self.0.values_mut() {
            line.from_canvas(canvas_rect);
        }
    }

    pub fn to_canvas(&mut self, canvas_rect: &egui::Rect) {
        for line in self.0.values_mut() {
            line.to_canvas(canvas_rect);
        }
    }
}

impl FromIterator<(u64, Line)> for Lines {
    fn from_iter<T: IntoIterator<Item = (u64, Line)>>(iter: T) -> Self {
        let mut lines = Lines::default();
        for (id, line) in iter {
            lines.0.insert(id, line);
        }
        lines
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
