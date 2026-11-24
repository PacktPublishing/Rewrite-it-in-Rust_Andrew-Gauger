//! The order form's save lifecycle as one value (Chapter 13).
//!
//! A form is `Editing`, `Saving` or `Error`. There is no `Saved`: a save
//! that lands leaves the machine, as an `Action` for the screen to run.
//! Nothing here draws, sends or waits. `next` takes the phase and an
//! event and returns the new phase and what the screen must do.
//!
//! The phase carries no request ID. The app already stamps every save
//! with the screen it came from (`ScreenId`) and applies the answer
//! only if that screen is still open, so an answer for a form that is
//! gone never reaches `next`.

/// What the rep asked for when the save was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Save, and stay on the record.
    Stay,
    /// Save & Close.
    Close,
    /// Save & New.
    New,
    /// A navigation-bar move or the find box: save, then go here.
    Go(Option<i32>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Editing,
    Saving {
        then: Then,
    },
    /// Postgres or the network said no. Never a bad field: validation
    /// gates the save, so the form does not reach `Saving` with one.
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Save pressed.
    Save {
        then: Then,
    },
    Saved,
    Failed(String),
    /// Another rep saved the record first (Chapter 12).
    Conflict,
    /// The rep typed.
    Edited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing for the screen to do.
    None,
    /// The save landed: run what the rep asked for.
    Leave(Then),
    /// The save was refused: open the three-way dialog.
    Settle,
    /// An answer when this form has no save out.
    Stale,
}

impl Phase {
    pub fn is_saving(&self) -> bool {
        matches!(self, Phase::Saving { .. })
    }

    pub fn next(&self, event: Event) -> (Phase, Action) {
        use Phase::*;
        match (self, event) {
            // One save at a time: the button is disabled while
            // `Saving`, so a second press is dropped, not queued.
            (Saving { .. }, Event::Save { .. }) => {
                (self.clone(), Action::None)
            }
            (_, Event::Save { then }) => {
                (Saving { then }, Action::None)
            }
            // The fields are disabled while `Saving`.
            (Saving { .. }, Event::Edited) => {
                (self.clone(), Action::None)
            }
            (_, Event::Edited) => (Editing, Action::None),
            (Saving { then }, Event::Saved) => {
                (Editing, Action::Leave(*then))
            }
            (Saving { .. }, Event::Failed(why)) => {
                (Error(why), Action::None)
            }
            (Saving { .. }, Event::Conflict) => {
                (Editing, Action::Settle)
            }
            // `Editing` or `Error`: nothing is out.
            (
                _,
                Event::Saved | Event::Failed(_) | Event::Conflict,
            ) => (self.clone(), Action::Stale),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saving(then: Then) -> Phase {
        Phase::Saving { then }
    }

    #[test]
    fn save_moves_editing_to_saving() {
        let (p, a) =
            Phase::Editing.next(Event::Save { then: Then::Close });
        assert_eq!(p, saving(Then::Close));
        assert_eq!(a, Action::None);
    }

    #[test]
    fn saved_leaves_the_machine_as_what_was_asked() {
        for then in
            [Then::Stay, Then::Close, Then::New, Then::Go(Some(7))]
        {
            let (p, a) = saving(then).next(Event::Saved);
            assert_eq!(p, Phase::Editing);
            assert_eq!(a, Action::Leave(then));
        }
    }

    #[test]
    fn failed_keeps_the_form_open_in_error() {
        let (p, a) =
            saving(Then::New).next(Event::Failed("no".into()));
        assert_eq!(p, Phase::Error("no".into()));
        assert_eq!(a, Action::None);
    }

    #[test]
    fn a_conflict_goes_to_the_dialog_and_back_to_editing() {
        let (p, a) = saving(Then::Close).next(Event::Conflict);
        assert_eq!(p, Phase::Editing);
        assert_eq!(a, Action::Settle);
    }

    #[test]
    fn an_answer_with_nothing_out_is_stale() {
        for p in [Phase::Editing, Phase::Error("x".into())] {
            for e in [
                Event::Saved,
                Event::Failed("x".into()),
                Event::Conflict,
            ] {
                assert_eq!(p.next(e), (p.clone(), Action::Stale));
            }
        }
    }

    #[test]
    fn a_second_save_while_saving_is_dropped() {
        let s = saving(Then::Close);
        let e = Event::Save { then: Then::New };
        assert_eq!(s.next(e), (s.clone(), Action::None));
    }

    #[test]
    fn typing_clears_an_error_and_is_ignored_while_saving() {
        let e = Phase::Error("x".into());
        assert_eq!(e.next(Event::Edited).0, Phase::Editing);
        let s = saving(Then::Stay);
        assert_eq!(s.next(Event::Edited).0, s);
    }

    #[test]
    fn save_after_an_error_tries_again() {
        let e = Phase::Error("x".into());
        let (p, _) = e.next(Event::Save { then: Then::Stay });
        assert_eq!(p, saving(Then::Stay));
    }
}
