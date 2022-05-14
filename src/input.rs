use std::fmt::Display;
use std::io::{self, Error, Write};

pub fn menu<'a, W: Write, C: Display>(w: &mut W, choices: &'a [C]) -> Result<&'a C, Error> {
    writeln!(w)?;

    loop {
        for (n, choice) in choices.iter().enumerate() {
            writeln!(w, " {}) {}", n, choice)?;
        }
        writeln!(w)?;
        w.flush()?;

        match readln(w, "? ")?.parse::<usize>() {
            Ok(index) => {
                w.flush()?;
                if let Some(chosen) = choices.get(index) {
                    writeln!(w)?;
                    return Ok(chosen);
                } else {
                    writeln!(w, "no such choice\n")?;
                }
            }
            Err(err) => writeln!(w, "{}\n", err)?,
        }
    }
}

pub fn readln(w: &mut impl Write, prompt: impl Display) -> Result<String, Error> {
    let stdin = io::stdin();
    let mut input = String::new();

    write!(w, "{}", prompt)?;
    w.flush()?;

    stdin.read_line(&mut input)?;

    Ok(input.trim().escape_default().to_string())
}
