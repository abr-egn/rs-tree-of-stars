use failure;
use specs::prelude::*;

pub type Result<T> = ::std::result::Result<T, failure::Error>;

/*
pub trait SystemErr<'a> {

}

impl<'a, T: SystemErr<'a>> System<'a> for T {

}
*/