use stack_vec::StackVec;
use core::str;
use core::fmt::Write;

use crate::console::{kprint, kprintln, CONSOLE};

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


fn read_and_parse_line<'a>(line_buf: &'a mut [u8], args_buf: &'a mut[&'a str]) -> Result<Command<'a>, Error> {
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

fn exec_command<'a>(cmd: Command<'a>) {
    if cmd.path() == "echo" {
        for (i, c) in cmd.args[1..].iter().enumerate() {
            kprint!("{}", c);
            if i < cmd.args.len() - 2 {
                kprint!(" ");
            } else {
                kprintln!();
            }
        }
    } else {
        kprintln!("unknown command: {}", cmd.path());
    }
}

/// Starts a shell using `prefix` as the prefix for each line. This function
/// returns if the `exit` command is called.
pub fn shell(prefix: &str) -> ! {
    loop {
        kprint!("{} ", prefix);
        let mut line_buf: [u8; 512] = [0; 512];
        let empty: &str = &"";
        let mut args_buf: [&str; 64] = [empty; 64];

        match read_and_parse_line(&mut line_buf, &mut args_buf) {
            Err(Error::Empty) => (),
            Err(Error::TooManyArgs) => kprintln!("error: too many arguments"),
            Ok(cmd) => exec_command(cmd)
        }
    }
}
