use failure;
use shred::DynamicSystemData;
use specs::prelude::*;

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