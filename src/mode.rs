use ggez::{
    event,
    Context,
};
use specs::prelude::*;

use error::Result;

pub enum TopAction {
    Do(EventAction),
    AsEvent,
    Pop,
    Swap(Box<Mode>),
}

impl TopAction {
    #[allow(unused)]
    pub fn continue_() -> Self { TopAction::Do(EventAction::Continue) }
    pub fn done() -> Self { TopAction::Do(EventAction::Done) }
    pub fn push(m: Box<Mode>) -> Self { TopAction::Do(EventAction::Push(m)) }
}

pub enum EventAction {
    Continue,
    Done,
    Push(Box<Mode>),
}

pub trait Mode {
    fn on_start(&mut self, _world: &mut World, _ctx: &mut Context) -> Result<()> { Ok(()) }
    fn on_stop(&mut self, _world: &mut World, _ctx: &mut Context) -> Result<()> { Ok(()) }    
    fn on_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> Result<EventAction> {
        Ok(EventAction::Continue)
    }
    fn on_top_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> Result<TopAction> {
        Ok(TopAction::AsEvent)
    }
}

pub struct Stack(Vec<Box<Mode>>);

impl Stack {
    pub fn new() -> Self { Stack(vec![]) }
    pub fn push(&mut self, world: &mut World, ctx: &mut Context, mut mode: Box<Mode>) -> Result<()> {
        mode.on_start(world, ctx)?;
        self.0.push(mode);
        Ok(())
    }
    pub fn pop(&mut self, world: &mut World, ctx: &mut Context) -> Result<bool> {
        match self.0.pop() {
            None => Ok(false),
            Some(mut m) => { m.on_stop(world, ctx)?; Ok(true) }
        }
    }
    pub fn handle(&mut self, world: &mut World, ctx: &mut Context, event: event::Event) -> Result<()> {
        let len = self.0.len();
        if len == 0 { return Ok(()) }
        let ea = match self.0[len-1].on_top_event(world, ctx, event.clone())? {
            TopAction::Do(ea) => ea,
            TopAction::AsEvent => self.0[len-1].on_event(world, ctx, event.clone())?,
            TopAction::Pop => { self.pop(world, ctx)?; return Ok(()) },
            TopAction::Swap(act) => {
                self.pop(world, ctx)?;
                self.push(world, ctx, act)?;
                return Ok(())
            },
        };
        match ea {
            EventAction::Continue => (),
            EventAction::Done => return Ok(()),
            EventAction::Push(act) => { self.push(world, ctx, act)?; return Ok(()) },
        }
        if len < 2 { return Ok(()) }
        let mut ix = len-2;
        loop {
            match self.0[ix].on_event(world, ctx, event.clone())? {
                EventAction::Continue => (),
                EventAction::Done => return Ok(()),
                EventAction::Push(act) => { self.push(world, ctx, act)?; return Ok(()) },
            }
            if ix == 0 { break }
            ix = ix - 1;
        }
        Ok(())
    }
}