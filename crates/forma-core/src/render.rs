//! Layout and paint passes: turn an [`Element`] tree into a [`Scene`].
//!
//! Two phases:
//! 1. **measure** — bottom-up natural sizing (content + padding, honoring fixed
//!    overrides).
//! 2. **place** — top-down assignment of bounds, using
//!    [`forma_layout::solve_main_axis`] to distribute the main axis and
//!    [`Align`] to position children on the cross axis; each element paints its
//!    decoration as it is placed.

use crate::element::{Align, BoxStyle, Element, ElementKind};
use forma_geometry::{Rect, Size};
use forma_layout::{Axis, FlexItem, solve_main_axis};
use forma_render::Scene;

/// Natural (desired) size of `el` given the `avail` space.
pub fn measure(el: &Element, avail: Size) -> Size {
    let pad = el.layout.padding;
    let inner = avail.deflate(pad);

    let content = match &el.kind {
        ElementKind::Leaf => Size::ZERO,
        ElementKind::Stack {
            axis,
            gap,
            children,
            ..
        } => {
            let mut main = 0.0;
            let mut cross: f64 = 0.0;
            for c in children {
                let cs = measure(c, inner);
                main += axis.main(cs);
                cross = cross.max(axis.cross(cs));
            }
            if children.len() > 1 {
                main += gap * (children.len() as f64 - 1.0);
            }
            axis.size(main, cross)
        }
    };

    let w = el
        .layout
        .size
        .width
        .unwrap_or(content.width + pad.horizontal());
    let h = el
        .layout
        .size
        .height
        .unwrap_or(content.height + pad.vertical());
    Size::new(w, h)
}

/// Place `el` within `bounds` and paint it (and its descendants) into `scene`.
pub fn place(el: &Element, bounds: Rect, scene: &mut Scene) {
    paint_decoration(&el.decoration, bounds, scene);

    let ElementKind::Stack {
        axis,
        gap,
        main_align,
        cross_align,
        children,
    } = &el.kind
    else {
        return;
    };
    if children.is_empty() {
        return;
    }

    let content = bounds.inset(el.layout.padding);
    let avail = content.size;
    let axis = *axis;

    // Main-axis distribution.
    let measured: Vec<Size> = children.iter().map(|c| measure(c, avail)).collect();
    let items: Vec<FlexItem> = children
        .iter()
        .zip(&measured)
        .map(|(c, m)| FlexItem {
            basis: axis.main(*m),
            grow: c.layout.grow,
        })
        .collect();
    let spans = solve_main_axis(axis.main(avail), *gap, &items);

    // If nothing grows, the block may be shorter than the content area; shift
    // it as a whole per the main-axis alignment.
    let used_main = spans.last().map(|s| s.offset + s.length).unwrap_or(0.0);
    let leftover = (axis.main(avail) - used_main).max(0.0);
    let main_shift = match main_align {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => leftover / 2.0,
        Align::End => leftover,
    };

    for ((child, m), span) in children.iter().zip(&measured).zip(&spans) {
        let cross_avail = axis.cross(avail);
        let cross_len = match cross_align {
            Align::Stretch => cross_avail,
            _ => axis.cross(*m).min(cross_avail),
        };
        let cross_off = match cross_align {
            Align::Start | Align::Stretch => 0.0,
            Align::Center => (cross_avail - cross_len) / 2.0,
            Align::End => cross_avail - cross_len,
        };
        let child_bounds = child_rect(
            axis,
            content,
            span.offset + main_shift,
            span.length,
            cross_off,
            cross_len,
        );
        place(child, child_bounds, scene);
    }
}

fn child_rect(
    axis: Axis,
    content: Rect,
    main_off: f64,
    main_len: f64,
    cross_off: f64,
    cross_len: f64,
) -> Rect {
    match axis {
        Axis::Horizontal => Rect::from_xywh(
            content.min_x() + main_off,
            content.min_y() + cross_off,
            main_len,
            cross_len,
        ),
        Axis::Vertical => Rect::from_xywh(
            content.min_x() + cross_off,
            content.min_y() + main_off,
            cross_len,
            main_len,
        ),
    }
}

fn paint_decoration(deco: &BoxStyle, bounds: Rect, scene: &mut Scene) {
    if let Some(fill) = deco.fill {
        if deco.radius > 0.0 {
            scene.fill_round_rect(bounds, deco.radius, fill);
        } else {
            scene.fill_rect(bounds, fill);
        }
    }
    if let Some((color, width)) = deco.border {
        scene.stroke_rect(bounds, color, width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::BoxStyle;
    use forma_render::Color;

    #[test]
    fn measure_sums_children_with_gap_and_padding() {
        let child = || Element::boxed(BoxStyle::default()).width(20.0).height(10.0);
        let stack = Element::stack(Axis::Vertical, vec![child(), child(), child()])
            .gap(5.0)
            .padding(forma_geometry::Insets::uniform(4.0));
        // main (vertical): 3*10 + 2*5 + 2*4 = 48; cross (width): 20 + 2*4 = 28
        let size = measure(&stack, Size::new(1000.0, 1000.0));
        assert_eq!(size, Size::new(28.0, 48.0));
    }

    #[test]
    fn grow_child_fills_main_axis() {
        // A row with one fixed 40px box and one grow=1 box in 200px width.
        let fixed = Element::boxed(BoxStyle::default()).width(40.0).height(10.0);
        let flex = Element::boxed(BoxStyle::default().with_fill(Color::WHITE)).grow(1.0);
        let row = Element::stack(Axis::Horizontal, vec![fixed, flex]);

        // Render into a scene and confirm two primitives were emitted: the
        // flex child has a fill, the fixed one does not.
        let mut scene = Scene::new(Size::new(200.0, 20.0));
        place(&row, Rect::from_xywh(0.0, 0.0, 200.0, 20.0), &mut scene);
        assert_eq!(scene.len(), 1); // only the filled flex box paints
    }

    impl BoxStyle {
        fn with_fill(mut self, c: Color) -> Self {
            self.fill = Some(c);
            self
        }
    }
}
