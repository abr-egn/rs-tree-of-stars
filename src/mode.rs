use ggez::{
    event,
    Context,
};
use specs::prelude::*;

pub enum EventAction {
    Continue,
    Done,
    Pop,
    Push(Box<Mode>),
}

pub trait Mode {
    fn on_start(&mut self, _world: &mut World, _ctx: &mut Context) { }
    fn on_stop(&mut self, _world: &mut World, _ctx: &mut Context) { }
    fn on_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> EventAction {
        EventAction::Continue
    }
}

pub struct Stack(Vec<Box<Mode>>);

impl Stack {
    pub fn new() -> Self { Stack(vec![]) }
    pub fn push(&mut self, world: &mut World, ctx: &mut Context, mut mode: Box<Mode>) {
        mode.on_start(world, ctx);
        self.0.push(mode)
    }
    pub fn pop(&mut self, world: &mut World, ctx: &mut Context) -> bool {
        match self.0.pop() {
            None => false,
            Some(mut m) => { m.on_stop(world, ctx); true }
        }
    }
    pub fn handle(&mut self, world: &mut World, ctx: &mut Context, event: event::Event) {
        let len = self.0.len();
        if len == 0 { return }
        match self.0[len-1].on_event(world, ctx, event.clone()) {
            EventAction::Continue => (),
            EventAction::Done => return,
            EventAction::Pop => { self.pop(world, ctx); return },
            EventAction::Push(act) => { self.push(world, ctx, act); return },
        }
        if len < 2 { return }
        let mut ix = len-2;
        loop {
            match self.0[ix].on_event(world, ctx, event.clone()) {
                EventAction::Continue => (),
                EventAction::Done => return,
                EventAction::Pop => panic!("out-of-order pop"),
                EventAction::Push(act) => { self.push(world, ctx, act); return },
            }
            if ix == 0 { break }
            ix = ix - 1;
        }
    }
}