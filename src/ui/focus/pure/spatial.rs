// Spatial adjacency logic for cross-panel navigation
//
// This module will contain pure functions for finding the nearest
// focusable element when navigating between regions/panels.
//
// Future implementation will use screen coordinates to compute
// which element is closest in the direction of navigation.

use crate::ui::focus::types::NavDirection;

/// A rectangle for spatial calculations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn center_top(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y)
    }

    pub fn center_bottom(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height)
    }

    pub fn left_center(&self) -> (f32, f32) {
        (self.x, self.y + self.height / 2.0)
    }

    pub fn right_center(&self) -> (f32, f32) {
        (self.x + self.width, self.y + self.height / 2.0)
    }
}

/// Calculate spatial distance between two points with directional weighting
///
/// This weights perpendicular distance more heavily so that aligned elements
/// are preferred over closer but off-axis elements.
pub fn spatial_distance(
    from: (f32, f32),
    to_rect: Rect,
    direction: NavDirection,
) -> f32 {
    let to = to_rect.center();

    let (dx, dy) = (to.0 - from.0, to.1 - from.1);

    // Weight perpendicular distance 2x to prefer aligned elements
    match direction {
        NavDirection::Up | NavDirection::Down => {
            dy.abs() + dx.abs() * 2.0
        }
        NavDirection::Left | NavDirection::Right => {
            dx.abs() + dy.abs() * 2.0
        }
    }
}

/// Find the index of the nearest element in a list of rects
pub fn find_nearest_index(
    source: Rect,
    targets: &[Rect],
    direction: NavDirection,
) -> Option<usize> {
    if targets.is_empty() {
        return None;
    }

    let source_edge = match direction {
        NavDirection::Up => source.center_top(),
        NavDirection::Down => source.center_bottom(),
        NavDirection::Left => source.left_center(),
        NavDirection::Right => source.right_center(),
    };

    targets
        .iter()
        .enumerate()
        .filter(|(_, target)| is_in_direction(source_edge, target.center(), direction))
        .min_by(|(_, a), (_, b)| {
            let dist_a = spatial_distance(source_edge, **a, direction);
            let dist_b = spatial_distance(source_edge, **b, direction);
            dist_a.partial_cmp(&dist_b).unwrap()
        })
        .map(|(idx, _)| idx)
}

/// Check if a target point is in the given direction from source
fn is_in_direction(source: (f32, f32), target: (f32, f32), direction: NavDirection) -> bool {
    match direction {
        NavDirection::Up => target.1 < source.1,
        NavDirection::Down => target.1 > source.1,
        NavDirection::Left => target.0 < source.0,
        NavDirection::Right => target.0 > source.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_center() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(rect.center(), (60.0, 45.0));
    }

    #[test]
    fn test_find_nearest_right() {
        let source = Rect::new(0.0, 0.0, 50.0, 50.0);
        let targets = vec![
            Rect::new(100.0, 0.0, 50.0, 50.0),   // Aligned, closer
            Rect::new(150.0, 100.0, 50.0, 50.0), // Farther and off-axis
        ];

        let nearest = find_nearest_index(source, &targets, NavDirection::Right);
        assert_eq!(nearest, Some(0));
    }

    #[test]
    fn test_is_in_direction() {
        let source = (50.0, 50.0);
        assert!(is_in_direction(source, (50.0, 10.0), NavDirection::Up));
        assert!(is_in_direction(source, (50.0, 90.0), NavDirection::Down));
        assert!(is_in_direction(source, (10.0, 50.0), NavDirection::Left));
        assert!(is_in_direction(source, (90.0, 50.0), NavDirection::Right));
    }
}
