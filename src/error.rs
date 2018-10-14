/*
Most errors in ToS are unrecoverable - graphics call failed, game logic
invariant broken - and will leave the system in some variety of corrupt state,
so there's nothing more reasonable to do than panic, maybe with a toplevel
panic handler for crash reporting.

`or_die` lets fallible blocks use the convenient `?` syntax instead of
peppering `unwrap` everywhere, and forces the type to be `failure::Error` for
niceties like stack traces.

Those very few places where an operation can fail without leaving broken state
can define a `Fail` type and return a `Result` directly.
*/

use failure;

pub type Result<T> = ::std::result::Result<T, failure::Error>;

pub fn or_die<T, F: FnOnce() -> Result<T>>(f: F) -> T {
    f().unwrap()
}

// Types for the type checker.
pub fn into_error<T: Into<failure::Error>>(e: T) -> failure::Error { e.into() }