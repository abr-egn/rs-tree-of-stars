use std::fmt::Debug;

use ggez::{
    GameResult, GameError,
};
use specs::prelude::*;

pub fn try_get<'a, 'b, T: Component>(storage: &'b ReadStorage<'a, T>, ent: Entity) -> GameResult<&'b T> {
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