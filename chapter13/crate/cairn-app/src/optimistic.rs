//! A save shown before Postgres has answered (Chapter 12). The list
//! takes the edited row at once, marked pending, and keeps the row it
//! replaced. Three answers move it on: stored, it is confirmed; refused
//! or failed, the row goes back; in conflict, the other rep's row takes
//! its place and the dialog settles the rest.
//!
//! Safe here because a refused conditional update changed nothing in
//! Postgres, and a failed one rolled back its transaction: putting the
//! last good row back makes the screen true again. It never touches the
//! rep's draft in the form.

use crate::worker::RequestId;

/// A row as the list shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Shown<T> {
    Confirmed(T),
    Pending {
        shown: T,
        last_good: T,
        request: RequestId,
    },
}

impl<T> Shown<T> {
    pub fn pending(
        shown: T,
        last_good: T,
        request: RequestId,
    ) -> Self {
        Shown::Pending {
            shown,
            last_good,
            request,
        }
    }

    /// What the list draws.
    pub fn shown(&self) -> &T {
        match self {
            Shown::Confirmed(r) => r,
            Shown::Pending { shown, .. } => shown,
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Shown::Pending { .. })
    }

    /// Stored: the row Postgres wrote, with its new version.
    pub fn confirmed(self, stored: T) -> Self {
        Shown::Confirmed(stored)
    }

    /// Refused or failed: the row as it was.
    pub fn rolled_back(self) -> Self {
        match self {
            Shown::Pending { last_good, .. } => {
                Shown::Confirmed(last_good)
            }
            confirmed => confirmed,
        }
    }
}
