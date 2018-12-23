/*
Most errors in ToS are unrecoverable - graphics call failed, game logic
invariant broken - and will leave the system in some variety of corrupt state,
so there's nothing more reasonable to do than panic, maybe with a toplevel
panic handler for crash reporting.

Those very few places where an operation can fail without leaving broken state
get their own Error enum.
*/

use std::sync::mpsc;

use ggez;
use specs;

#[derive(Debug)]
pub enum Error {
    NoPath,
    NoSuchComponent,
    NoSuchEdge,
    Occupied,
    PathIxOverflow,
    PoolUnderflow,
    PullChannel,
    StorageOverflow,
    #[allow(unused)]  // TODO
    WrongEdge,
    Ggez(ggez::GameError),
    Specs(specs::error::Error),
    SpecsGen(specs::error::WrongGeneration),
    Channel(mpsc::SendError<bool>),
}

pub type Result<T> = ::std::result::Result<T, Error>;

// Let fallible blocks use the convenient `?` syntax instead of
// peppering `unwrap` everywhere, and coalesce the error type.
pub fn or_die<T, F: FnOnce() -> Result<T>>(f: F) -> T {
    f().unwrap()
}

impl From<ggez::GameError> for Error {
    fn from(err: ggez::GameError) -> Self { Error::Ggez(err) }
}

impl From<specs::error::Error> for Error {
    fn from(err: specs::error::Error) -> Self { Error::Specs(err) }
}

impl From<specs::error::WrongGeneration> for Error {
    fn from(err: specs::error::WrongGeneration) -> Self { Error::SpecsGen(err) }
}

impl From<mpsc::SendError<bool>> for Error {
    fn from(err: mpsc::SendError<bool>) -> Self { Error::Channel(err) }
}