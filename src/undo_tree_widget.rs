use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::{Color, Element, Event, Length, Rectangle, Size, Theme};

use crate::undo_tree::{NodeId, UndoNode};

type Renderer = iced::Renderer;

pub const ROW_HEIGHT: f32 = 28.0;
pub const NODE_R: f32 = 5.0;
const CLICK_RADIUS: f32 = 10.0;
const BRANCH_X: f32 = 20.0;
pub const PANEL_WIDTH: f32 = 220.0;
const START_X: f32 = 24.0;
pub const START_Y: f32 = 20.0;

pub fn format_elapsed(ts: std::time::SystemTime) -> String {
    let secs = ts.elapsed().unwrap_or_default().as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

pub fn node_positions(nodes: &[UndoNode]) -> Vec<(NodeId, f32, f32)> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut row = 0usize;
    fn dfs(id: NodeId, x: f32, row: &mut usize, nodes: &[UndoNode], result: &mut Vec<(NodeId, f32, f32)>) {
        let y = START_Y + *row as f32 * ROW_HEIGHT;
        result.push((id, x, y));
        *row += 1;
        let children = &nodes[id].children;
        for (i, &child_id) in children.iter().enumerate().skip(1) {
            dfs(child_id, x + i as f32 * BRANCH_X, row, nodes, result);
        }
        if let Some(&main_child) = children.first() {
            dfs(main_child, x, row, nodes, result);
        }
    }
    dfs(0, START_X, &mut row, nodes, &mut result);
    result
}

pub struct UndoTreeWidget<'a, Message> {
    nodes: &'a [UndoNode],
    current: NodeId,
    selected: Option<NodeId>,
    on_select: Box<dyn Fn(NodeId) -> Message + 'a>,
}

impl<'a, Message> UndoTreeWidget<'a, Message> {
    pub fn new(
        nodes: &'a [UndoNode],
        current: NodeId,
        selected: Option<NodeId>,
        on_select: impl Fn(NodeId) -> Message + 'a,
    ) -> Self {
        Self { nodes, current, selected, on_select: Box::new(on_select) }
    }
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for UndoTreeWidget<'_, Message> {
    fn tag(&self) -> widget::tree::Tag { widget::tree::Tag::stateless() }
    fn state(&self) -> widget::tree::State { widget::tree::State::None }
    fn children(&self) -> Vec<widget::Tree> { vec![] }
    fn diff(&self, _tree: &mut widget::Tree) {}

    fn size(&self) -> Size<Length> {
        Size { width: Length::Fixed(PANEL_WIDTH), height: Length::Shrink }
    }

    fn layout(&mut self, _tree: &mut widget::Tree, _renderer: &Renderer, _limits: &layout::Limits) -> layout::Node {
        let n = self.nodes.len();
        let height = START_Y + n as f32 * ROW_HEIGHT + 8.0;
        layout::Node::new(Size::new(PANEL_WIDTH, height))
    }

    fn draw(
        &self, _tree: &widget::Tree, renderer: &mut Renderer,
        _theme: &Theme, _style: &renderer::Style,
        layout: Layout<'_>, _cursor: mouse::Cursor, _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let positions = node_positions(self.nodes);
        let pos_map: std::collections::HashMap<NodeId, (f32, f32)> =
            positions.iter().map(|&(id, x, y)| (id, (x, y))).collect();

        for &(id, x, y) in &positions {
            let ax = bounds.x + x;
            let ay = bounds.y + y;

            if let Some(parent_id) = self.nodes[id].parent {
                if let Some(&(px, py)) = pos_map.get(&parent_id) {
                    let bpx = bounds.x + px;
                    let bpy = bounds.y + py;
                    if (ax - bpx).abs() < 1.0 {
                        renderer.fill_quad(renderer::Quad {
                            bounds: Rectangle { x: ax - 0.5, y: bpy + NODE_R, width: 1.0, height: ay - bpy - NODE_R * 2.0 },
                            ..Default::default()
                        }, Color::from_rgb(0.35, 0.35, 0.35));
                    } else {
                        let mid_y = bpy + ROW_HEIGHT * 0.5;
                        renderer.fill_quad(renderer::Quad {
                            bounds: Rectangle { x: bpx - 0.5, y: bpy + NODE_R, width: 1.0, height: mid_y - bpy - NODE_R },
                            ..Default::default()
                        }, Color::from_rgb(0.35, 0.35, 0.35));
                        let (lx, rx) = if ax > bpx { (bpx, ax) } else { (ax, bpx) };
                        renderer.fill_quad(renderer::Quad {
                            bounds: Rectangle { x: lx, y: mid_y - 0.5, width: rx - lx + 1.0, height: 1.0 },
                            ..Default::default()
                        }, Color::from_rgb(0.35, 0.35, 0.35));
                        renderer.fill_quad(renderer::Quad {
                            bounds: Rectangle { x: ax - 0.5, y: mid_y, width: 1.0, height: ay - mid_y - NODE_R },
                            ..Default::default()
                        }, Color::from_rgb(0.35, 0.35, 0.35));
                    }
                }
            }

            let is_current = id == self.current;
            let is_selected = self.selected == Some(id);
            let node_color = if is_current {
                Color::from_rgb(1.0, 0.75, 0.0)
            } else if is_selected {
                Color::from_rgb(0.4, 0.7, 1.0)
            } else {
                Color::from_rgb(0.4, 0.4, 0.4)
            };

            renderer.fill_quad(renderer::Quad {
                bounds: Rectangle { x: ax - NODE_R, y: ay - NODE_R, width: NODE_R * 2.0, height: NODE_R * 2.0 },
                border: iced::Border { radius: NODE_R.into(), ..Default::default() },
                ..Default::default()
            }, node_color);
        }
    }

    fn update(
        &mut self, _tree: &mut widget::Tree, event: &Event,
        layout: Layout<'_>, cursor: mouse::Cursor, _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>, _viewport: &Rectangle,
    ) {
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position() {
                let bounds = layout.bounds();
                for (id, x, y) in node_positions(self.nodes) {
                    let cx = bounds.x + x;
                    let cy = bounds.y + y;
                    if ((pos.x - cx).powi(2) + (pos.y - cy).powi(2)).sqrt() <= CLICK_RADIUS {
                        shell.publish((self.on_select)(id));
                        return;
                    }
                }
            }
        }
    }

    fn mouse_interaction(
        &self, _tree: &widget::Tree, layout: Layout<'_>,
        cursor: mouse::Cursor, _viewport: &Rectangle, _renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(pos) = cursor.position() {
            let bounds = layout.bounds();
            for (_, x, y) in node_positions(self.nodes) {
                let cx = bounds.x + x;
                let cy = bounds.y + y;
                if ((pos.x - cx).powi(2) + (pos.y - cy).powi(2)).sqrt() <= CLICK_RADIUS {
                    return mouse::Interaction::Pointer;
                }
            }
        }
        mouse::Interaction::default()
    }

    fn operate(&mut self, _: &mut widget::Tree, _: Layout<'_>, _: &Renderer, _: &mut dyn widget::Operation) {}

    fn overlay<'b>(
        &'b mut self, _: &'b mut widget::Tree, _: Layout<'b>,
        _: &Renderer, _: &Rectangle, _: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> { None }
}

impl<'a, Message: Clone + 'a> From<UndoTreeWidget<'a, Message>> for Element<'a, Message> {
    fn from(w: UndoTreeWidget<'a, Message>) -> Element<'a, Message> { Element::new(w) }
}

