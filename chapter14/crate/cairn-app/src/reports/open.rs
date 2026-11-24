//! Showing a finished PDF. The one platform-specific step in the
//! chapter, so it sits behind a trait at the app's edge: a test hands
//! the app a recorder, a tablet build hands it something else.

use std::path::Path;
use std::process::Command;

pub trait OpenFile: Send {
    fn open(&self, path: &Path) -> std::io::Result<()>;
}

/// The desktop's own viewer.
pub struct System;

impl OpenFile for System {
    fn open(&self, path: &Path) -> std::io::Result<()> {
        let mut cmd = if cfg!(target_os = "macos") {
            Command::new("open")
        } else if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", ""]);
            c
        } else {
            Command::new("xdg-open")
        };
        cmd.arg(path).spawn().map(|_| ())
    }
}
