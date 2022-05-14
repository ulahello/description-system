use lib::action::Action;
use lib::context::Context;
use lib::input;

use std::io::{self, Error};
use std::process;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("fatal: {}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<(), Error> {
    let mut out = io::stdout();
    let mut ctx = Context::spawn(io::stdout());

    ctx.act(Action::Describe)?;

    loop {
        let actions = ctx.available_actions();
        let action = input::menu(&mut out, &actions)?;

        if ctx.act(*action)? {
            break;
        }
    }

    Ok(())
}
