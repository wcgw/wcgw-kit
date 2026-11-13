//! The window layout: a tree of horizontal and vertical splits, and which window has the cursor.

use std::collections::HashMap;

use ratatui::layout::Rect;

use super::window::Window;

/// Identifies a window for as long as it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

/// How much room a window takes in its split, across for side-by-side windows and down for stacked ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    /// A share of what the windows of a fixed size leave, alike with the others: Vim's default.
    #[default]
    Shared,
    /// So many cells, as a sidebar: Vim's `'winfixwidth'`.
    Cells(u16),
    /// So much of the split, in percent.
    Percent(u16),
}

/// Where a window opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Left of the current window: `:vsplit`.
    Left,
    /// Right of it: `:rightbelow vsplit`.
    Right,
    /// Above it: `:split`.
    Above,
    /// Below it: `:rightbelow split`.
    Below,
    /// At the left of the screen, as tall as it: `:topleft vsplit`, for a sidebar.
    FarLeft,
    /// At the right of the screen: `:botright vsplit`.
    FarRight,
}

/// Which way a split divides its space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    /// `:split`: windows stacked, one above the other.
    Rows,
    /// `:vsplit`: windows side by side, with a separator column between them.
    Columns,
}

/// A direction to move between windows in, as `<C-w>h`, `j`, `k` and `l`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Toward {
    Left,
    Down,
    Up,
    Right,
}

#[derive(Debug, Clone)]
enum Split {
    Leaf(WindowId),
    Node(Axis, Vec<Split>),
}

/// Where everything goes on screen.
#[derive(Debug, Default)]
pub(super) struct Arrangement {
    /// Each window with its area, statusline row included, in tree order.
    pub(super) windows: Vec<(WindowId, Rect)>,
    /// The columns separating side-by-side windows.
    pub(super) separators: Vec<Separator>,
}

/// A column between side-by-side windows. Its last row is a row of statuslines, where it is not a bar but the
/// statusline of the window on its left goes on, as in Vim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Separator {
    /// Its column, from top to bottom.
    pub(super) area: Rect,
    /// The window on its left at its last row.
    pub(super) left: WindowId,
}

/// All open windows and their layout. There is always at least one, and one of them is current.
#[derive(Debug)]
pub(super) struct Windows {
    tree: Split,
    open: HashMap<WindowId, Window>,
    /// The windows of a size of their own, along the axis it was given for: a sidebar's width stays its column's
    /// when it is split in rows.
    sizes: HashMap<WindowId, (Axis, Size)>,
    current: WindowId,
    next_id: u64,
}

impl Windows {
    /// A layout of `window` alone.
    pub(super) fn new(window: Window) -> Self {
        let id = WindowId(0);
        Self {
            tree: Split::Leaf(id),
            open: HashMap::from([(id, window)]),
            sizes: HashMap::new(),
            current: id,
            next_id: 1,
        }
    }

    /// Opens `window` as `placement` says, `size` big, and makes it current.
    pub(super) fn open(&mut self, placement: Placement, size: Size, window: Window) -> WindowId {
        let id = WindowId(self.next_id);
        self.next_id += 1;
        self.open.insert(id, window);
        if size != Size::Shared {
            let axis = match placement {
                Placement::Above | Placement::Below => Axis::Rows,
                Placement::Left | Placement::Right | Placement::FarLeft | Placement::FarRight => {
                    Axis::Columns
                }
            };
            self.sizes.insert(id, (axis, size));
        }
        match placement {
            Placement::Left => {
                let _ = self.tree.place(self.current, id, Axis::Columns, true);
            }
            Placement::Right => {
                let _ = self.tree.place(self.current, id, Axis::Columns, false);
            }
            Placement::Above => {
                let _ = self.tree.place(self.current, id, Axis::Rows, true);
            }
            Placement::Below => {
                let _ = self.tree.place(self.current, id, Axis::Rows, false);
            }
            Placement::FarLeft | Placement::FarRight => {
                let before = placement == Placement::FarLeft;
                let root = std::mem::replace(&mut self.tree, Split::Leaf(id));
                self.tree = match root {
                    Split::Node(Axis::Columns, mut children) => {
                        let at = if before { 0 } else { children.len() };
                        children.insert(at, Split::Leaf(id));
                        Split::Node(Axis::Columns, children)
                    }
                    other if before => Split::Node(Axis::Columns, vec![Split::Leaf(id), other]),
                    other => Split::Node(Axis::Columns, vec![other, Split::Leaf(id)]),
                };
            }
        }
        self.current = id;
        id
    }

