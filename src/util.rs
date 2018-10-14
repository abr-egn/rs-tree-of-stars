use std::{
    time::Duration,
};

use specs::{
    prelude::*,
    storage::GenericReadStorage,
};

use error::Result;

#[derive(Fail, Debug)]
#[fail(display = "No such component.")]
pub struct NoSuchComponent;

pub fn try_get<S, T>(storage: &S, ent: Entity) -> Result<&T>
    where S: GenericReadStorage<Component=T>,
          T: Component,
{
    storage.get(ent).ok_or_else(|| NoSuchComponent.into())
}

pub fn try_get_mut<'a, 'b, T: Component>(storage: &'b mut WriteStorage<'a, T>, ent: Entity) -> Result<&'b mut T> {
    storage.get_mut(ent).ok_or_else(|| NoSuchComponent.into())
}

pub fn duration_f32(dt: Duration) -> f32 {
    (dt.as_secs() as f32) + ((dt.subsec_micros() as f32) * 1e-6)
}

pub fn f32_duration(ft: f32) -> Duration {
    Duration::from_nanos((ft * 1e-9) as u64)
}