use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        event::EventReader,
        query::With,
        system::{Commands, Query, ResMut},
    },
    transform::components::Transform,
    utils::hashbrown::HashSet,
};
use ndshape::AbstractShape;

use crate::{
    colonists::{get_block_flags, Item, PartitionEvent},
    common::flood_fill_i32,
    Terrain,
};

use super::NavigationGraph;

#[derive(Component)]
pub struct InPartition {
    pub partition_id: u32,
}

pub fn partition(
    mut cmd: Commands,
    mut partition_ev: EventReader<PartitionEvent>,
    mut graph: ResMut<NavigationGraph>,
    mut terrain: ResMut<Terrain>,
    q_items: Query<&Transform, With<Item>>,
) {
    for ev in partition_ev.read() {
        let chunk_idx = ev.chunk_idx;

        let mut items: HashSet<Entity> = HashSet::new();

        let cleanups = graph.delete_partitions_for_chunk(chunk_idx);

        for cleanup in cleanups {
            for block_cleanup_idx in cleanup.blocks.iter() {
                terrain.unset_partition_id(chunk_idx, *block_cleanup_idx);
            }
            items.extend(cleanup.items);
        }

        for block_idx in 0..terrain.chunk_shape.size() {
            let [x, y, z] = terrain.get_block_world_pos(chunk_idx, block_idx);
            let block_flags = get_block_flags(&terrain, x as i32, y as i32, z as i32);

            // ignore empty blocks
            if block_flags.is_empty() {
                continue;
            }

            // get the partition for this block. if it does not exist, create one
            let mut partition_id = terrain
                .get_partition_id(chunk_idx, block_idx)
                .unwrap_or_else(|| {
                    let new_region_id = graph.create_region(block_flags);
                    let new_partition_id =
                        graph.create_partition(new_region_id, chunk_idx, block_flags);
                    let new_partition = graph.get_partition_mut(&new_partition_id).unwrap();

                    terrain.set_partition_id(chunk_idx, block_idx, new_partition_id);
                    new_partition.assign_block(block_idx, [x, y, z]);

                    new_partition_id
                });

            {
                let partition = graph.get_partition_mut(&partition_id).unwrap();

                // if the partition is already computed, we can safely skip this
                if partition.is_computed {
                    continue;
                }
            }

            let mut region_id = graph.get_partition(&partition_id).unwrap().region_id;

            flood_fill_i32([x as i32, y as i32, z as i32], |[nx, ny, nz]| {
                if terrain.is_oob(nx, ny, nz) {
                    return false;
                }

                let [nchunk_idx, nblock_idx] =
                    terrain.get_block_indexes(nx as u32, ny as u32, nz as u32);

                // this is the seed block
                if nblock_idx == block_idx && nchunk_idx == chunk_idx {
                    return true;
                }

                if let Some(npartition_id) = terrain.get_partition_id(nchunk_idx, nblock_idx) {
                    // already assigned to this partition
                    if npartition_id == partition_id {
                        return false;
                    }

                    let nblock_flags = get_block_flags(&terrain, nx, ny, nz);

                    if nblock_flags.is_empty() {
                        return false;
                    }

                    let flag_diff = nblock_flags != block_flags;
                    let chunk_diff = nchunk_idx != chunk_idx;

                    if flag_diff || chunk_diff {
                        if let Some(new_region_id) =
                            graph.set_partition_neighbors(&partition_id, &npartition_id)
                        {
                            region_id = new_region_id;
                        };

                        return false;
                    }

                    (partition_id, region_id) =
                        graph.merge_partitions(&partition_id, &npartition_id, &mut terrain);

                    return true;
                }

                let nblock_flags = get_block_flags(&terrain, nx, ny, nz);

                if nblock_flags.is_empty() {
                    return false;
                }

                let flag_diff = nblock_flags != block_flags;
                let chunk_diff = nchunk_idx != chunk_idx;

                if flag_diff || chunk_diff {
                    // if flags are the same, we add to existing region, otherwise we make
                    // a new region and add it as a neighbor.
                    let nregion_id = if flag_diff {
                        let new_region_id = graph.create_region(nblock_flags);
                        graph.set_region_neighbors(&region_id, &new_region_id);
                        new_region_id
                    } else {
                        region_id
                    };

                    let npartition_id =
                        graph.create_partition(nregion_id, nchunk_idx, nblock_flags);

                    terrain.set_partition_id(nchunk_idx, nblock_idx, npartition_id);
                    graph.assign_block(
                        &npartition_id,
                        nblock_idx,
                        [nx as u32, ny as u32, nz as u32],
                        &mut terrain,
                    );

                    return false;
                }

                terrain.set_partition_id(nchunk_idx, nblock_idx, partition_id);
                graph.assign_block(
                    &partition_id,
                    nblock_idx,
                    [nx as u32, ny as u32, nz as u32],
                    &mut terrain,
                );

                true
            });

            let partition = graph.get_partition_mut(&partition_id).unwrap();
            partition.is_computed = true;
            partition.extents.update_traversal_distance();
        }

        for item in items {
            let Ok(transform) = q_items.get(item) else {
                println!("Item does not exist anymore. {}", item.index());
                continue;
            };

            let x = transform.translation.x as u32;
            let y = transform.translation.y as u32;
            let z = transform.translation.z as u32;

            let mut ecmd = cmd.entity(item);

            ecmd.remove::<InPartition>();

            let Some(item_partition_id) = terrain.get_partition_id_u32(x, y, z) else {
                println!("Item is not in a valid partition! Teleport it?");
                continue;
            };

            let partition = graph.get_partition_mut(&item_partition_id).unwrap();

            partition.items.insert(item);
            ecmd.insert(InPartition {
                partition_id: item_partition_id,
            });
        }
    }
}
