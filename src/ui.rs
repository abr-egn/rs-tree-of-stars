use imgui::Ui;
use specs::prelude::*;

use game;
use graph;

pub fn draw(world: &mut World, ui: &Ui) {
    SelectedUi(ui).run_now(&mut world.res);
}

struct SelectedUi<'a, 'b: 'a>(&'a Ui<'b>);

impl<'a, 'b, 'c> System<'a> for SelectedUi<'b, 'c> {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, game::Selected>,
        ReadStorage<'a, graph::Link>,
        ReadStorage<'a, graph::Node>,
    );

    fn run(&mut self, (entities, selected, links, nodes): Self::SystemData) {
        let ui = self.0;
        for (entity, _) in (&*entities, &selected).join() {
            ui.window(im_str!("Selected")).build(|| {
                let kind = if links.get(entity).is_some() {
                    "link"
                } else if nodes.get(entity).is_some() {
                    "node"
                } else { "unknown" };
                ui.text(format!("Type: {}", kind));
            });
        }
    }
}