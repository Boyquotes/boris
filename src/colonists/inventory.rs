use std::fmt::{Display, Formatter, Result};

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        event::{Event, EventReader},
        system::{Commands, Query, ResMut},
    },
    hierarchy::DespawnRecursiveExt,
};

use super::{InPartition, NavigationGraph};

#[derive(Component, Default)]
pub struct Inventory {
    pub items: Vec<Entity>,
}

#[derive(Component)]
pub struct Item {
    pub tags: Vec<ItemTag>,
    pub reserved: Option<Entity>,
}

#[derive(Component)]
pub struct InInventory {
    pub holder: Entity,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ItemTag {
    Pickaxe,
    Stone,
}

impl Display for ItemTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:?}", self)
    }
}

pub fn test_item_tags(all: &[ItemTag], test: &[ItemTag]) -> bool {
    test.iter().all(|tag| all.contains(tag))
}

#[derive(Event)]
pub struct DestroyItemEvent {
    pub entity: Entity,
}

pub fn destroy_items(
    mut graph: ResMut<NavigationGraph>,
    mut cmd: Commands,
    q_items: Query<&InPartition>,
    mut ev_destroy_item: EventReader<DestroyItemEvent>,
) {
    for ev in ev_destroy_item.read() {
        println!("destroying item {}", ev.entity.index());
        cmd.entity(ev.entity).despawn_recursive();

        let Ok(in_partition) = q_items.get(ev.entity) else {
            continue;
        };

        let Some(partition) = graph.get_partition_mut(&in_partition.partition_id) else {
            panic!("Missing partition!? {}", in_partition.partition_id);
        };

        println!("Removing item from partition");
        if !partition.items.remove(&ev.entity) {
            println!("Item not here!");
        }
    }
}
