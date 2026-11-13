//! A question in the command line, answered by the next key.

use crate::app::{App, Effects};
use crate::doc::Document;
use crate::shell::Kit;

/// What answering does: hears the key's character, or `'\0'` for a key that is none.
pub(super) type Then<A> = Box<dyn FnOnce(&mut A, &mut Kit<A>, char) -> Effects<A>>;

/// A question, what it is about, and what the answer does.
pub(super) struct Confirm<A: App> {
    question: String,
    /// What the question is about, shown above it.
    shown: Option<Document<A::Target>>,
    then: Then<A>,
}

impl<A: App> Confirm<A> {
    pub(super) fn new(question: String, shown: Option<Document<A::Target>>, then: Then<A>) -> Self {
        Self {
            question,
            shown,
            then,
        }
    }

    /// The question, with its answers.
    pub(super) fn question(&self) -> &str {
        &self.question
    }

    /// What the question is about, to show above it.
    pub(super) fn shown(&self) -> Option<&Document<A::Target>> {
        self.shown.as_ref()
    }

    /// Answers with `answer`.
    pub(super) fn answer(self, app: &mut A, kit: &mut Kit<A>, answer: char) -> Effects<A> {
        (self.then)(app, kit, answer)
    }
}
