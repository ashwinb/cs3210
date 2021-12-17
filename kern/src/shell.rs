use shim::io;
use shim::path::{Component, Path, PathBuf};
use io::{Read, Write as ioWrite};

use core::fmt::Write;
use core::str;
use stack_vec::StackVec;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry, File, Metadata, Timestamp};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::ALLOCATOR;
use crate::FILESYSTEM;

/// Error type for `Command` parse failures.
#[derive(Debug)]
enum Error {
    Empty,
    TooManyArgs,
}

/// A structure representing a single shell command.
struct Command<'a> {
    args: StackVec<'a, &'a str>,
}

impl<'a> Command<'a> {
    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
        let mut args = StackVec::new(buf);
        for arg in s.split(' ').filter(|a| !a.is_empty()) {
            args.push(arg).map_err(|_| Error::TooManyArgs)?;
        }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// Returns this command's path. This is equivalent to the first argument.
    fn path(&self) -> &str {
        self.args[0]
    }
}

fn read_and_parse_line<'a>(line_buf: &'a mut [u8], args_buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
    let mut console = CONSOLE.lock();

    let max = line_buf.len();
    let mut n: usize = 0;

    // we can probably use a StackVec<u8> and ::{push, pop} to avoid book-keeping with the count here
    loop {
        let b = console.read_byte();
        if n == 0 && b == 0x0 {
            // null bytes from the Rx line
            continue;
        }

        if b == 0x7f || b == 0x08 {
            if n == 0 {
                console.write_byte(0x7);
            } else {
                n -= 1;
                console.write_str("\x08 \x08").unwrap();
            }
            continue;
        }

        if b == b'\r' || b == b'\n' {
            console.write_byte(b);
            console.write_byte(b'\n');
            let s = str::from_utf8(&line_buf[..n]).unwrap();
            return Command::parse(s, args_buf);
        }

        if n == max - 1 || !b.is_ascii() {
            console.write_byte(0x7);
            continue;
        }

        console.write_byte(b);
        line_buf[n] = b;
        n += 1;
    }
}

fn echo<'a>(cmd: Command<'a>) {
    let mut iter = cmd.args[1..].iter();
    if let Some(arg) = iter.next() {
        kprint!("{}", arg);
        for arg in iter {
            kprint!(" {}", arg);
        }
        kprintln!();
    }
}

fn cd<'a>(cmd: Command<'a>, cwd: &PathBuf) -> Option<PathBuf> {
    let mut dir = "/";

    match cmd.args[1..] {
        [d] => dir = d,
        [] => (),
        _ => {
            kprintln!("Usage: cd [directory]");
            return None;
        }
    }

    let path = absolute_path(dir, &cwd);
    let res = FILESYSTEM.open(&path).and_then(|entry| {
        entry
            .into_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "not a dir"))
    });

    match res {
        Err(e) => {
            kprintln!("Error: {}", e);
            None
        }
        Ok(_) => Some(path),
    }
}

fn ls<'a>(cmd: Command<'a>, cwd: &PathBuf) {
    let mut hidden = false;
    let mut dir = ".";

    match cmd.args[1..] {
        [] => {}
        ["-a"] => {
            hidden = true;
        }
        [d] => {
            dir = d;
        }
        ["-a", d] => {
            hidden = true;
            dir = d;
        }
        _ => {
            kprintln!("Usage: ls [-a] [directory]");
            return;
        }
    };

    // TODO: make errors better by using "dir" in the messages?
    let path = absolute_path(dir, cwd);
    let res = FILESYSTEM
        .open(&path)
        .and_then(|entry| {
            entry
                .into_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "not a dir"))
        })
        .and_then(|d| d.entries())
        .and_then(|entries| {
            for e in entries.filter(|e| hidden || e.metadata().hidden() == hidden) {
                kprintln!("{}", e);
            }
            Ok(())
        });

    if let Err(e) = res {
        kprintln!("Error: {}", e);
    }
}

fn cat<'a>(cmd: Command<'a>, cwd: &PathBuf) {
    fn cat_file(f: &str, path: PathBuf) {
        let mut buf: [u8; 512] = [0; 512];
        let mut console = CONSOLE.lock();

        let res = FILESYSTEM.open(&path).and_then(|entry| {
            entry
                .into_file()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "not a file"))
        });

        let n = 0;
        match res {
            Err(e) => { kprintln!("{}: {}", f, e); },
            Ok(mut file) => loop {
                match file.read(&mut buf) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(ref e) => { kprintln!("{}: {}", f, e); break; },
                    Ok(n) => {
                        if n == 0 {
                            break;
                        } else {
                            for b in &buf[..n] {
                                if *b == b'\n' {
                                    // LF CR will go to LF LF CR but that's fine I guess since
                                    // multiple line-feeds are no issue
                                    console.write_byte(b'\r');
                                }
                                console.write_byte(*b);
                            }
                        }
                    }
                }
            }
        }
    }

    if cmd.args.len() == 1 {
        kprintln!("Usage: cat file [...]");
        return;
    }

    let mut iter = cmd.args[1..].iter();
    if let Some(f) = iter.next() {
        cat_file(f, pathbuf_from(cwd).join(f));
        for f in iter {
            kprintln!("");
            cat_file(f, pathbuf_from(cwd).join(f));
        }
    }
}

/// Starts a shell using `prefix` as the prefix for each line. This function
/// never returns.
pub fn shell(prefix: &str) -> ! {
    let mut cwd = PathBuf::from("/");

    loop {
        kprint!("{} ", prefix);
        let mut line_buf: [u8; 512] = [0; 512];
        let empty: &str = &"";
        let mut args_buf: [&str; 64] = [empty; 64];

        match read_and_parse_line(&mut line_buf, &mut args_buf) {
            Err(Error::Empty) => (),
            Err(Error::TooManyArgs) => kprintln!("error: too many arguments"),
            Ok(cmd) => match cmd.path() {
                "echo" => echo(cmd),
                "cd" => {
                    if let Some(c) = cd(cmd, &cwd) {
                        cwd = c;
                    }
                }
                "ls" => ls(cmd, &cwd),
                "cat" => cat(cmd, &cwd),
                "pwd" => kprintln!("{}", cwd.display()),
                _ => kprintln!("unknown command: {}", cmd.path()),
            },
        }
    }
}

fn absolute_path<P: AsRef<Path>>(arg: &str, cwd: P) -> PathBuf {
    let cwd_path = cwd.as_ref();
    match arg {
        "." => pathbuf_from(cwd_path),
        ".." => pathbuf_from(cwd_path.parent().unwrap_or(cwd_path)),
        _ => {
            let path = Path::new(arg);
            if path.is_absolute() {
                pathbuf_from(path)
            } else {
                pathbuf_from(cwd).join(path)
            }
        }
    }
}

fn pathbuf_from<P: AsRef<Path>>(path: P) -> PathBuf {
    let mut p = PathBuf::new();
    p.push(path.as_ref());
    p
}
