use ggez::{
    event,
    Context, GameResult,
};

#[derive(Debug)]
pub enum Fallthrough {
    Continue,
    Stop,
}

pub trait Mode {
    fn update(&mut self, _ctx: &mut Context) -> GameResult<Fallthrough> {
        Ok(Fallthrough::Continue)
    }

    fn draw(&mut self, _ctx: &mut Context) -> GameResult<Fallthrough> {
        Ok(Fallthrough::Continue)
    }
}

pub struct Stack(Vec<Box<Mode>>);

impl Stack {
    pub fn new() -> Self { Stack(vec![]) }
    pub fn push(&mut self, mode: impl Mode + 'static) { self.0.push(Box::new(mode)) }
    pub fn pop(&mut self) -> bool { self.0.pop().is_some() }
}

impl event::EventHandler for Stack {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        for mode in self.0.iter_mut().rev() {
            match mode.update(ctx)? {
                Fallthrough::Stop => break,
                _ => (),
            }
        }
        Ok(())
    }
    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        for mode in self.0.iter_mut().rev() {
            match mode.draw(ctx)? {
                Fallthrough::Stop => break,
                _ => (),
            }
        }
        Ok(())
    }
}