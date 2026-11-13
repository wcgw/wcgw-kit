//! The statusline's segments (D10): the kit's (mode, buffer, ruler, where the view is) and the application's,
//! dropped by priority when the line is too narrow.

use std::fmt;
use std::rc::Rc;

use crate::app::App;
use crate::shell::Kit;

/// Which end of the statusline a segment goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// After the mode and the buffer's name.
    Left,
    /// Before the ruler.
    Right,
}

/// What a segment shows now, if anything: its text, without padding.
pub type Render<A> = Rc<dyn Fn(&A, &Kit<A>) -> Option<String>>;

/// A part of the statusline an application adds, such as unread counts or the account connected.
pub struct Segment<A: App> {
    pub(crate) side: Side,
    pub(crate) priority: u8,
    pub(crate) render: Render<A>,
}

impl<A: App> fmt::Debug for Segment<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Segment")
            .field("side", &self.side)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

impl<A: App> Segment<A> {
    /// A segment on `side` showing what `render` says; when the line is short of room, segments of a lower
    /// `priority` go first, before the buffer's name is shortened.
    pub fn new(
        side: Side,
        priority: u8,
        render: impl Fn(&A, &Kit<A>) -> Option<String> + 'static,
    ) -> Self {
        Self {
            side,
            priority,
            render: Rc::new(render),
        }
    }
}
