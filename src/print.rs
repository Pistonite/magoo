//! Printing utilities

use std::io::{IsTerminal, Write};
use std::process::Command;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static mut VERBOSE: bool = false;
static mut QUIET: bool = true;
static mut COLOR_CHOICE: ColorChoice = ColorChoice::Auto;

/// Set the global printing options
///
/// If color is not speficied, it will be determined from the git config `color.ui`.
pub fn set_options(verbose: bool, quiet: bool, color: Option<bool>) {
    let color_choice = match color {
        Some(true) => ColorChoice::Always,
        Some(false) => ColorChoice::Never,
        None => {
            let mut color_choice = get_color_choice_from_git();
            if color_choice == ColorChoice::Auto && !std::io::stdout().is_terminal() {
                color_choice = ColorChoice::Never;
            }
            color_choice
        }
    };
    unsafe {
        VERBOSE = verbose;
        QUIET = quiet;
        COLOR_CHOICE = color_choice;
    }
}

fn get_color_choice_from_git() -> ColorChoice {
    let output = match Command::new("git").args(["config", "color.ui"]).output() {
        Ok(output) => output,
        Err(_) => return ColorChoice::Auto,
    };

    if output.stdout == b"true" {
        ColorChoice::Always
    } else if output.stdout == b"false" {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    }
}

pub fn stdout() -> StandardStream {
    unsafe { StandardStream::stdout(COLOR_CHOICE) }
}

#[inline]
pub fn is_quiet() -> bool {
    unsafe { QUIET && !VERBOSE }
}

#[inline]
pub fn is_verbose() -> bool {
    unsafe { VERBOSE }
}

pub fn warn_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Yellow));
    x
}

pub fn progress_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Yellow)).set_intense(true);
    x
}

pub fn error_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Red)).set_intense(true);
    x
}

pub fn verbose_color() -> ColorSpec {
    let mut x = ColorSpec::new();
    x.set_fg(Some(Color::Black)).set_intense(true);
    x
}

/// Print using info color
macro_rules! println_info {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            println!($($args)*);
        }
    };
}
pub(crate) use println_info;

/// Print using warning color
macro_rules! println_warn {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::warn_color());
            let _ = writeln!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
pub(crate) use println_warn;

/// Print using warning color without a newline
#[allow(unused_macros)]
macro_rules! print_warn {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::warn_color());
            let _ = write!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
#[allow(unused)]
pub(crate) use print_warn;

/// Print using error color
macro_rules! println_error {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::error_color());
            let _ = writeln!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
pub(crate) use println_error;

/// Print using error color without a newline
#[allow(unused_macros)]
macro_rules! print_error {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::error_color());
            let _ = write!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
#[allow(unused)]
pub(crate) use print_error;

/// Print process
macro_rules! print_progress {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::{Write, IsTerminal};
            use termcolor::WriteColor;
            if std::io::stdout().is_terminal() {
                let mut stdout = $crate::print::stdout();
                let _ = stdout.set_color(&$crate::print::progress_color());
                if let Some(cols) = termsize::get().map(|size| size.cols as usize) {
                    let _ = write!(&mut stdout, "{:width$}\r", "", width = cols - 1);
                }
                let _ = write!(&mut stdout, $($args)*);
                let _ = write!(&mut stdout, "\r");
                let _ = std::io::stdout().flush();
            } else {
                println!($($args)*);
            }
        }
    };
}
pub(crate) use print_progress;

/// Clear the progress line and reset the color
pub fn progress_done() {
    if is_quiet() {
        return;
    }
    if std::io::stdout().is_terminal() {
        let mut stdout = stdout();
        let _ = stdout.reset();
        if let Some(cols) = termsize::get().map(|size| size.cols as usize) {
            print!("{:width$}\r", "", width = cols - 1);
        }
        let _ = std::io::stdout().flush();
    }
}

/// Print using dimmed color
macro_rules! println_dimmed {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::verbose_color());
            let _ = writeln!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
pub(crate) use println_dimmed;

/// Print using dimmed color without a new line
#[allow(unused_macros)]
macro_rules! print_dimmed {
    ($($args:tt)*) => {
        if !$crate::print::is_quiet() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::verbose_color());
            let _ = write!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
#[allow(unused)]
pub(crate) use print_dimmed;

/// Print message if verbose is true
macro_rules! println_verbose {
    ($($args:tt)*) => {
        if $crate::print::is_verbose() {
            use std::io::Write;
            use termcolor::WriteColor;
            let mut stdout = $crate::print::stdout();
            let _ = stdout.set_color(&$crate::print::verbose_color());
            let _ = writeln!(&mut stdout, $($args)*);
            let _ = stdout.reset();
        }
    };
}
pub(crate) use println_verbose;
