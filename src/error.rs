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