    /// The window `id`, if open.
    pub(super) fn find(&self, id: WindowId) -> Option<&Window> {
        self.open.get(&id)
    }

    /// The window `id`, if open, to change.
    pub(super) fn find_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.open.get_mut(&id)
    }

    /// Makes `id` current, if open.
    pub(super) fn focus_id(&mut self, id: WindowId) -> bool {
        let open = self.open.contains_key(&id);
        if open {
            self.current = id;
        }
        open
    }

    /// Closes `id`, unless it is the last window; the cursor goes as `:close` sends it when it was current.
    pub(super) fn close_id(&mut self, id: WindowId) -> bool {
        if !self.open.contains_key(&id) {
            return false;
        }
        let current = self.current;
        self.current = id;
        let closed = self.close();
        if closed && current != id {
            self.current = current;
        }
        if !closed {
            self.current = current;
        }
        closed
    }

    /// Gives window `id` `size`.
    pub(super) fn resize(&mut self, id: WindowId, size: Size) {
        if size == Size::Shared {
            self.sizes.remove(&id);
        } else {
            let axis = self
                .sizes
                .get(&id)
                .map(|&(axis, _)| axis)
                .or_else(|| self.tree.parent_axis(id))
                .unwrap_or(Axis::Columns);
            self.sizes.insert(id, (axis, size));
        }
    }

    /// The windows, in tree order: top to bottom, left to right.
    pub(super) fn ids(&self) -> Vec<WindowId> {
        self.order()
    }

    pub(super) fn current_id(&self) -> WindowId {
        self.current
    }

    pub(super) fn current(&self) -> &Window {
        &self.open[&self.current]
    }

    pub(super) fn current_mut(&mut self) -> &mut Window {
        self.open
            .get_mut(&self.current)
            .expect("the current window is open")
    }

    pub(super) fn get(&self, id: WindowId) -> &Window {
        &self.open[&id]
    }

    pub(super) fn count(&self) -> usize {
        self.open.len()
    }

    /// `:split` and `:vsplit`: a copy of the current window's view goes above or to the left of it and becomes
    /// current.
    pub(super) fn split(&mut self, axis: Axis) {
        let id = WindowId(self.next_id);
        self.next_id += 1;
        // Cloned so the new window starts where the current one is, then moves on its own.
        self.open.insert(id, self.current().clone());
        self.tree.split(self.current, id, axis);
        self.current = id;
    }

    /// Makes current the first window, in tree order, that shows buffer `buffer`; whether there is one.
    pub(super) fn go_to_buffer(&mut self, buffer: crate::app::BufferId) -> bool {
        let showing = self
            .order()
            .into_iter()
            .find(|id| self.open[id].buffer == buffer);
        if let Some(id) = showing {
            self.current = id;
        }
        showing.is_some()
    }

    /// `:close`: closes the current window, giving the cursor to the one before it (or after it, for the first).
    /// Returns `false`, closing nothing, when it is the last window.
    pub(super) fn close(&mut self) -> bool {
        if self.open.len() == 1 {
            return false;
        }
        let order = self.order();
        let at = order.iter().position(|&id| id == self.current).unwrap_or(0);
        let next = if at > 0 { order[at - 1] } else { order[at + 1] };
        self.tree.remove(self.current);
        self.open.remove(&self.current);
        self.sizes.remove(&self.current);
        self.current = next;
        true
    }

    /// `:only`: closes every window but the current one.
    pub(super) fn only(&mut self) {
        self.tree = Split::Leaf(self.current);
        self.open.retain(|&id, _| id == self.current);
        self.sizes.clear();
    }

    /// `<C-w>w` (`forward`) and `<C-w>W`: the next or previous window, wrapping around.
    pub(super) fn cycle(&mut self, forward: bool) {
        let order = self.order();
        let at = order.iter().position(|&id| id == self.current).unwrap_or(0);
        let next = if forward {
            (at + 1) % order.len()
        } else {
            (at + order.len() - 1) % order.len()
        };
        self.current = order[next];
    }

    /// `<C-w>h` and friends: the nearest window on that side of the current one, laid out in `area`, if any.
    pub(super) fn focus(&mut self, area: Rect, toward: Toward) {
        let arrangement = self.arrange(area);
        let Some(&(_, here)) = arrangement
            .windows
            .iter()
            .find(|(id, _)| *id == self.current)
        else {
            return;
        };
        let rows_overlap = |r: &Rect| r.top() < here.bottom() && here.top() < r.bottom();
        let columns_overlap = |r: &Rect| r.left() < here.right() && here.left() < r.right();
        let distance = |r: &Rect| match toward {
            Toward::Left if rows_overlap(r) && r.right() <= here.left() => {
                Some(here.left() - r.right())
            }
            Toward::Right if rows_overlap(r) && r.left() >= here.right() => {
                Some(r.left() - here.right())
            }
            Toward::Up if columns_overlap(r) && r.bottom() <= here.top() => {
                Some(here.top() - r.bottom())
            }
            Toward::Down if columns_overlap(r) && r.top() >= here.bottom() => {
                Some(r.top() - here.bottom())
            }
            _ => None,
        };
        if let Some((id, _)) = arrangement
            .windows
            .iter()
            .filter_map(|(id, rect)| distance(rect).map(|d| (*id, d)))
            .min_by_key(|&(_, d)| d)
        {
            self.current = id;
        }
    }

    /// Lays the windows out in `area`, sharing space evenly; side-by-side windows lose a column to a separator.
    pub(super) fn arrange(&self, area: Rect) -> Arrangement {
        let mut arrangement = Arrangement::default();
        arrange(&self.tree, area, &self.sizes, &mut arrangement);
        arrangement
    }

    /// Window ids in tree order: top to bottom, left to right.
    fn order(&self) -> Vec<WindowId> {
        let mut order = Vec::with_capacity(self.open.len());
        self.tree.leaves(&mut order);
        order
    }
}

