use ggez::{
    event,
    Context,
};
use imgui::Ui;
use specs::prelude::*;

use crate::draw::ModeText;

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
    pub fn push<M: Mode + 'static>(m: M) -> Self { TopAction::Do(EventAction::push(m)) }
    pub fn swap<M: Mode + 'static>(m: M) -> Self { TopAction::Swap(Box::new(m)) }
}

pub enum EventAction {
    Continue,
    Done,
    Push(Box<Mode>),
}

impl EventAction {
    pub fn push<M: Mode + 'static>(m: M) -> Self { EventAction::Push(Box::new(m)) }
}

pub trait Mode {
    fn name(&self) -> &str;
    fn on_push(&mut self, _world: &mut World) { }
    fn on_pop(&mut self, _world: &mut World) { }
    fn on_show(&mut self, _world: &mut World) { }
    fn on_hide(&mut self, _world: &mut World) { }
    fn on_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> EventAction {
        EventAction::Continue
    }
    fn on_top_event(&mut self, _world: &mut World, _ctx: &mut Context, _event: event::Event) -> TopAction {
        TopAction::AsEvent
    }
    fn on_ui(&mut self, _world: &mut World, _ui: &Ui) -> EventAction {
        EventAction::Continue
    }
    fn on_top_ui(&mut self, _world: &mut World, _ui: &Ui) -> TopAction {
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
    pub fn push(&mut self, world: &mut World, mut mode: Box<Mode>) {
        self.top_mut().map(|mode| mode.on_hide(world));
        mode.on_push(world);
        mode.on_show(world);
        world.write_resource::<ModeText>().set(mode.name());
        self.0.push(mode);
    }
    pub fn pop(&mut self, world: &mut World) -> bool {
        match self.0.pop() {
            None => false,
            Some(mut m) => {
                m.on_hide(world);
                m.on_pop(world);
                match self.top_mut() {
                    Some(mode) => {
                        mode.on_show(world);
                        world.write_resource::<ModeText>().set(mode.name());
                    },
                    None => world.write_resource::<ModeText>().set("<<none>>"),
                }
                true
            }
        }
    }
    pub fn handle_event(&mut self, world: &mut World, ctx: &mut Context, event: event::Event) {
        self.apply(
            world, ctx,
            |mode, world, ctx| { mode.on_top_event(world, ctx, event.clone()) },
            |mode, world, ctx| { mode.on_event(world, ctx, event.clone()) },
        )
    }
    pub fn handle_ui(&mut self, world: &mut World, mut ui: &Ui) {
        self.apply(
            world, &mut ui,
            |mode, world, ui| { mode.on_top_ui(world, ui) },
            |mode, world, ui| { mode.on_ui(world, ui) },
        )
    }
    fn apply<Top, Stack, State>(
        &mut self, world: &mut World, state: &mut State,
        on_top: Top, mut on_stack: Stack,
    )   where Top: FnOnce(&mut Box<Mode>, &mut World, &mut State) -> TopAction,
              Stack: FnMut(&mut Box<Mode>, &mut World, &mut State) -> EventAction,
    {
        let len = self.0.len();
        if len == 0 { return }
        let ea = match on_top(&mut self.0[len-1], world, state) {
            TopAction::Do(ea) => ea,
            TopAction::AsEvent => on_stack(&mut self.0[len-1], world, state),
            TopAction::Pop => { self.pop(world); return },
            TopAction::Swap(act) => {
                self.pop(world);
                self.push(world, act);
                return
            },
        };
        match ea {
            EventAction::Continue => (),
            EventAction::Done => return,
            EventAction::Push(act) => { self.push(world, act); return },
        }
        if len < 2 { return }
        let mut ix = len-2;
        loop {
            match on_stack(&mut self.0[ix], world, state) {
                EventAction::Continue => (),
                EventAction::Done => return,
                EventAction::Push(act) => { self.push(world, act); return },
            }
            if ix == 0 { break }
            ix = ix - 1;
        }

    }
}