use std::{
    fmt::Debug,
    time::Duration,
};

use ggez::{
    GameResult, GameError,
};
use specs::{
    prelude::*,
    storage::GenericReadStorage,
};

pub fn try_get<S, T>(storage: &S, ent: Entity) -> GameResult<&T>
    where S: GenericReadStorage<Component=T>,
          T: Component,
{
    match storage.get(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such component".into())),
    }
}

pub fn try_get_mut<'a, 'b, T: Component>(storage: &'b mut WriteStorage<'a, T>, ent: Entity) -> GameResult<&'b mut T> {
    match storage.get_mut(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such component".into())),
    }
}

pub fn dbg<T: Debug>(t: T) -> String { format!("{:?}", t) }

pub fn duration_f32(dt: Duration) -> f32 {
    (dt.as_secs() as f32) + ((dt.subsec_micros() as f32) * 1e-6)
}

pub fn f32_duration(ft: f32) -> Duration {
    Duration::from_nanos((ft * 1e-9) as u64)
}