use ggez::{
    event,
    Context,
};
use specs::prelude::*;

use draw::ModeText;

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
    fn name(&self) -> &str;
    fn on_push(&mut self, _world: &mut World, _ctx: &mut Context) { }
    fn on_pop(&mut self, _world: &mut World, _ctx: &mut Context) { }
    fn on_show(&mut self, _world: &mut World, _ctx: &mut Context) { }
    fn on_hide(&mut self, _world: &mut World, _ctx: &mut Context) { }
    fn on_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> EventAction {
        EventAction::Continue
    }
    fn on_top_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> TopAction {
        TopAction::AsEvent
    }
}

pub struct Stack(Vec<Box<Mode>>);

impl Stack {
    pub fn new() -> Self { Stack(vec![]) }
    fn top_mut(&mut self) -> Option<&mut Box<Mode>> {
        if self.0.is_empty() { return None }
        let ix = self.0.len()-1;
        Some(&mut self.0[ix])
    }
    pub fn push(&mut self, world: &mut World, ctx: &mut Context, mut mode: Box<Mode>) {
        self.top_mut().map(|mode| mode.on_hide(world, ctx));
        mode.on_push(world, ctx);
        mode.on_show(world, ctx);
        world.write_resource::<ModeText>().set(mode.name());
        self.0.push(mode);
    }
    pub fn pop(&mut self, world: &mut World, ctx: &mut Context) -> bool {
        match self.0.pop() {
            None => false,
            Some(mut m) => {
                m.on_hide(world, ctx);
                m.on_pop(world, ctx);
                match self.top_mut() {
                    Some(mode) => {
                        mode.on_show(world, ctx);
                        world.write_resource::<ModeText>().set(mode.name());
                    },
                    None => world.write_resource::<ModeText>().set("<<none>>"),
                }
                true
            }
        }
    }
    pub fn handle(&mut self, world: &mut World, ctx: &mut Context, event: event::Event) {
        let len = self.0.len();
        if len == 0 { return }
        let ea = match self.0[len-1].on_top_event(world, ctx, event.clone()) {
            TopAction::Do(ea) => ea,
            TopAction::AsEvent => self.0[len-1].on_event(world, ctx, event.clone()),
            TopAction::Pop => { self.pop(world, ctx); return },
            TopAction::Swap(act) => {
                self.pop(world, ctx);
                self.push(world, ctx, act);
                return
            },
        };
        match ea {
            EventAction::Continue => (),
            EventAction::Done => return,
            EventAction::Push(act) => { self.push(world, ctx, act); return },
        }
        if len < 2 { return }
        let mut ix = len-2;
        loop {
            match self.0[ix].on_event(world, ctx, event.clone()) {
                EventAction::Continue => (),
                EventAction::Done => return,
                EventAction::Push(act) => { self.push(world, ctx, act); return },
            }
            if ix == 0 { break }
            ix = ix - 1;
        }
    }
}