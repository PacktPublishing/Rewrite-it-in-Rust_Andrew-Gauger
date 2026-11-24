//! The lookups and totals the screens read, rebuilt off the UI
//! thread. Rebuilding them over the whole snapshot takes about two
//! frames, so the UI hands each answer to this thread instead. It
//! keeps its own copy of the snapshot, merges the answer into it,
//! builds a new `AppContext`, and hands that back whole. The UI swaps
//! it in between two frames, and sends the old one back here to be
//! dropped.

use crate::context::{AppContext, Saved};
use crate::data::{Rows, Snapshot};
use crate::orchestrator::ScreenId;
use std::sync::mpsc::{self, Receiver, Sender};

/// An answer from the workers, to merge.
pub enum Change {
    Read(Rows, Box<Snapshot>),
    /// A save as stored, with the document number Postgres gave it.
    Saved(Box<Saved>, Option<String>),
    /// A save shown before Postgres has answered: the list takes the
    /// edited row at once.
    Pending(Box<Saved>),
}

/// What the UI does once the context it waited for is in.
#[derive(Debug)]
pub enum Done {
    Pending,
    Read {
        screen: ScreenId,
        rows: Rows,
        changed: bool,
    },
    Saved {
        screen: ScreenId,
        key: i32,
        number: Option<String>,
    },
}

/// A finished context, if anything changed, and what each change was.
pub struct Built {
    pub ctx: Option<Box<AppContext>>,
    pub done: Vec<Done>,
}

enum Job {
    Change {
        screen: ScreenId,
        change: Change,
        wake: egui::Context,
    },
    Retire(Box<AppContext>),
}

pub struct Builder {
    jobs: Sender<Job>,
    built: Receiver<Built>,
    /// Changes sent and not yet back.
    pending: usize,
}

impl Builder {
    /// The thread, starting from the snapshot the UI's context holds.
    pub fn spawn(data: Snapshot) -> Builder {
        let (jobs, take) = mpsc::channel();
        let (give, built) = mpsc::channel();
        std::thread::Builder::new()
            .name("cairn-builder".into())
            .spawn(move || build(data, take, give))
            .expect("a thread");
        Builder {
            jobs,
            built,
            pending: 0,
        }
    }

    pub fn change(
        &mut self,
        screen: ScreenId,
        change: Change,
        wake: &egui::Context,
    ) {
        let wake = wake.clone();
        if self
            .jobs
            .send(Job::Change {
                screen,
                change,
                wake,
            })
            .is_ok()
        {
            self.pending += 1;
        }
    }

    /// Drop an old context here, not on the UI thread.
    pub fn retire(&self, old: Box<AppContext>) {
        let _ = self.jobs.send(Job::Retire(old));
    }

    /// A finished context, if one is ready. Never waits.
    pub fn try_take(&mut self) -> Option<Built> {
        let built = self.built.try_recv().ok()?;
        self.pending -= built.done.len();
        Some(built)
    }

    /// The next finished context, waiting for it: for tests. `None`
    /// once nothing is pending.
    pub fn wait(&mut self) -> Option<Built> {
        if self.pending == 0 {
            return None;
        }
        let built = self.built.recv().ok()?;
        self.pending -= built.done.len();
        Some(built)
    }

    pub fn pending(&self) -> usize {
        self.pending
    }
}

/// The thread: everything that's arrived, merged, then one rebuild.
fn build(
    mut data: Snapshot,
    jobs: Receiver<Job>,
    built: Sender<Built>,
) {
    while let Ok(first) = jobs.recv() {
        let (mut done, mut changed, mut wake) =
            (Vec::new(), false, None);
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            let (screen, change) = match job {
                Job::Retire(old) => {
                    drop(old);
                    continue;
                }
                Job::Change {
                    screen,
                    change,
                    wake: w,
                } => {
                    wake = Some(w);
                    (screen, change)
                }
            };
            done.push(match change {
                Change::Read(rows, fresh) => {
                    let new = data.merge(rows, *fresh);
                    changed |= new;
                    Done::Read {
                        screen,
                        rows,
                        changed: new,
                    }
                }
                Change::Pending(saved) => {
                    changed = true;
                    data.apply(*saved);
                    Done::Pending
                }
                Change::Saved(saved, number) => {
                    changed = true;
                    let key = data.apply(*saved);
                    Done::Saved {
                        screen,
                        key,
                        number,
                    }
                }
            });
        }
        if done.is_empty() {
            continue;
        }
        let ctx =
            changed.then(|| Box::new(AppContext::new(data.clone())));
        if built.send(Built { ctx, done }).is_err() {
            break;
        }
        if let Some(wake) = wake {
            wake.request_repaint();
        }
    }
}
