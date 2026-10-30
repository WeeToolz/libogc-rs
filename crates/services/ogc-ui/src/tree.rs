use alloc::{boxed::Box, format, string::String, vec::Vec};

use crate::{Canvas, CanvasError, Color, Rect, TextAlign, TextStyle, canvas::ClippedCanvas};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WidgetId(u32);

impl WidgetId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dimension {
    Pixels(u16),
    Fill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Size {
    pub width: Dimension,
    pub height: Dimension,
}

/// Slider bounds, step, and current value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SliderSpec {
    pub min: u16,
    pub max: u16,
    pub step: u16,
    pub value: u16,
}

impl Size {
    pub const fn fixed(width: u16, height: u16) -> Self {
        Self {
            width: Dimension::Pixels(width),
            height: Dimension::Pixels(height),
        }
    }

    pub const fn fill() -> Self {
        Self {
            width: Dimension::Fill,
            height: Dimension::Fill,
        }
    }

    pub const fn fill_width(height: u16) -> Self {
        Self {
            width: Dimension::Fill,
            height: Dimension::Pixels(height),
        }
    }

    pub const fn fill_height(width: u16) -> Self {
        Self {
            width: Dimension::Pixels(width),
            height: Dimension::Fill,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub axis: Axis,
    pub gap: u16,
    pub padding: u16,
}

impl Layout {
    pub const fn column(gap: u16, padding: u16) -> Self {
        Self {
            axis: Axis::Vertical,
            gap,
            padding,
        }
    }

    pub const fn row(gap: u16, padding: u16) -> Self {
        Self {
            axis: Axis::Horizontal,
            gap,
            padding,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Previous,
    Next,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiInput {
    /// One navigation press edge. Held-key repeat is controlled by the app.
    pub direction: Option<Direction>,
    /// True only on the frame that the accept button is pressed.
    pub activate: bool,
    /// True only on the frame that the cancel button is pressed.
    pub cancel: bool,
    /// Current pointer position in screen coordinates.
    pub pointer: Option<(u16, u16)>,
    /// True only on the frame that the pointer button is pressed.
    pub pointer_activate: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiEvent {
    Activated(WidgetId),
    Cancelled(WidgetId),
    CustomUpdated(WidgetId),
    Toggled { widget: WidgetId, checked: bool },
    ValueChanged { widget: WidgetId, value: u16 },
    SelectionChanged { widget: WidgetId, selected: usize },
    RadioSelected(WidgetId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiError {
    InvalidViewport,
    AllocationFailed,
    TooManyWidgets,
    UnknownWidget,
    ParentIsNotContainer,
    NotTextWidget,
    InvalidTextScale,
    InvalidRange,
    EmptyOptions,
    NotControl,
    FocusScopeIsNotContainer,
}

#[derive(Clone, Copy)]
struct WidgetStyle {
    background: Color,
    focused_background: Color,
    foreground: Color,
    text_scale: u8,
}

impl Default for WidgetStyle {
    fn default() -> Self {
        Self {
            background: Color::rgb(28, 43, 59),
            focused_background: Color::rgb(44, 126, 176),
            foreground: Color::rgb(255, 255, 255),
            text_scale: 2,
        }
    }
}

pub trait CustomWidget {
    /// Draw changing content through the same canvas used by retained widgets.
    fn draw(&mut self, canvas: &mut dyn Canvas, bounds: Rect) -> Result<(), CanvasError>;

    /// Handle input when this widget has focus. Return true when visible state changed.
    fn update(&mut self, _input: UiInput) -> bool {
        false
    }

    fn focusable(&self) -> bool {
        false
    }
}

enum Kind {
    Root,
    Container(Layout),
    Button(String),
    Label(String),
    Checkbox {
        label: String,
        checked: bool,
    },
    Radio {
        label: String,
        group: u16,
        selected: bool,
    },
    Slider {
        label: String,
        min: u16,
        max: u16,
        step: u16,
        value: u16,
    },
    List {
        items: Vec<String>,
        selected: usize,
    },
    Progress {
        label: String,
        value: u8,
    },
    Custom(Box<dyn CustomWidget>),
}

struct Node {
    kind: Kind,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    size: Size,
    bounds: Rect,
    clip: Rect,
    style: WidgetStyle,
    enabled: bool,
    visible: bool,
}

impl Node {
    fn new(kind: Kind, parent: Option<WidgetId>, size: Size) -> Self {
        Self {
            kind,
            parent,
            children: Vec::new(),
            size,
            bounds: Rect::default(),
            clip: Rect::default(),
            style: WidgetStyle::default(),
            enabled: true,
            visible: true,
        }
    }

    fn focusable(&self) -> bool {
        self.enabled
            && match &self.kind {
                Kind::Button(_) => true,
                Kind::Checkbox { .. }
                | Kind::Radio { .. }
                | Kind::Slider { .. }
                | Kind::List { .. } => true,
                Kind::Custom(widget) => widget.focusable(),
                Kind::Root | Kind::Container(_) | Kind::Label(_) | Kind::Progress { .. } => false,
            }
    }
}

pub struct Ui {
    nodes: Vec<Node>,
    focused: Option<WidgetId>,
    viewport: Rect,
    previous_pointer: Option<(u16, u16)>,
    focus_scope: WidgetId,
}

impl Ui {
    pub fn new(width: u16, height: u16) -> Result<Self, UiError> {
        if width == 0 || height == 0 {
            return Err(UiError::InvalidViewport);
        }
        let viewport = Rect::new(0, 0, width, height);
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(1)
            .map_err(|_| UiError::AllocationFailed)?;
        let mut root = Node::new(Kind::Root, None, Size::fill());
        root.bounds = viewport;
        root.clip = viewport;
        nodes.push(root);
        Ok(Self {
            nodes,
            focused: None,
            viewport,
            previous_pointer: None,
            focus_scope: WidgetId(0),
        })
    }

    pub const fn root(&self) -> WidgetId {
        WidgetId(0)
    }

    pub const fn viewport(&self) -> Rect {
        self.viewport
    }

    pub const fn focused(&self) -> Option<WidgetId> {
        self.focused
    }

    /// Restrict keyboard focus and pointer hit testing to a container subtree.
    /// Use the root container to restore the full UI after closing a modal.
    pub fn set_focus_scope(&mut self, scope: WidgetId) -> Result<(), UiError> {
        let node = self
            .nodes
            .get(scope.index())
            .ok_or(UiError::UnknownWidget)?;
        if !matches!(node.kind, Kind::Root | Kind::Container(_)) {
            return Err(UiError::FocusScopeIsNotContainer);
        }
        self.focus_scope = scope;
        if self
            .focused
            .is_some_and(|focused| !self.in_focus_scope(focused.index()))
        {
            self.focused = None;
        }
        Ok(())
    }

    pub const fn focus_scope(&self) -> WidgetId {
        self.focus_scope
    }

    /// Return the same focus-scope-aware pointer target used by `update`.
    /// This lays out the tree but does not change focus or widget state.
    pub fn pointer_target(&mut self, x: u16, y: u16) -> Option<WidgetId> {
        self.layout_children(WidgetId(0));
        self.hit_test(x, y)
    }

    pub fn bounds(&mut self, id: WidgetId) -> Result<Rect, UiError> {
        self.layout_children(WidgetId(0));
        self.nodes
            .get(id.index())
            .map(|node| node.bounds)
            .ok_or(UiError::UnknownWidget)
    }

    pub fn add_container(
        &mut self,
        parent: WidgetId,
        layout: Layout,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        self.add_node(parent, Kind::Container(layout), size)
    }

    pub fn add_button(
        &mut self,
        parent: WidgetId,
        label: &str,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        let label = copy_text(label)?;
        self.add_node(parent, Kind::Button(label), size)
    }

    pub fn add_label(
        &mut self,
        parent: WidgetId,
        text: &str,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        let text = copy_text(text)?;
        self.add_node(parent, Kind::Label(text), size)
    }

    /// Add a check box that toggles when activated.
    pub fn add_checkbox(
        &mut self,
        parent: WidgetId,
        label: &str,
        checked: bool,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        let label = copy_text(label)?;
        self.add_node(parent, Kind::Checkbox { label, checked }, size)
    }

    /// Add a radio button. Buttons with the same parent and group are exclusive.
    pub fn add_radio(
        &mut self,
        parent: WidgetId,
        group: u16,
        label: &str,
        selected: bool,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        let label = copy_text(label)?;
        let id = self.add_node(
            parent,
            Kind::Radio {
                label,
                group,
                selected: false,
            },
            size,
        )?;
        if selected {
            self.select_radio(id);
        }
        Ok(id)
    }

    /// Add an integer slider. The range is inclusive and step must be nonzero.
    pub fn add_slider(
        &mut self,
        parent: WidgetId,
        label: &str,
        spec: SliderSpec,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        if spec.min > spec.max || spec.step == 0 || spec.value < spec.min || spec.value > spec.max {
            return Err(UiError::InvalidRange);
        }
        let label = copy_text(label)?;
        self.add_node(
            parent,
            Kind::Slider {
                label,
                min: spec.min,
                max: spec.max,
                step: spec.step,
                value: spec.value,
            },
            size,
        )
    }

    /// Add a vertically navigable list. At least one option is required.
    pub fn add_list(
        &mut self,
        parent: WidgetId,
        items: &[&str],
        selected: usize,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        if items.is_empty() {
            return Err(UiError::EmptyOptions);
        }
        if selected >= items.len() {
            return Err(UiError::InvalidRange);
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(items.len())
            .map_err(|_| UiError::AllocationFailed)?;
        for item in items {
            owned.push(copy_text(item)?);
        }
        self.add_node(
            parent,
            Kind::List {
                items: owned,
                selected,
            },
            size,
        )
    }

    /// Add a non-focusable progress bar. Values are percentages from 0 to 100.
    pub fn add_progress(
        &mut self,
        parent: WidgetId,
        label: &str,
        value: u8,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        if value > 100 {
            return Err(UiError::InvalidRange);
        }
        let label = copy_text(label)?;
        self.add_node(parent, Kind::Progress { label, value }, size)
    }

    pub fn checked(&self, id: WidgetId) -> Result<bool, UiError> {
        match &self
            .nodes
            .get(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind
        {
            Kind::Checkbox { checked, .. }
            | Kind::Radio {
                selected: checked, ..
            } => Ok(*checked),
            _ => Err(UiError::NotControl),
        }
    }

    pub fn set_checked(&mut self, id: WidgetId, checked: bool) -> Result<(), UiError> {
        let kind = &self
            .nodes
            .get(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind;
        if matches!(kind, Kind::Radio { .. }) && checked {
            self.select_radio(id);
            return Ok(());
        }
        match &mut self.nodes[id.index()].kind {
            Kind::Checkbox {
                checked: current, ..
            } => *current = checked,
            Kind::Radio { selected, .. } => *selected = false,
            _ => return Err(UiError::NotControl),
        }
        Ok(())
    }

    pub fn value(&self, id: WidgetId) -> Result<u16, UiError> {
        match &self
            .nodes
            .get(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind
        {
            Kind::Slider { value, .. } => Ok(*value),
            _ => Err(UiError::NotControl),
        }
    }

    pub fn set_value(&mut self, id: WidgetId, value: u16) -> Result<(), UiError> {
        match &mut self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind
        {
            Kind::Slider {
                min,
                max,
                value: current,
                ..
            } if value >= *min && value <= *max => {
                *current = value;
            }
            Kind::Slider { .. } => return Err(UiError::InvalidRange),
            _ => return Err(UiError::NotControl),
        }
        Ok(())
    }

    pub fn selected(&self, id: WidgetId) -> Result<usize, UiError> {
        match &self
            .nodes
            .get(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind
        {
            Kind::List { selected, .. } => Ok(*selected),
            _ => Err(UiError::NotControl),
        }
    }

    pub fn set_selected(&mut self, id: WidgetId, selected: usize) -> Result<(), UiError> {
        match &mut self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind
        {
            Kind::List {
                items,
                selected: current,
            } if selected < items.len() => *current = selected,
            Kind::List { .. } => return Err(UiError::InvalidRange),
            _ => return Err(UiError::NotControl),
        }
        Ok(())
    }

    pub fn set_progress(&mut self, id: WidgetId, value: u8) -> Result<(), UiError> {
        if value > 100 {
            return Err(UiError::InvalidRange);
        }
        match &mut self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?
            .kind
        {
            Kind::Progress { value: current, .. } => *current = value,
            _ => return Err(UiError::NotControl),
        }
        Ok(())
    }

    pub fn add_custom(
        &mut self,
        parent: WidgetId,
        widget: Box<dyn CustomWidget>,
        size: Size,
    ) -> Result<WidgetId, UiError> {
        self.add_node(parent, Kind::Custom(widget), size)
    }

    pub fn set_text(&mut self, id: WidgetId, text: &str) -> Result<(), UiError> {
        let text = copy_text(text)?;
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?;
        match &mut node.kind {
            Kind::Button(current) | Kind::Label(current) => *current = text,
            Kind::Checkbox { label: current, .. }
            | Kind::Radio { label: current, .. }
            | Kind::Slider { label: current, .. }
            | Kind::Progress { label: current, .. } => *current = text,
            _ => return Err(UiError::NotTextWidget),
        }
        Ok(())
    }

    pub fn set_enabled(&mut self, id: WidgetId, enabled: bool) -> Result<(), UiError> {
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?;
        node.enabled = enabled;
        if !enabled && self.focused == Some(id) {
            self.focused = self.next_focus(id.index(), Direction::Next);
        }
        Ok(())
    }

    pub fn set_visible(&mut self, id: WidgetId, visible: bool) -> Result<(), UiError> {
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?;
        node.visible = visible;
        if !visible && self.focused == Some(id) {
            self.focused = self.next_focus(id.index(), Direction::Next);
        }
        Ok(())
    }

    pub fn set_colors(
        &mut self,
        id: WidgetId,
        background: Color,
        focused_background: Color,
        foreground: Color,
    ) -> Result<(), UiError> {
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?;
        node.style.background = background;
        node.style.focused_background = focused_background;
        node.style.foreground = foreground;
        Ok(())
    }

    /// Set the pixel scale for a text-bearing built-in control.
    pub fn set_text_scale(&mut self, id: WidgetId, scale: u8) -> Result<(), UiError> {
        if scale == 0 {
            return Err(UiError::InvalidTextScale);
        }
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(UiError::UnknownWidget)?;
        if !matches!(
            &node.kind,
            Kind::Button(_)
                | Kind::Label(_)
                | Kind::Checkbox { .. }
                | Kind::Radio { .. }
                | Kind::Slider { .. }
                | Kind::List { .. }
                | Kind::Progress { .. }
        ) {
            return Err(UiError::NotTextWidget);
        }
        node.style.text_scale = scale;
        Ok(())
    }

    /// Apply one input snapshot and return one user-visible action, if any.
    pub fn update(&mut self, input: UiInput) -> Option<UiEvent> {
        self.layout_children(WidgetId(0));
        self.ensure_focus();
        let pointer_moved = self.previous_pointer != input.pointer;
        self.previous_pointer = input.pointer;
        if let Some((x, y)) = input.pointer
            && let Some(id) = self.hit_test(x, y)
        {
            if pointer_moved || input.pointer_activate {
                self.focused = Some(id);
            }
            if input.pointer_activate {
                if let Some(event) = self.pointer_activate(id, x, y) {
                    return Some(event);
                }
                return self.activate_widget(id);
            }
        }
        // The same physical press can set both activation flags. A click with
        // no enabled target is still consumed and cannot activate focus.
        if input.pointer_activate {
            return None;
        }
        if let Some(direction) = input.direction {
            let current = self.focused?;
            let handled = match &mut self.nodes[current.index()].kind {
                Kind::Slider {
                    min,
                    max,
                    step,
                    value,
                    ..
                } if matches!(direction, Direction::Left | Direction::Right) => {
                    let previous = *value;
                    *value = match direction {
                        Direction::Left => value.saturating_sub(*step).max(*min),
                        Direction::Right => value.saturating_add(*step).min(*max),
                        _ => previous,
                    };
                    if *value != previous {
                        return Some(UiEvent::ValueChanged {
                            widget: current,
                            value: *value,
                        });
                    }
                    false
                }
                Kind::List { items, selected }
                    if matches!(direction, Direction::Up | Direction::Down) =>
                {
                    let previous = *selected;
                    *selected = match direction {
                        Direction::Up => selected.saturating_sub(1),
                        Direction::Down => (*selected + 1).min(items.len() - 1),
                        _ => previous,
                    };
                    if *selected != previous {
                        return Some(UiEvent::SelectionChanged {
                            widget: current,
                            selected: *selected,
                        });
                    }
                    false
                }
                _ => false,
            };
            if !handled {
                self.focused = self.next_focus(current.index(), direction);
            }
        }
        let focused = self.focused?;
        if input.cancel {
            return Some(UiEvent::Cancelled(focused));
        }
        if input.activate {
            if let Some(event) = self.activate_widget(focused) {
                return Some(event);
            }
            if let Kind::Custom(widget) = &mut self.nodes[focused.index()].kind
                && widget.update(input)
            {
                return Some(UiEvent::CustomUpdated(focused));
            }
            return matches!(self.nodes[focused.index()].kind, Kind::Button(_))
                .then_some(UiEvent::Activated(focused));
        }
        if let Kind::Custom(widget) = &mut self.nodes[focused.index()].kind
            && widget.update(input)
        {
            return Some(UiEvent::CustomUpdated(focused));
        }
        None
    }

    fn activate_widget(&mut self, id: WidgetId) -> Option<UiEvent> {
        match &mut self.nodes[id.index()].kind {
            Kind::Checkbox { checked, .. } => {
                *checked = !*checked;
                Some(UiEvent::Toggled {
                    widget: id,
                    checked: *checked,
                })
            }
            Kind::Radio { selected, .. } => {
                if !*selected {
                    self.select_radio(id);
                }
                Some(UiEvent::RadioSelected(id))
            }
            Kind::List { selected, .. } => Some(UiEvent::SelectionChanged {
                widget: id,
                selected: *selected,
            }),
            Kind::Button(_) => Some(UiEvent::Activated(id)),
            _ => None,
        }
    }

    fn select_radio(&mut self, id: WidgetId) {
        let (parent, group) = match self.nodes[id.index()].kind {
            Kind::Radio { group, .. } => (self.nodes[id.index()].parent, group),
            _ => return,
        };
        let Some(parent) = parent else { return };
        let parent_index = parent.index();
        let child_count = self.nodes[parent_index].children.len();
        for index in 0..child_count {
            let sibling = self.nodes[parent_index].children[index];
            if let Kind::Radio {
                group: sibling_group,
                selected,
                ..
            } = &mut self.nodes[sibling.index()].kind
                && *sibling_group == group
            {
                *selected = sibling == id;
            }
        }
    }

    fn pointer_activate(&mut self, id: WidgetId, x: u16, y: u16) -> Option<UiEvent> {
        let bounds = self.nodes[id.index()].bounds;
        let text_scale = self.nodes[id.index()].style.text_scale;
        match &mut self.nodes[id.index()].kind {
            Kind::Slider {
                min, max, value, ..
            } => {
                if bounds.width == 0 {
                    return None;
                }
                let denominator = u32::from(bounds.width.saturating_sub(1)).max(1);
                let offset = u32::from(x.saturating_sub(bounds.x)).min(denominator);
                let range = u32::from(*max) - u32::from(*min);
                let value_at_pointer = u32::from(*min) + range.saturating_mul(offset) / denominator;
                let value_at_pointer = value_at_pointer.min(u32::from(u16::MAX)) as u16;
                if *value != value_at_pointer {
                    *value = value_at_pointer;
                    return Some(UiEvent::ValueChanged {
                        widget: id,
                        value: *value,
                    });
                }
            }
            Kind::List { items, selected } => {
                let row_height = u16::from(text_scale)
                    .saturating_mul(8)
                    .saturating_add(4)
                    .max(1);
                let visible_rows = (bounds.height / row_height).max(1) as usize;
                let first = selected.saturating_add(1).saturating_sub(visible_rows);
                let relative_row = usize::from(y.saturating_sub(bounds.y) / row_height);
                let choice = first.saturating_add(relative_row);
                if choice < items.len() && choice != *selected {
                    *selected = choice;
                    return Some(UiEvent::SelectionChanged {
                        widget: id,
                        selected: *selected,
                    });
                }
            }
            _ => {}
        }
        None
    }

    pub fn draw(&mut self, canvas: &mut dyn Canvas) -> Result<(), CanvasError> {
        self.layout_children(WidgetId(0));
        self.draw_node(WidgetId(0), canvas)
    }

    fn add_node(&mut self, parent: WidgetId, kind: Kind, size: Size) -> Result<WidgetId, UiError> {
        let parent_index = parent.index();
        let parent_node = self.nodes.get(parent_index).ok_or(UiError::UnknownWidget)?;
        if !matches!(parent_node.kind, Kind::Root | Kind::Container(_)) {
            return Err(UiError::ParentIsNotContainer);
        }
        let raw_id = u32::try_from(self.nodes.len()).map_err(|_| UiError::TooManyWidgets)?;
        self.nodes
            .try_reserve(1)
            .map_err(|_| UiError::AllocationFailed)?;
        self.nodes[parent_index]
            .children
            .try_reserve(1)
            .map_err(|_| UiError::AllocationFailed)?;
        let id = WidgetId(raw_id);
        self.nodes.push(Node::new(kind, Some(parent), size));
        self.nodes[parent_index].children.push(id);
        Ok(id)
    }

    fn ensure_focus(&mut self) {
        if self.focused.is_some_and(|id| {
            self.in_focus_scope(id.index())
                && self
                    .nodes
                    .get(id.index())
                    .is_some_and(|node| node.visible && node.focusable())
                && self.ancestors_visible(id.index())
        }) {
            return;
        }
        self.focused = self.next_focus(self.focus_scope.index(), Direction::Next);
    }

    fn next_focus(&self, from: usize, direction: Direction) -> Option<WidgetId> {
        let count = self.nodes.len();
        if count <= 1 {
            return None;
        }
        if matches!(
            direction,
            Direction::Up | Direction::Down | Direction::Left | Direction::Right
        ) {
            return self.spatial_focus(from, direction);
        }
        for distance in 1..count {
            let index = match direction {
                Direction::Next => (from + distance) % count,
                Direction::Previous => (from + count - (distance % count)) % count,
                Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
                    unreachable!("spatial directions return before sequential navigation")
                }
            };
            let node = &self.nodes[index];
            if self.in_focus_scope(index)
                && node.visible
                && node.focusable()
                && self.ancestors_visible(index)
            {
                return Some(WidgetId(index as u32));
            }
        }
        None
    }

    fn spatial_focus(&self, from: usize, direction: Direction) -> Option<WidgetId> {
        let origin = self.nodes.get(from)?.bounds;
        let origin_x = u32::from(origin.x) + u32::from(origin.width) / 2;
        let origin_y = u32::from(origin.y) + u32::from(origin.height) / 2;
        let mut best: Option<(u64, usize)> = None;
        let mut wrapped: Option<(u64, usize)> = None;

        for (index, node) in self.nodes.iter().enumerate().skip(1) {
            if index == from
                || !self.in_focus_scope(index)
                || !node.visible
                || !node.focusable()
                || !self.ancestors_visible(index)
            {
                continue;
            }
            let center_x = u32::from(node.bounds.x) + u32::from(node.bounds.width) / 2;
            let center_y = u32::from(node.bounds.y) + u32::from(node.bounds.height) / 2;
            let (primary, secondary, in_direction, wrap_distance) = match direction {
                Direction::Up => (
                    origin_y.saturating_sub(center_y),
                    origin_x.abs_diff(center_x),
                    center_y < origin_y,
                    u32::MAX - center_y,
                ),
                Direction::Down => (
                    center_y.saturating_sub(origin_y),
                    origin_x.abs_diff(center_x),
                    center_y > origin_y,
                    center_y,
                ),
                Direction::Left => (
                    origin_x.saturating_sub(center_x),
                    origin_y.abs_diff(center_y),
                    center_x < origin_x,
                    u32::MAX - center_x,
                ),
                Direction::Right => (
                    center_x.saturating_sub(origin_x),
                    origin_y.abs_diff(center_y),
                    center_x > origin_x,
                    center_x,
                ),
                Direction::Previous | Direction::Next => return None,
            };
            let spatial_score =
                u64::from(primary) * (u64::from(u16::MAX) + 1) + u64::from(secondary);
            if in_direction && best.is_none_or(|(score, _)| spatial_score < score) {
                best = Some((spatial_score, index));
            }
            let wrap_score =
                u64::from(wrap_distance) * (u64::from(u16::MAX) + 1) + u64::from(secondary);
            if wrapped.is_none_or(|(score, _)| wrap_score < score) {
                wrapped = Some((wrap_score, index));
            }
        }

        best.or(wrapped).map(|(_, index)| WidgetId(index as u32))
    }

    fn hit_test(&self, x: u16, y: u16) -> Option<WidgetId> {
        self.nodes
            .iter()
            .enumerate()
            .rev()
            .find(|(index, node)| {
                node.visible
                    && node.focusable()
                    && self.in_focus_scope(*index)
                    && self.ancestors_visible(*index)
                    && node.clip.contains(x, y)
                    && node.bounds.contains(x, y)
            })
            .map(|(index, _)| WidgetId(index as u32))
    }

    fn ancestors_visible(&self, mut index: usize) -> bool {
        while let Some(parent) = self.nodes[index].parent {
            index = parent.index();
            if !self.nodes[index].visible {
                return false;
            }
        }
        true
    }

    fn in_focus_scope(&self, mut index: usize) -> bool {
        loop {
            if index == self.focus_scope.index() {
                return true;
            }
            let Some(parent) = self.nodes[index].parent else {
                return false;
            };
            index = parent.index();
        }
    }

    fn layout_children(&mut self, parent: WidgetId) {
        let parent_index = parent.index();
        let parent_bounds = self.nodes[parent_index].bounds;
        let parent_clip = self.nodes[parent_index]
            .clip
            .intersection(parent_bounds)
            .unwrap_or_default();
        let child_count = self.nodes[parent_index].children.len();
        let layout = match self.nodes[parent_index].kind {
            Kind::Root => Some(Layout::column(0, 0)),
            Kind::Container(layout) => Some(layout),
            _ => None,
        };
        let Some(layout) = layout else { return };

        let pad = u32::from(layout.padding);
        let (origin, cross_origin, total, cross_total) = match layout.axis {
            Axis::Vertical => (
                u32::from(parent_bounds.y) + pad,
                u32::from(parent_bounds.x) + pad,
                u32::from(parent_bounds.height).saturating_sub(pad * 2),
                u32::from(parent_bounds.width).saturating_sub(pad * 2),
            ),
            Axis::Horizontal => (
                u32::from(parent_bounds.x) + pad,
                u32::from(parent_bounds.y) + pad,
                u32::from(parent_bounds.width).saturating_sub(pad * 2),
                u32::from(parent_bounds.height).saturating_sub(pad * 2),
            ),
        };
        let gaps = u32::from(layout.gap).saturating_mul(child_count.saturating_sub(1) as u32);
        let available = total.saturating_sub(gaps);
        let mut fixed = 0u32;
        let mut fill_count = 0u32;
        for index in 0..child_count {
            let child = self.nodes[parent_index].children[index].index();
            let dimension = match layout.axis {
                Axis::Vertical => self.nodes[child].size.height,
                Axis::Horizontal => self.nodes[child].size.width,
            };
            match dimension {
                Dimension::Pixels(value) => fixed = fixed.saturating_add(u32::from(value)),
                Dimension::Fill => fill_count += 1,
            }
        }
        let fill_size = available
            .saturating_sub(fixed)
            .checked_div(fill_count)
            .unwrap_or(0);
        let mut cursor = origin;
        for index in 0..child_count {
            let child_id = self.nodes[parent_index].children[index];
            let child_index = child_id.index();
            let child_size = self.nodes[child_index].size;
            let (main_dim, cross_dim) = match layout.axis {
                Axis::Vertical => (child_size.height, child_size.width),
                Axis::Horizontal => (child_size.width, child_size.height),
            };
            let main = match main_dim {
                Dimension::Pixels(value) => u32::from(value),
                Dimension::Fill => fill_size,
            };
            let cross = match cross_dim {
                Dimension::Pixels(value) => u32::from(value).min(cross_total),
                Dimension::Fill => cross_total,
            };
            let (x, y, width, height) = match layout.axis {
                Axis::Vertical => (cross_origin, cursor, cross, main),
                Axis::Horizontal => (cursor, cross_origin, main, cross),
            };
            let node = &mut self.nodes[child_index];
            node.bounds = Rect::new(
                x.min(u32::from(u16::MAX)) as u16,
                y.min(u32::from(u16::MAX)) as u16,
                width.min(u32::from(u16::MAX)) as u16,
                height.min(u32::from(u16::MAX)) as u16,
            );
            node.clip = parent_clip;
            cursor = cursor
                .saturating_add(main)
                .saturating_add(u32::from(layout.gap));
        }
        for index in 0..child_count {
            let child = self.nodes[parent_index].children[index];
            self.layout_children(child);
        }
    }

    fn draw_node(&mut self, id: WidgetId, canvas: &mut dyn Canvas) -> Result<(), CanvasError> {
        let index = id.index();
        if !self.nodes[index].visible || !self.ancestors_visible(index) {
            return Ok(());
        }
        let bounds = self.nodes[index].bounds;
        let clip = self.nodes[index]
            .clip
            .intersection(self.nodes[index].bounds)
            .unwrap_or_default();
        let focused = self.focused == Some(id);
        {
            let node = &mut self.nodes[index];
            let mut clipped = ClippedCanvas::new(canvas, clip);
            match &mut node.kind {
                Kind::Root | Kind::Container(_) => {}
                Kind::Button(label) => {
                    let background = if focused {
                        node.style.focused_background
                    } else {
                        node.style.background
                    };
                    clipped.fill_rect(bounds, background, clip)?;
                    clipped.draw_text(
                        label,
                        bounds,
                        TextStyle {
                            color: node.style.foreground,
                            scale: node.style.text_scale,
                            align: TextAlign::Center,
                        },
                        clip,
                    )?;
                }
                Kind::Label(text) => clipped.draw_text(
                    text,
                    bounds,
                    TextStyle {
                        color: node.style.foreground,
                        scale: node.style.text_scale,
                        align: TextAlign::Start,
                    },
                    clip,
                )?,
                Kind::Checkbox { label, checked } => {
                    draw_choice(
                        &mut clipped,
                        bounds,
                        label,
                        ChoiceState {
                            selected: *checked,
                            radio: false,
                            focused,
                        },
                        node.style,
                        clip,
                    )?;
                }
                Kind::Radio {
                    label, selected, ..
                } => {
                    draw_choice(
                        &mut clipped,
                        bounds,
                        label,
                        ChoiceState {
                            selected: *selected,
                            radio: true,
                            focused,
                        },
                        node.style,
                        clip,
                    )?;
                }
                Kind::Slider {
                    label,
                    min,
                    max,
                    value,
                    ..
                } => {
                    let background = if focused {
                        node.style.focused_background
                    } else {
                        node.style.background
                    };
                    clipped.fill_rect(bounds, background, clip)?;
                    let text = format!("{label}: {value}");
                    let track_height = 6u16;
                    let track = Rect::new(
                        bounds.x.saturating_add(6),
                        bounds.y.saturating_add(
                            bounds.height.saturating_sub(track_height).saturating_sub(5),
                        ),
                        bounds.width.saturating_sub(12),
                        track_height,
                    );
                    clipped.fill_rect(track, node.style.foreground, clip)?;
                    let range = u32::from(*max) - u32::from(*min);
                    let filled = (u32::from(track.width) * (u32::from(*value) - u32::from(*min)))
                        .checked_div(range)
                        .unwrap_or(u32::from(track.width));
                    clipped.fill_rect(
                        Rect::new(track.x, track.y, filled as u16, track.height),
                        Color::rgb(45, 168, 220),
                        clip,
                    )?;
                    clipped.draw_text(
                        &text,
                        Rect::new(
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            bounds.height.saturating_sub(track_height + 7),
                        ),
                        TextStyle {
                            color: node.style.foreground,
                            scale: node.style.text_scale,
                            align: TextAlign::Center,
                        },
                        clip,
                    )?;
                }
                Kind::List { items, selected } => {
                    let row_height = u16::from(node.style.text_scale)
                        .saturating_mul(8)
                        .saturating_add(4)
                        .max(1);
                    let visible_rows = (bounds.height / row_height).max(1) as usize;
                    let first = selected.saturating_add(1).saturating_sub(visible_rows);
                    for (index, item) in items
                        .iter()
                        .enumerate()
                        .take(items.len().min(first.saturating_add(visible_rows)))
                        .skip(first)
                    {
                        let row = Rect::new(
                            bounds.x,
                            bounds.y.saturating_add((index - first) as u16 * row_height),
                            bounds.width,
                            row_height,
                        );
                        let background = if focused && index == *selected {
                            node.style.focused_background
                        } else {
                            node.style.background
                        };
                        clipped.fill_rect(row, background, clip)?;
                        clipped.draw_text(
                            item,
                            row,
                            TextStyle {
                                color: node.style.foreground,
                                scale: node.style.text_scale,
                                align: TextAlign::Start,
                            },
                            clip,
                        )?;
                    }
                }
                Kind::Progress { label, value } => {
                    let text = format!("{label}: {value}%");
                    clipped.draw_text(
                        &text,
                        Rect::new(
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            bounds.height.saturating_sub(10),
                        ),
                        TextStyle {
                            color: node.style.foreground,
                            scale: node.style.text_scale,
                            align: TextAlign::Start,
                        },
                        clip,
                    )?;
                    let track = Rect::new(
                        bounds.x,
                        bounds.y.saturating_add(bounds.height.saturating_sub(7)),
                        bounds.width,
                        6,
                    );
                    clipped.fill_rect(track, node.style.background, clip)?;
                    let fill = u32::from(track.width) * u32::from(*value) / 100;
                    clipped.fill_rect(
                        Rect::new(track.x, track.y, fill as u16, track.height),
                        node.style.focused_background,
                        clip,
                    )?;
                }
                Kind::Custom(widget) => widget.draw(&mut clipped, bounds)?,
            }
        }
        let child_count = self.nodes[index].children.len();
        for child_index in 0..child_count {
            let child = self.nodes[index].children[child_index];
            self.draw_node(child, canvas)?;
        }
        Ok(())
    }
}

fn copy_text(text: &str) -> Result<String, UiError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| UiError::AllocationFailed)?;
    owned.push_str(text);
    Ok(owned)
}

struct ChoiceState {
    selected: bool,
    radio: bool,
    focused: bool,
}

fn draw_choice(
    canvas: &mut dyn Canvas,
    bounds: Rect,
    label: &str,
    state: ChoiceState,
    style: WidgetStyle,
    clip: Rect,
) -> Result<(), CanvasError> {
    let background = if state.focused {
        style.focused_background
    } else {
        style.background
    };
    canvas.fill_rect(bounds, background, clip)?;

    let scale = style.text_scale;
    let marker_width = if state.radio {
        u16::from(scale).saturating_mul(18)
    } else {
        u16::from(scale).saturating_mul(5).max(10)
    };
    let marker = Rect::new(
        bounds.x.saturating_add(5),
        bounds.y,
        marker_width,
        bounds.height,
    );
    if state.radio {
        canvas.draw_text(
            if state.selected { "(O)" } else { "( )" },
            marker,
            TextStyle {
                color: style.foreground,
                scale,
                align: TextAlign::Center,
            },
            clip,
        )?;
    } else {
        let box_rect = Rect::new(
            marker.x,
            bounds
                .y
                .saturating_add(bounds.height.saturating_sub(marker_width) / 2),
            marker_width,
            marker_width,
        );
        canvas.fill_rect(box_rect, style.foreground, clip)?;
        if state.selected {
            let inset = marker_width / 4;
            canvas.fill_rect(
                Rect::new(
                    box_rect.x.saturating_add(inset),
                    box_rect.y.saturating_add(inset),
                    marker_width.saturating_sub(inset.saturating_mul(2)),
                    marker_width.saturating_sub(inset.saturating_mul(2)),
                ),
                Color::rgb(35, 135, 190),
                clip,
            )?;
        }
    }

    canvas.draw_text(
        label,
        Rect::new(
            marker.x.saturating_add(marker_width).saturating_add(8),
            bounds.y,
            bounds.width.saturating_sub(marker_width.saturating_add(18)),
            bounds.height,
        ),
        TextStyle {
            color: style.foreground,
            scale,
            align: TextAlign::Start,
        },
        clip,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct CaptureCanvas {
        rectangles: Vec<Rect>,
        text: Vec<String>,
        fill_colors: Vec<Color>,
        text_colors: Vec<Color>,
    }

    struct PassiveCustom;

    impl CustomWidget for PassiveCustom {
        fn draw(&mut self, canvas: &mut dyn Canvas, bounds: Rect) -> Result<(), CanvasError> {
            canvas.fill_rect(bounds, Color::rgb(10, 20, 30), bounds)
        }
    }

    struct ActiveCustom;

    impl CustomWidget for ActiveCustom {
        fn draw(&mut self, _canvas: &mut dyn Canvas, _bounds: Rect) -> Result<(), CanvasError> {
            Ok(())
        }

        fn update(&mut self, input: UiInput) -> bool {
            input.activate
        }

        fn focusable(&self) -> bool {
            true
        }
    }

    impl Canvas for CaptureCanvas {
        fn fill_rect(&mut self, rect: Rect, color: Color, _clip: Rect) -> Result<(), CanvasError> {
            self.rectangles.push(rect);
            self.fill_colors.push(color);
            Ok(())
        }

        fn draw_text(
            &mut self,
            text: &str,
            _rect: Rect,
            style: TextStyle,
            _clip: Rect,
        ) -> Result<(), CanvasError> {
            self.text.push(copy_text(text).expect("test allocation"));
            self.text_colors.push(style.color);
            Ok(())
        }
    }

    #[test]
    fn checkbox_activation_changes_and_reports_state() {
        let mut ui = Ui::new(120, 40).unwrap();
        let checkbox = ui
            .add_checkbox(ui.root(), "ENABLE SOUND", false, Size::fill())
            .unwrap();

        assert_eq!(
            ui.update(UiInput {
                activate: true,
                ..UiInput::default()
            }),
            Some(UiEvent::Toggled {
                widget: checkbox,
                checked: true,
            })
        );
        assert!(ui.checked(checkbox).unwrap());
        assert_eq!(ui.update(UiInput::default()), None);
    }

    #[test]
    fn a_pointer_press_cannot_also_activate_the_focused_widget() {
        let mut ui = Ui::new(120, 60).unwrap();
        let button = ui
            .add_button(ui.root(), "CONFIRM", Size::fixed(60, 20))
            .unwrap();
        ui.update(UiInput::default());
        assert_eq!(ui.focused(), Some(button));
        assert_eq!(
            ui.update(UiInput {
                pointer: Some((119, 59)),
                pointer_activate: true,
                activate: true,
                ..UiInput::default()
            }),
            None
        );
        assert_eq!(
            ui.update(UiInput {
                pointer: Some((10, 10)),
                pointer_activate: true,
                activate: true,
                ..UiInput::default()
            }),
            Some(UiEvent::Activated(button))
        );
        assert_eq!(ui.update(UiInput::default()), None);
    }

    #[test]
    fn a_stationary_pointer_does_not_undo_controller_navigation() {
        let mut ui = Ui::new(120, 60).unwrap();
        let first = ui
            .add_button(ui.root(), "FIRST", Size::fixed(60, 20))
            .unwrap();
        let second = ui
            .add_button(ui.root(), "SECOND", Size::fixed(60, 20))
            .unwrap();
        let pointer = Some((10, 10));
        ui.update(UiInput {
            pointer,
            ..UiInput::default()
        });
        assert_eq!(ui.focused(), Some(first));
        ui.update(UiInput {
            pointer,
            direction: Some(Direction::Next),
            ..UiInput::default()
        });
        assert_eq!(ui.focused(), Some(second));
        ui.update(UiInput {
            pointer,
            ..UiInput::default()
        });
        assert_eq!(ui.focused(), Some(second));
        ui.update(UiInput {
            pointer: Some((11, 10)),
            ..UiInput::default()
        });
        assert_eq!(ui.focused(), Some(first));
    }

    #[test]
    fn focus_scope_keeps_navigation_and_pointer_activation_inside_modal() {
        let mut ui = Ui::new(200, 100).unwrap();
        let background = ui
            .add_button(ui.root(), "Background", Size::fixed(80, 30))
            .unwrap();
        let modal = ui
            .add_container(ui.root(), Layout::column(0, 0), Size::fixed(100, 80))
            .unwrap();
        let confirm = ui
            .add_button(modal, "Confirm", Size::fixed(80, 30))
            .unwrap();
        let cancel = ui.add_button(modal, "Cancel", Size::fixed(80, 30)).unwrap();
        ui.set_focus_scope(modal).unwrap();
        assert_eq!(ui.focus_scope(), modal);
        assert_eq!(ui.update(UiInput::default()), None);
        assert_eq!(ui.focused(), Some(confirm));

        ui.update(UiInput {
            direction: Some(Direction::Next),
            ..UiInput::default()
        });
        assert_eq!(ui.focused(), Some(cancel));
        assert_eq!(
            ui.update(UiInput {
                pointer: Some((5, 5)),
                pointer_activate: true,
                activate: true,
                ..UiInput::default()
            }),
            None
        );
        assert_eq!(ui.focused(), Some(cancel));

        ui.set_focus_scope(ui.root()).unwrap();
        assert_eq!(ui.focus_scope(), ui.root());
        ui.update(UiInput {
            direction: Some(Direction::Next),
            ..UiInput::default()
        });
        assert_eq!(ui.focused(), Some(background));
    }

    #[test]
    fn pointer_target_uses_focus_scope_and_skips_disabled_widgets_without_state_change() {
        let mut ui = Ui::new(100, 80).unwrap();
        let background = ui
            .add_button(ui.root(), "BACKGROUND", Size::fill_width(20))
            .unwrap();
        let modal = ui
            .add_container(ui.root(), Layout::column(0, 0), Size::fill())
            .unwrap();
        let disabled = ui
            .add_button(modal, "DISABLED", Size::fill_width(30))
            .unwrap();
        let enabled = ui
            .add_button(modal, "ENABLED", Size::fill_width(30))
            .unwrap();
        ui.set_enabled(disabled, false).unwrap();
        ui.set_focus_scope(modal).unwrap();

        assert_eq!(ui.pointer_target(10, 10), None);
        assert_eq!(ui.pointer_target(10, 40), None);
        assert_eq!(ui.pointer_target(10, 70), Some(enabled));
        assert_eq!(ui.focused(), None);
        assert_ne!(ui.focused(), Some(background));

        assert_eq!(
            ui.update(UiInput {
                pointer: Some((10, 70)),
                ..UiInput::default()
            }),
            None
        );
        assert_eq!(ui.focused(), Some(enabled));
    }

    #[test]
    fn focus_scope_rejects_non_container_widgets() {
        let mut ui = Ui::new(100, 60).unwrap();
        let button = ui
            .add_button(ui.root(), "Button", Size::fixed(50, 20))
            .unwrap();
        assert_eq!(
            ui.set_focus_scope(button),
            Err(UiError::FocusScopeIsNotContainer)
        );
    }

    #[test]
    fn radio_selection_is_exclusive_within_its_parent_group() {
        let mut ui = Ui::new(200, 80).unwrap();
        let panel = ui
            .add_container(ui.root(), Layout::column(0, 0), Size::fill())
            .unwrap();
        let first = ui
            .add_radio(panel, 7, "FIRST", true, Size::fill_width(24))
            .unwrap();
        let second = ui
            .add_radio(panel, 7, "SECOND", false, Size::fill_width(24))
            .unwrap();
        let other_group = ui
            .add_radio(panel, 8, "OTHER", true, Size::fill_width(24))
            .unwrap();

        assert!(ui.checked(first).unwrap());
        assert!(!ui.checked(second).unwrap());
        assert!(ui.checked(other_group).unwrap());
        ui.set_checked(second, true).unwrap();
        assert!(!ui.checked(first).unwrap());
        assert!(ui.checked(second).unwrap());
        assert!(ui.checked(other_group).unwrap());
    }

    #[test]
    fn slider_keyboard_and_pointer_use_the_real_range() {
        let mut ui = Ui::new(101, 24).unwrap();
        let slider = ui
            .add_slider(
                ui.root(),
                "VOLUME",
                SliderSpec {
                    min: 10,
                    max: 30,
                    step: 5,
                    value: 15,
                },
                Size::fill(),
            )
            .unwrap();

        assert_eq!(
            ui.update(UiInput {
                direction: Some(Direction::Right),
                ..UiInput::default()
            }),
            Some(UiEvent::ValueChanged {
                widget: slider,
                value: 20,
            })
        );
        assert_eq!(
            ui.update(UiInput {
                direction: Some(Direction::Left),
                ..UiInput::default()
            }),
            Some(UiEvent::ValueChanged {
                widget: slider,
                value: 15,
            })
        );
        assert_eq!(
            ui.update(UiInput {
                pointer: Some((100, 12)),
                pointer_activate: true,
                ..UiInput::default()
            }),
            Some(UiEvent::ValueChanged {
                widget: slider,
                value: 30,
            })
        );
        assert_eq!(ui.value(slider).unwrap(), 30);
        assert_eq!(
            ui.update(UiInput {
                direction: Some(Direction::Right),
                ..UiInput::default()
            }),
            None
        );
        assert_eq!(ui.set_value(slider, 31), Err(UiError::InvalidRange));
    }

    #[test]
    fn controls_at_their_limit_allow_focus_navigation() {
        let mut ui = Ui::new(100, 60).unwrap();
        let slider = ui
            .add_slider(
                ui.root(),
                "LEVEL",
                SliderSpec {
                    min: 0,
                    max: 1,
                    step: 1,
                    value: 1,
                },
                Size::fixed(100, 24),
            )
            .unwrap();
        let next = ui
            .add_button(ui.root(), "NEXT", Size::fixed(100, 24))
            .unwrap();

        assert_eq!(ui.update(UiInput::default()), None);
        assert_eq!(ui.focused(), Some(slider));
        assert_eq!(
            ui.update(UiInput {
                direction: Some(Direction::Right),
                ..UiInput::default()
            }),
            None
        );
        assert_eq!(ui.focused(), Some(next));
    }

    #[test]
    fn list_navigates_selects_and_rejects_invalid_options() {
        let mut ui = Ui::new(120, 60).unwrap();
        let list = ui
            .add_list(ui.root(), &["FIRST", "SECOND", "THIRD"], 0, Size::fill())
            .unwrap();

        assert_eq!(
            ui.update(UiInput {
                direction: Some(Direction::Down),
                ..UiInput::default()
            }),
            Some(UiEvent::SelectionChanged {
                widget: list,
                selected: 1
            })
        );
        assert_eq!(ui.selected(list).unwrap(), 1);
        assert_eq!(
            ui.update(UiInput {
                activate: true,
                ..UiInput::default()
            }),
            Some(UiEvent::SelectionChanged {
                widget: list,
                selected: 1
            })
        );
        assert_eq!(ui.set_selected(list, 3), Err(UiError::InvalidRange));
        assert_eq!(
            ui.add_list(ui.root(), &[], 0, Size::fill()),
            Err(UiError::EmptyOptions)
        );
    }

    #[test]
    fn progress_and_slider_inputs_are_validated() {
        let mut ui = Ui::new(120, 60).unwrap();
        assert_eq!(
            ui.add_slider(
                ui.root(),
                "BAD",
                SliderSpec {
                    min: 5,
                    max: 4,
                    step: 1,
                    value: 4,
                },
                Size::fill(),
            ),
            Err(UiError::InvalidRange)
        );
        assert_eq!(
            ui.add_slider(
                ui.root(),
                "BAD",
                SliderSpec {
                    min: 0,
                    max: 4,
                    step: 0,
                    value: 2,
                },
                Size::fill(),
            ),
            Err(UiError::InvalidRange)
        );
        assert_eq!(
            ui.add_progress(ui.root(), "LOAD", 101, Size::fill()),
            Err(UiError::InvalidRange)
        );
    }

    #[test]
    fn built_in_controls_draw_state_and_progress_width() {
        let mut ui = Ui::new(120, 140).unwrap();
        let root = ui.root();
        ui.add_checkbox(root, "SOUND", true, Size::fixed(120, 24))
            .unwrap();
        let slider = ui
            .add_slider(
                root,
                "VOLUME",
                SliderSpec {
                    min: 0,
                    max: 100,
                    step: 5,
                    value: 35,
                },
                Size::fixed(120, 34),
            )
            .unwrap();
        ui.add_list(root, &["FIRST", "SECOND"], 1, Size::fixed(120, 50))
            .unwrap();
        let progress = ui
            .add_progress(root, "LOAD", 0, Size::fixed(120, 24))
            .unwrap();
        ui.set_text_scale(slider, 1).unwrap();
        ui.set_text_scale(progress, 1).unwrap();
        ui.set_progress(progress, 25).unwrap();

        let mut canvas = CaptureCanvas::default();
        ui.draw(&mut canvas).unwrap();

        assert!(canvas.text.iter().any(|text| text == "SOUND"));
        assert!(canvas.text.iter().any(|text| text == "VOLUME: 35"));
        assert!(canvas.text.iter().any(|text| text == "SECOND"));
        assert!(canvas.text.iter().any(|text| text == "LOAD: 25%"));
        assert!(canvas.rectangles.contains(&Rect::new(0, 125, 30, 6)));
    }

    #[test]
    fn custom_content_draws_and_does_not_take_focus_by_default() {
        let mut ui = Ui::new(60, 30).unwrap();
        let custom = ui
            .add_custom(ui.root(), Box::new(PassiveCustom), Size::fill())
            .unwrap();
        assert!(!ui.nodes[custom.index()].focusable());
        assert_eq!(ui.update(UiInput::default()), None);
        assert_eq!(ui.focused(), None);

        let mut canvas = CaptureCanvas::default();
        ui.draw(&mut canvas).unwrap();
        assert!(canvas.rectangles.contains(&Rect::new(0, 0, 60, 30)));
    }

    #[test]
    fn focusable_custom_widget_receives_input_and_reports_changes() {
        let mut ui = Ui::new(60, 30).unwrap();
        let custom = ui
            .add_custom(ui.root(), Box::new(ActiveCustom), Size::fill())
            .unwrap();
        assert_eq!(
            ui.update(UiInput {
                activate: true,
                ..UiInput::default()
            }),
            Some(UiEvent::CustomUpdated(custom))
        );
    }
    #[test]
    fn public_layout_helpers_produce_exact_child_bounds() {
        let mut ui = Ui::new(100, 80).unwrap();
        assert_eq!(
            ui.viewport(),
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 80
            }
        );
        let column = ui
            .add_container(ui.root(), Layout::column(4, 2), Size::fill())
            .unwrap();
        let label = ui.add_label(column, "TITLE", Size::fill_width(10)).unwrap();
        let row = ui
            .add_container(column, Layout::row(3, 1), Size::fill())
            .unwrap();
        let left = ui.add_button(row, "LEFT", Size::fill_height(20)).unwrap();
        let right = ui.add_button(row, "RIGHT", Size::fill()).unwrap();
        assert_eq!(label.index(), 2);
        assert_eq!(ui.bounds(label), Ok(Rect::new(2, 2, 96, 10)));
        assert_eq!(ui.bounds(row), Ok(Rect::new(2, 16, 96, 62)));
        assert_eq!(ui.bounds(left), Ok(Rect::new(3, 17, 20, 60)));
        assert_eq!(ui.bounds(right), Ok(Rect::new(26, 17, 71, 60)));
        assert_eq!(ui.bounds(WidgetId(u32::MAX)), Err(UiError::UnknownWidget));
        let fixed = Size::fixed(7, 9);
        assert_eq!(fixed.width, Dimension::Pixels(7));
        assert_eq!(fixed.height, Dimension::Pixels(9));
    }

    #[test]
    fn text_and_color_updates_reach_the_drawing_boundary() {
        let mut ui = Ui::new(100, 40).unwrap();
        let label = ui
            .add_label(ui.root(), "BEFORE", Size::fixed(100, 20))
            .unwrap();
        let button = ui
            .add_button(ui.root(), "BUTTON", Size::fixed(100, 20))
            .unwrap();
        let mut before = CaptureCanvas::default();
        ui.draw(&mut before).unwrap();
        assert_eq!(before.text, ["BEFORE", "BUTTON"]);
        ui.set_text(label, "AFTER").unwrap();
        let background = Color::rgb(1, 2, 3);
        let focused = Color::rgb(4, 5, 6);
        let foreground = Color::rgba(7, 8, 9, 10);
        ui.set_colors(button, background, focused, foreground)
            .unwrap();
        let mut normal = CaptureCanvas::default();
        ui.draw(&mut normal).unwrap();
        assert_eq!(normal.text, ["AFTER", "BUTTON"]);
        assert!(normal.fill_colors.contains(&background));
        assert_eq!(normal.text_colors[1], foreground);
        assert_eq!(ui.update(UiInput::default()), None);
        assert_eq!(ui.focused(), Some(button));
        let mut selected = CaptureCanvas::default();
        ui.draw(&mut selected).unwrap();
        assert!(selected.fill_colors.contains(&focused));
        assert!(!selected.fill_colors.contains(&background));
        assert_eq!(
            ui.set_text(ui.root(), "INVALID"),
            Err(UiError::NotTextWidget)
        );
        assert_eq!(
            ui.set_text(WidgetId(u32::MAX), "INVALID"),
            Err(UiError::UnknownWidget)
        );
        assert_eq!(
            ui.set_colors(WidgetId(u32::MAX), background, focused, foreground),
            Err(UiError::UnknownWidget)
        );
        let mut after_errors = CaptureCanvas::default();
        ui.draw(&mut after_errors).unwrap();
        assert_eq!(after_errors.text, selected.text);
        assert_eq!(after_errors.fill_colors, selected.fill_colors);
    }

    #[test]
    fn visibility_and_enable_changes_remove_and_restore_activation() {
        let mut ui = Ui::new(100, 40).unwrap();
        let first = ui
            .add_button(ui.root(), "FIRST", Size::fixed(100, 20))
            .unwrap();
        let second = ui
            .add_button(ui.root(), "SECOND", Size::fixed(100, 20))
            .unwrap();
        let activate = UiInput {
            activate: true,
            ..UiInput::default()
        };
        assert_eq!(ui.update(activate), Some(UiEvent::Activated(first)));
        ui.set_enabled(first, false).unwrap();
        assert_eq!(ui.pointer_target(10, 10), None);
        assert_eq!(ui.update(activate), Some(UiEvent::Activated(second)));
        ui.set_visible(second, false).unwrap();
        assert_eq!(ui.update(activate), None);
        let mut hidden = CaptureCanvas::default();
        ui.draw(&mut hidden).unwrap();
        assert_eq!(hidden.text, ["FIRST"]);
        ui.set_enabled(first, true).unwrap();
        assert_eq!(ui.update(activate), Some(UiEvent::Activated(first)));
        ui.set_visible(second, true).unwrap();
        let mut visible = CaptureCanvas::default();
        ui.draw(&mut visible).unwrap();
        assert_eq!(visible.text, ["FIRST", "SECOND"]);
        assert_eq!(
            ui.set_enabled(WidgetId(u32::MAX), false),
            Err(UiError::UnknownWidget)
        );
        assert_eq!(
            ui.set_visible(WidgetId(u32::MAX), false),
            Err(UiError::UnknownWidget)
        );
        assert_eq!(ui.update(activate), Some(UiEvent::Activated(first)));
    }
}