impl Split {
    /// Puts `new` next to the leaf `target` along `axis`, before it or after it.
    fn place(&mut self, target: WindowId, new: WindowId, axis: Axis, before: bool) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                let pair = if before {
                    vec![Self::Leaf(new), Self::Leaf(target)]
                } else {
                    vec![Self::Leaf(target), Self::Leaf(new)]
                };
                *self = Self::Node(axis, pair);
                true
            }
            Self::Leaf(_) => false,
            Self::Node(node_axis, children) => {
                let at = children
                    .iter()
                    .position(|child| matches!(child, Self::Leaf(id) if *id == target));
                if let Some(at) = at
                    && *node_axis == axis
                {
                    children.insert(if before { at } else { at + 1 }, Self::Leaf(new));
                    return true;
                }
                children
                    .iter_mut()
                    .any(|child| child.place(target, new, axis, before))
            }
        }
    }

    /// Splits the leaf `target`, putting `new` before it.
    fn split(&mut self, target: WindowId, new: WindowId, axis: Axis) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                *self = Self::Node(axis, vec![Self::Leaf(new), Self::Leaf(target)]);
                true
            }
            Self::Leaf(_) => false,
            Self::Node(node_axis, children) => {
                let at = children
                    .iter()
                    .position(|child| matches!(child, Self::Leaf(id) if *id == target));
                if let Some(at) = at
                    && *node_axis == axis
                {
                    children.insert(at, Self::Leaf(new));
                    return true;
                }
                children
                    .iter_mut()
                    .any(|child| child.split(target, new, axis))
            }
        }
    }

    /// Removes the leaf `target`, collapsing splits left with one child.
    fn remove(&mut self, target: WindowId) -> bool {
        let Self::Node(_, children) = self else {
            return false;
        };
        let at = children
            .iter()
            .position(|child| matches!(child, Self::Leaf(id) if *id == target));
        match at {
            Some(at) => {
                children.remove(at);
            }
            None if children.iter_mut().any(|child| child.remove(target)) => {}
            None => return false,
        }
        if children.len() == 1 {
            *self = children.pop().expect("one child left");
        }
        true
    }

    /// The axis of the split holding the leaf `target`, if any holds it.
    fn parent_axis(&self, target: WindowId) -> Option<Axis> {
        let Self::Node(axis, children) = self else {
            return None;
        };
        if children
            .iter()
            .any(|child| matches!(child, Self::Leaf(id) if *id == target))
        {
            return Some(*axis);
        }
        children.iter().find_map(|child| child.parent_axis(target))
    }

    fn leaves(&self, out: &mut Vec<WindowId>) {
        match self {
            Self::Leaf(id) => out.push(*id),
            Self::Node(_, children) => children.iter().for_each(|child| child.leaves(out)),
        }
    }
}

