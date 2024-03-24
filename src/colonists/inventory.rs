use bevy::ecs::{component::Component, entity::Entity};

#[derive(Component, Default)]
pub struct Inventory {
    pub items: Vec<Entity>,
}

#[derive(Component)]
pub struct Item {
    pub tags: Vec<ItemTag>,
}

#[derive(Component)]
pub struct InInventory {
    pub holder: Entity,
}

#[derive(Clone, PartialEq)]
pub enum ItemTag {
    PickAxe,
}

pub fn test_item_flags(all: &Vec<ItemTag>, test: &Vec<ItemTag>) -> bool {
    test.iter().all(|tag| all.contains(tag))
}
