//! The element IR: the lowered, paint-ready description of a UI subtree.
//!
//! Declarative [`View`](crate::View)s build a tree of [`Element`]s; the layout
//! and paint passes (see [`crate::render`]) consume it. Keeping a concrete IR
//! between widgets and rendering is what lets the (future) reactive runtime
//! diff one tree against the next.

use crate::runtime::{ActionId, Cx, DragId, FocusId, KeyInput};
use forma_geometry::Insets;
use forma_layout::Axis;
use forma_render::Color;

/// Alignment of children along an axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// Cross-axis only: stretch children to fill the cross extent.
    Stretch,
}

/// Fixed-size overrides for an element. `None` means "size to content / fill".
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeOverride {
    pub width: Option<f64>,
    pub height: Option<f64>,
}

/// Layout properties common to every element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutStyle {
    pub padding: Insets,
    pub size: SizeOverride,
    /// Main-axis grow weight when this element is a flex child. `0.0` = fixed.
    pub grow: f64,
}

/// The painted decoration of an element's box: fill, corner radius, border.
/// Applies to any element — a leaf bar, a button, or a container panel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxStyle {
    pub fill: Option<Color>,
    pub radius: f64,
    /// Border as `(color, width)`.
    pub border: Option<(Color, f64)>,
}

/// What an element arranges; all kinds share [`Element::decoration`].
#[derive(Clone, Debug, PartialEq)]
pub enum ElementKind {
    /// A leaf with no children.
    Leaf,
    /// A single line of text. Sizes to the shaped run (via the active font);
    /// painted with [`Scene::fill_text`](forma_render::Scene::fill_text).
    Text {
        text: String,
        size: f64,
        color: Color,
    },
    /// A linear container that lays children along `axis`.
    Stack {
        axis: Axis,
        gap: f64,
        main_align: Align,
        cross_align: Align,
        children: Vec<Element>,
    },
}

/// A node in the element tree: layout properties, decoration, an optional
/// interaction handle, and a kind.
///
/// `PartialEq` enables the runtime's no-op repaint skip: if a rebuilt tree
/// equals the previous one, the cached frame is reused. Handler ids compare by
/// value and are assigned deterministically per build, so an unchanged view
/// produces an equal tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub layout: LayoutStyle,
    pub decoration: BoxStyle,
    /// Handler this element routes pointer taps to, if any. Set via
    /// [`Element::on_tap`]; resolved against the [`Cx`] handler table.
    pub action: Option<ActionId>,
    /// Focus + keyboard handle, if this element is focusable. Set via
    /// [`Element::on_key`].
    pub focus: Option<FocusId>,
    /// Drag handle, if this element responds to pointer drags. Set via
    /// [`Element::on_drag`].
    pub drag: Option<DragId>,
    pub kind: ElementKind,
}

impl Element {
    /// A decorated leaf (background/button/divider).
    pub fn boxed(style: BoxStyle) -> Self {
        Self {
            layout: LayoutStyle::default(),
            decoration: style,
            action: None,
            focus: None,
            drag: None,
            kind: ElementKind::Leaf,
        }
    }

    /// A single line of text in `color` at `size` logical pixels.
    pub fn text(text: impl Into<String>, size: f64, color: Color) -> Self {
        Self {
            layout: LayoutStyle::default(),
            decoration: BoxStyle::default(),
            action: None,
            focus: None,
            drag: None,
            kind: ElementKind::Text {
                text: text.into(),
                size,
                color,
            },
        }
    }

    /// An undecorated container laying `children` along `axis`.
    pub fn stack(axis: Axis, children: Vec<Element>) -> Self {
        Self {
            layout: LayoutStyle::default(),
            decoration: BoxStyle::default(),
            action: None,
            focus: None,
            drag: None,
            kind: ElementKind::Stack {
                axis,
                gap: 0.0,
                main_align: Align::Start,
                cross_align: Align::Start,
                children,
            },
        }
    }

    /// Route pointer taps on this element to `handler`, which runs against the
    /// app state. Registers the handler in `cx` and stamps its [`ActionId`].
    ///
    /// ```
    /// # use forma_core::{Element, BoxStyle, runtime::Cx};
    /// # use forma_style::Theme;
    /// let theme = Theme::light();
    /// let mut cx = Cx::new(&theme);
    /// let button = Element::boxed(BoxStyle::default())
    ///     .width(80.0)
    ///     .height(32.0)
    ///     .on_tap(&mut cx, |count: &mut i32| *count += 1);
    /// assert!(button.action.is_some());
    /// ```
    pub fn on_tap<S>(mut self, cx: &mut Cx<'_, S>, handler: impl FnMut(&mut S) + 'static) -> Self {
        self.action = Some(cx.register(handler));
        self
    }

    /// Make this element focusable and route keyboard input to `handler`.
    /// The handler receives a [`KeyInput`] (committed text or an editing key)
    /// while this element holds focus. Registers in `cx` and stamps the
    /// resulting [`FocusId`].
    pub fn on_key<S>(
        mut self,
        cx: &mut Cx<'_, S>,
        handler: impl FnMut(&mut S, &KeyInput) + 'static,
    ) -> Self {
        self.focus = Some(cx.register_key(handler));
        self
    }

    /// Make this element respond to pointer drags. The `handler` receives the
    /// pointer's fractional x position (0..=1) across the element on press and
    /// while dragging. Registers in `cx` and stamps the resulting [`DragId`].
    pub fn on_drag<S>(
        mut self,
        cx: &mut Cx<'_, S>,
        handler: impl FnMut(&mut S, f64) + 'static,
    ) -> Self {
        self.drag = Some(cx.register_drag(handler));
        self
    }

    // --- decoration modifiers ---

    pub fn fill(mut self, color: Color) -> Self {
        self.decoration.fill = Some(color);
        self
    }

    pub fn radius(mut self, radius: f64) -> Self {
        self.decoration.radius = radius;
        self
    }

    pub fn border(mut self, color: Color, width: f64) -> Self {
        self.decoration.border = Some((color, width));
        self
    }

    // --- layout modifiers ---

    pub fn padding(mut self, insets: Insets) -> Self {
        self.layout.padding = insets;
        self
    }

    pub fn width(mut self, w: f64) -> Self {
        self.layout.size.width = Some(w);
        self
    }

    pub fn height(mut self, h: f64) -> Self {
        self.layout.size.height = Some(h);
        self
    }

    pub fn grow(mut self, grow: f64) -> Self {
        self.layout.grow = grow;
        self
    }

    pub fn gap(mut self, gap: f64) -> Self {
        if let ElementKind::Stack { gap: g, .. } = &mut self.kind {
            *g = gap;
        }
        self
    }

    pub fn align(mut self, main: Align, cross: Align) -> Self {
        if let ElementKind::Stack {
            main_align,
            cross_align,
            ..
        } = &mut self.kind
        {
            *main_align = main;
            *cross_align = cross;
        }
        self
    }
}
