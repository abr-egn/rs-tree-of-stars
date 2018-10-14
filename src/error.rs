use failure;
use shred::DynamicSystemData;
use specs::{
    prelude::*,
    storage::GenericReadStorage,
};

use util::try_get;

pub type Result<T> = ::std::result::Result<T, failure::Error>;

pub trait SystemErr<'a> {
    type SystemData: DynamicSystemData<'a>;

    fn run(&mut self, data: Self::SystemData) -> Result<()>;
}

pub struct SE<T>(pub T);

impl<'a, T: SystemErr<'a>> System<'a> for SE<T> {
    type SystemData = <T as SystemErr<'a>>::SystemData;

    fn run(&mut self, data: Self::SystemData) {
        self.0.run(data).unwrap();
    }
}

pub trait LazyExt {
    fn exec_mut_err<F>(&self, f: F)
        where F: FnOnce(&mut World) -> Result<()> + 'static + Send + Sync;
}

impl LazyExt for LazyUpdate {
    fn exec_mut_err<F>(&self, f: F)
        where F: FnOnce(&mut World) -> Result<()> + 'static + Send + Sync
    {
        self.exec_mut(move |world| { f(world).unwrap() })
    }
}

// Types for the type checker.
pub fn into_error<T: Into<failure::Error>>(e: T) -> failure::Error { e.into() }

pub fn get_or_die<S, T>(storage: &S, ent: Entity) -> &T
    where S: GenericReadStorage<Component=T>,
          T: Component,
{ try_get(storage, ent).unwrap() }