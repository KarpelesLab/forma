//! A retained list of draw primitives, lowered to an `oxideav-core`
//! [`VectorFrame`] for rasterization.
//!
//! `forma-core` rebuilds a `Scene` from the widget tree each time the visible
//! state changes; `forma-render` then rasterizes and presents it. The scene
//! works entirely in **logical pixels** — DPI scaling is applied at raster
//! time via the frame's view box (see [`Scene::into_vector_frame`]).

use crate::Color;
use crate::convert::to_ox_point;
use forma_geometry::{Rect, Size};
use oxideav_core::{
    Group, Node, Paint, Path, PathNode, Point as OxPoint, Rgba, Stroke, VectorFrame, ViewBox,
};

/// Control-point offset for approximating a quarter circle with a cubic
/// Bézier (the standard `4/3·(√2 − 1)` "kappa" constant).
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// A builder of vector draw primitives in logical-pixel space.
#[derive(Clone, Debug)]
pub struct Scene {
    logical_size: Size,
    nodes: Vec<Node>,
}

impl Scene {
    /// Create an empty scene covering `logical_size`.
    pub fn new(logical_size: Size) -> Self {
        Self {
            logical_size,
            nodes: Vec::new(),
        }
    }

    /// The scene's extent in logical pixels.
    #[inline]
    pub fn logical_size(&self) -> Size {
        self.logical_size
    }

    /// Number of top-level primitives queued.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Fill an axis-aligned rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let path = rect_path(rect);
        self.nodes.push(Node::Path(
            PathNode::new(path).with_fill(Paint::Solid(color.into())),
        ));
    }

    /// Fill a rectangle with rounded corners (corner `radius` in logical
    /// pixels, clamped to half the shorter side).
    pub fn fill_round_rect(&mut self, rect: Rect, radius: f64, color: Color) {
        let path = round_rect_path(rect, radius);
        self.nodes.push(Node::Path(
            PathNode::new(path).with_fill(Paint::Solid(color.into())),
        ));
    }

    /// Stroke the outline of a rectangle with the given line `width`.
    pub fn stroke_rect(&mut self, rect: Rect, color: Color, width: f64) {
        let path = rect_path(rect);
        let stroke = Stroke::solid(width as f32, Rgba::from(color));
        self.nodes
            .push(Node::Path(PathNode::new(path).with_stroke(stroke)));
    }

    /// Escape hatch: push a pre-built oxideav scene-graph node. Lets callers
    /// emit text runs, images, gradients, or clips that the typed helpers
    /// don't yet cover.
    pub fn push_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Lower the scene to an `oxideav-core` [`VectorFrame`].
    ///
    /// The frame carries a view box equal to the logical size, so the
    /// rasterizer maps logical pixels onto whatever physical canvas size the
    /// renderer was constructed with — that mapping *is* the DPI scale.
    pub fn into_vector_frame(self) -> VectorFrame {
        let w = self.logical_size.width as f32;
        let h = self.logical_size.height as f32;
        let root = Group {
            children: self.nodes,
            ..Group::new()
        };
        VectorFrame::new(w, h)
            .with_view_box(ViewBox {
                min_x: 0.0,
                min_y: 0.0,
                width: w,
                height: h,
            })
            .with_root(root)
    }
}

fn rect_path(rect: Rect) -> Path {
    let (x0, y0, x1, y1) = (rect.min_x(), rect.min_y(), rect.max_x(), rect.max_y());
    let mut p = Path::new();
    p.move_to(OxPoint {
        x: x0 as f32,
        y: y0 as f32,
    });
    p.line_to(OxPoint {
        x: x1 as f32,
        y: y0 as f32,
    });
    p.line_to(OxPoint {
        x: x1 as f32,
        y: y1 as f32,
    });
    p.line_to(OxPoint {
        x: x0 as f32,
        y: y1 as f32,
    });
    p.close();
    p
}

fn round_rect_path(rect: Rect, radius: f64) -> Path {
    let r = radius.max(0.0).min(rect.width().min(rect.height()) / 2.0);
    if r <= 0.0 {
        return rect_path(rect);
    }
    let (x0, y0, x1, y1) = (rect.min_x(), rect.min_y(), rect.max_x(), rect.max_y());
    let k = r * KAPPA;
    use forma_geometry::Point as P;
    let mut p = Path::new();
    // Start at the top edge just right of the top-left corner, go clockwise.
    p.move_to(to_ox_point(P::new(x0 + r, y0)));
    p.line_to(to_ox_point(P::new(x1 - r, y0)));
    p.cubic_to(
        to_ox_point(P::new(x1 - r + k, y0)),
        to_ox_point(P::new(x1, y0 + r - k)),
        to_ox_point(P::new(x1, y0 + r)),
    );
    p.line_to(to_ox_point(P::new(x1, y1 - r)));
    p.cubic_to(
        to_ox_point(P::new(x1, y1 - r + k)),
        to_ox_point(P::new(x1 - r + k, y1)),
        to_ox_point(P::new(x1 - r, y1)),
    );
    p.line_to(to_ox_point(P::new(x0 + r, y1)));
    p.cubic_to(
        to_ox_point(P::new(x0 + r - k, y1)),
        to_ox_point(P::new(x0, y1 - r + k)),
        to_ox_point(P::new(x0, y1 - r)),
    );
    p.line_to(to_ox_point(P::new(x0, y0 + r)));
    p.cubic_to(
        to_ox_point(P::new(x0, y0 + r - k)),
        to_ox_point(P::new(x0 + r - k, y0)),
        to_ox_point(P::new(x0 + r, y0)),
    );
    p.close();
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geometry::Point;

    #[test]
    fn scene_lowers_to_frame_with_viewbox() {
        let mut scene = Scene::new(Size::new(200.0, 100.0));
        scene.fill_rect(Rect::from_xywh(10.0, 10.0, 50.0, 50.0), Color::WHITE);
        assert_eq!(scene.len(), 1);
        let frame = scene.into_vector_frame();
        assert_eq!((frame.width, frame.height), (200.0, 100.0));
        let vb = frame.view_box.expect("view box set");
        assert_eq!((vb.width, vb.height), (200.0, 100.0));
        assert_eq!(frame.root.children.len(), 1);
    }

    #[test]
    fn zero_radius_round_rect_is_plain_rect() {
        let path = round_rect_path(Rect::from_xywh(0.0, 0.0, 10.0, 10.0), 0.0);
        // Plain rect: move + 3 lines + close == 5 commands.
        assert_eq!(path.commands.len(), 5);
    }

    #[test]
    fn round_rect_has_corner_curves() {
        let path = round_rect_path(Rect::from_xywh(0.0, 0.0, 20.0, 20.0), 4.0);
        let cubics = path
            .commands
            .iter()
            .filter(|c| matches!(c, oxideav_core::PathCommand::CubicCurveTo { .. }))
            .count();
        assert_eq!(cubics, 4);
        let _ = Point::ORIGIN;
    }
}