/// The room each of `children` takes of `total` cells: windows of a size of their own first, then alike shares of
/// what is left; the last child takes what nobody else can.
fn shares(
    children: &[Split],
    total: u16,
    axis: Axis,
    sizes: &HashMap<WindowId, (Axis, Size)>,
) -> Vec<u16> {
    // A split's size along `axis` is that of a window in it sized so, as a sidebar split in rows keeps its width.
    let size_of = |child: &Split| {
        let mut leaves = Vec::new();
        child.leaves(&mut leaves);
        leaves
            .into_iter()
            .find_map(|id| sizes.get(&id).filter(|(along, _)| *along == axis))
            .map_or(Size::Shared, |&(_, size)| size)
    };
    let mut left = total;
    let mut room: Vec<Option<u16>> = children
        .iter()
        .map(|child| {
            let fixed = match size_of(child) {
                Size::Shared => return None,
                Size::Cells(cells) => cells,
                Size::Percent(percent) => {
                    u16::try_from(u32::from(total) * u32::from(percent.min(100)) / 100)
                        .unwrap_or(total)
                }
            };
            let fixed = fixed.min(left);
            left -= fixed;
            Some(fixed)
        })
        .collect();
    let shared =
        u16::try_from(room.iter().filter(|room| room.is_none()).count()).unwrap_or(u16::MAX);
    if shared == 0 {
        // Every window has a size of its own: the last takes what is left over.
        if let Some(Some(last)) = room.last_mut() {
            *last += left;
        }
        return room.into_iter().map(Option::unwrap_or_default).collect();
    }
    let (base, extra) = (left / shared, left % shared);
    let mut nth = 0;
    room.iter_mut()
        .map(|room| {
            room.unwrap_or_else(|| {
                nth += 1;
                base + u16::from(nth <= extra)
            })
        })
        .collect()
}

fn arrange(
    split: &Split,
    area: Rect,
    sizes: &HashMap<WindowId, (Axis, Size)>,
    out: &mut Arrangement,
) {
    let (axis, children) = match split {
        Split::Leaf(id) => {
            out.windows.push((*id, area));
            return;
        }
        Split::Node(axis, children) => (*axis, children),
    };
    let count = u16::try_from(children.len()).unwrap_or(u16::MAX).max(1);
    let total = match axis {
        Axis::Rows => area.height,
        Axis::Columns => area.width.saturating_sub(count - 1),
    };
    let room = shares(children, total, axis, sizes);
    let mut offset = 0;
    for ((i, child), size) in (0..count).zip(children).zip(room) {
        let child_area = match axis {
            Axis::Rows => Rect::new(area.x, area.y + offset, area.width, size),
            Axis::Columns => Rect::new(area.x + offset, area.y, size, area.height),
        };
        arrange(child, child_area, sizes, out);
        offset += size;
        if axis == Axis::Columns && i + 1 < count {
            // Windows are laid out top to bottom, left to right: the last one is the child's bottom right.
            let (left, _) = *out.windows.last().expect("a split's child holds a window");
            out.separators.push(Separator {
                area: Rect::new(area.x + offset, area.y, 1, area.height),
                left,
            });
            offset += 1;
        }
    }
}
