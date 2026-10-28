// Where the work of the program says what it is doing. In the window it is the
// output of a job (see jobs.rs): the lines `@@step <message> <argument>` are
// messages the window translates. On the command line (`--cli`) it is the
// terminal.

use super::jobs::JobLog;

pub trait Report {
    /// A line of output, as it is.
    fn line(&self, text: &str);
    /// A message for the window, such as `installed forge@jmmaranan.com`.
    fn step(&self, text: &str);
}

impl Report for JobLog {
    fn line(&self, text: &str) {
        JobLog::line(self, text);
    }

    fn step(&self, text: &str) {
        JobLog::step(self, text);
    }
}

/// The terminal, for the command line.
pub struct Console;

impl Report for Console {
    fn line(&self, text: &str) {
        println!("{text}");
    }

    fn step(&self, text: &str) {
        println!("{text}");
    }
}
