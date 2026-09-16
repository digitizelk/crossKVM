use crate::protocol::{DisplayInfo, ScreenEdge};
use serde::{Deserialize, Serialize};

/// Relative positioning of a neighbor peer screen
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborPlacement {
    pub peer_id: String,
    pub edge: ScreenEdge,
    /// Offset ratio along the edge, between -1.0 and 1.0 (0.0 = centered / aligned)
    pub offset_ratio: f32,
}

/// Screen boundary definition for transition detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenBoundary {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
    /// Threshold in pixels to push into the edge before triggering a jump
    pub edge_threshold_px: f32,
    /// Corner dead-zone in pixels to prevent accidental switching when clicking window controls
    pub corner_deadzone_px: f32,
}

impl ScreenBoundary {
    pub fn from_display(display: &DisplayInfo, threshold_px: f32, corner_deadzone_px: f32) -> Self {
        Self {
            x_min: display.x as f32,
            y_min: display.y as f32,
            x_max: (display.x + display.width as i32) as f32,
            y_max: (display.y + display.height as i32) as f32,
            edge_threshold_px: threshold_px.max(1.0),
            corner_deadzone_px: corner_deadzone_px.max(0.0),
        }
    }

    /// Check if cursor (x, y) hit a screen boundary edge.
    /// Returns Some((ScreenEdge, normalized_ratio)) if hitting an edge, None otherwise.
    pub fn check_edge_hit(&self, x: f32, y: f32) -> Option<(ScreenEdge, f32)> {
        let width = self.x_max - self.x_min;
        let height = self.y_max - self.y_min;

        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        // Check if cursor is in corner dead zones
        let in_top_left = (x - self.x_min) < self.corner_deadzone_px && (y - self.y_min) < self.corner_deadzone_px;
        let in_top_right = (self.x_max - x) < self.corner_deadzone_px && (y - self.y_min) < self.corner_deadzone_px;
        let in_bottom_left = (x - self.x_min) < self.corner_deadzone_px && (self.y_max - y) < self.corner_deadzone_px;
        let in_bottom_right = (self.x_max - x) < self.corner_deadzone_px && (self.y_max - y) < self.corner_deadzone_px;

        if in_top_left || in_top_right || in_bottom_left || in_bottom_right {
            return None;
        }

        // Check Right edge hit
        if x >= self.x_max - self.edge_threshold_px {
            let ratio = ((y - self.y_min) / height).clamp(0.0, 1.0);
            return Some((ScreenEdge::Right, ratio));
        }

        // Check Left edge hit
        if x <= self.x_min + self.edge_threshold_px {
            let ratio = ((y - self.y_min) / height).clamp(0.0, 1.0);
            return Some((ScreenEdge::Left, ratio));
        }

        // Check Bottom edge hit
        if y >= self.y_max - self.edge_threshold_px {
            let ratio = ((x - self.x_min) / width).clamp(0.0, 1.0);
            return Some((ScreenEdge::Bottom, ratio));
        }

        // Check Top edge hit
        if y <= self.y_min + self.edge_threshold_px {
            let ratio = ((x - self.x_min) / width).clamp(0.0, 1.0);
            return Some((ScreenEdge::Top, ratio));
        }

        None
    }

    /// Calculate entry coordinate on destination display when coming from an edge at normalized_ratio
    pub fn calculate_entry_point(&self, entering_edge: ScreenEdge, normalized_ratio: f32) -> (f32, f32) {
        let width = self.x_max - self.x_min;
        let height = self.y_max - self.y_min;
        let ratio = normalized_ratio.clamp(0.0, 1.0);

        match entering_edge {
            ScreenEdge::Left => {
                // Enters at the Left edge, y proportional
                (self.x_min + 2.0, self.y_min + (height * ratio))
            }
            ScreenEdge::Right => {
                // Enters at Right edge, y proportional
                (self.x_max - 2.0, self.y_min + (height * ratio))
            }
            ScreenEdge::Top => {
                // Enters at Top edge, x proportional
                (self.x_min + (width * ratio), self.y_min + 2.0)
            }
            ScreenEdge::Bottom => {
                // Enters at Bottom edge, x proportional
                (self.x_min + (width * ratio), self.y_max - 2.0)
            }
        }
    }
}

/// Topology manager that coordinates layout of multiple machines
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenTopology {
    pub local_peer_id: String,
    pub local_displays: Vec<DisplayInfo>,
    pub neighbors: Vec<NeighborPlacement>,
    pub edge_threshold_px: f32,
    pub corner_deadzone_px: f32,
}

impl ScreenTopology {
    pub fn new(local_peer_id: impl Into<String>) -> Self {
        Self {
            local_peer_id: local_peer_id.into(),
            local_displays: Vec::new(),
            neighbors: Vec::new(),
            edge_threshold_px: 2.0,
            corner_deadzone_px: 20.0,
        }
    }

    pub fn add_neighbor(&mut self, peer_id: impl Into<String>, edge: ScreenEdge) {
        let peer_id = peer_id.into();
        self.neighbors.retain(|n| n.peer_id != peer_id);
        self.neighbors.push(NeighborPlacement {
            peer_id,
            edge,
            offset_ratio: 0.0,
        });
    }

    pub fn find_neighbor_at_edge(&self, edge: ScreenEdge) -> Option<&NeighborPlacement> {
        self.neighbors.iter().find(|n| n.edge == edge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_hit_detection() {
        let display = DisplayInfo {
            id: 1,
            name: "Primary".into(),
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            is_primary: true,
            x: 0,
            y: 0,
        };

        let boundary = ScreenBoundary::from_display(&display, 2.0, 10.0);

        // Center should not hit
        assert_eq!(boundary.check_edge_hit(960.0, 540.0), None);

        // Right edge middle
        let hit = boundary.check_edge_hit(1919.0, 540.0);
        assert!(hit.is_some());
        let (edge, ratio) = hit.unwrap();
        assert_eq!(edge, ScreenEdge::Right);
        assert!((ratio - 0.5).abs() < 0.01);

        // Corner should be ignored due to deadzone
        assert_eq!(boundary.check_edge_hit(1919.0, 5.0), None);
    }

    #[test]
    fn test_entry_point_calculation() {
        let target_display = DisplayInfo {
            id: 2,
            name: "Windows PC".into(),
            width: 3840,
            height: 2160,
            scale_factor: 2.0,
            is_primary: true,
            x: 0,
            y: 0,
        };

        let boundary = ScreenBoundary::from_display(&target_display, 2.0, 10.0);

        // Entering at Left edge at 50% height
        let (x, y) = boundary.calculate_entry_point(ScreenEdge::Left, 0.5);
        assert_eq!(x, 2.0);
        assert_eq!(y, 1080.0);
    }
}
