use bevy::{
    ecs::system::{Res, ResMut, Resource},
    gizmos::gizmos::Gizmos,
    math::Vec3,
    render::color::Color,
    utils::{HashMap, HashSet},
};
use ndshape::AbstractShape;

use crate::{common::flood_fill, Terrain};

pub struct Partition {
    id: u16,
    pub neighbors: HashSet<u16>,
    pub is_computed: bool,
    pub chunk_idx: u32,
    pub blocks: Vec<u32>,
}

impl Partition {
    pub const NONE: u16 = 0;

    pub fn add_block(&mut self, block_idx: u32) {
        self.blocks.push(block_idx);
    }
}

#[derive(Resource, Default)]
pub struct PartitionDebug {
    pub id: u16,
    pub show: bool,
}

pub fn partition_debug(
    terrain: Res<Terrain>,
    graph: Res<PartitionGraph>,
    debug: Res<PartitionDebug>,
    mut gizmos: Gizmos,
) {
    if !debug.show {
        return;
    }

    if let Some(partition) = graph.partitions.get(&debug.id) {
        for block_idx in partition.blocks.iter() {
            let [x, y, z] = terrain.get_block_world_pos(partition.chunk_idx, *block_idx);
            let pos = Vec3::new(x as f32, y as f32 + 0.1, z as f32);

            gizmos.line(pos, pos + Vec3::new(1., 0., 0.), Color::GRAY);
            gizmos.line(pos, pos + Vec3::new(0., 0., 1.), Color::GRAY);

            gizmos.line(pos, pos + Vec3::new(1., 0., 0.), Color::GRAY);
            gizmos.line(pos, pos + Vec3::new(0., 0., 1.), Color::GRAY);

            gizmos.line(
                pos + Vec3::new(1., 0., 1.),
                pos + Vec3::new(1., 0., 0.),
                Color::GRAY,
            );
            gizmos.line(
                pos + Vec3::new(1., 0., 1.),
                pos + Vec3::new(0., 0., 1.),
                Color::GRAY,
            );
        }
    }
}

#[derive(Resource, Default)]
pub struct PartitionGraph {
    pub partitions: HashMap<u16, Partition>,
    cur_id: u16,
}

impl PartitionGraph {
    pub fn create_partition(&mut self, chunk_idx: u32) -> u16 {
        self.cur_id += 1;
        let p = Partition {
            id: self.cur_id,
            chunk_idx,
            neighbors: HashSet::new(),
            is_computed: false,
            blocks: vec![],
        };

        self.partitions.insert(p.id, p);

        self.cur_id
    }

    pub fn is_partition_computed(&self, id: u16) -> bool {
        if let Some(p) = self.partitions.get(&id) {
            return p.is_computed;
        }
        false
    }

    pub fn set_partition_computed(&mut self, id: u16, value: bool) {
        if let Some(p) = self.partitions.get_mut(&id) {
            p.is_computed = value;
        }
    }

    pub fn set_block(&mut self, partition_id: u16, block_idx: u32) {
        if let Some(p) = self.partitions.get_mut(&partition_id) {
            p.add_block(block_idx);
        }
    }

    pub fn set_neighbors(&mut self, a_id: u16, b_id: u16) {
        let a = self.partitions.get_mut(&a_id).unwrap();
        a.neighbors.insert(b_id);

        let b = self.partitions.get_mut(&b_id).unwrap();
        b.neighbors.insert(a_id);
    }
}

pub fn partition(mut terrain: ResMut<Terrain>, mut graph: ResMut<PartitionGraph>) {
    println!("partitioning world..");

    for chunk_idx in 0..terrain.chunk_count {
        println!("partitioning chunk {}", chunk_idx);
        for block_idx in 0..terrain.chunk_shape.size() {
            let block = terrain.get_block_by_idx(chunk_idx, block_idx);

            let p_id = terrain.get_partition(chunk_idx, block_idx);

            if p_id == Partition::NONE {
                // lets check if the block is navigable.
                // a block can be navigated if it is empty,
                // the block above it is empty, and the block
                // below it is filled.
                let is_empty = block.is_empty();

                if !is_empty {
                    continue;
                }

                let [x, y, z] = terrain.get_block_world_pos(chunk_idx, block_idx);

                let block_above = terrain.get_block(x, y + 1, z);

                if !block_above.is_empty() {
                    continue;
                }

                let block_below = terrain.get_block(x, y - 1, z);

                if !block_below.is_filled() {
                    continue;
                }

                // if we are here, that means the block is navigable,
                // and it is not assigned to a partition yet. We must
                // create a new partition and assign it
                let new_partition_id = graph.create_partition(chunk_idx);
                terrain.set_partition(chunk_idx, block_idx, new_partition_id);
                graph.set_block(new_partition_id, block_idx);
                println!("created new partition {}", new_partition_id);
            };

            let partition_id = terrain.get_partition(chunk_idx, block_idx);

            // if the block is already in a computed partition, it has
            // already been claimed and we can skip it.
            if graph.is_partition_computed(partition_id) {
                continue;
            }

            let [x, y, z] = terrain.get_block_world_pos(chunk_idx, block_idx);

            // next, flood fill from the block, looking for other
            // navigable blocks to add to the current partition
            flood_fill([x as i32, y as i32, z as i32], |[nx, ny, nz]| {
                println!("flooding {} {} {}", nx, ny, nz);
                if terrain.is_oob(nx, ny, nz) {
                    return false;
                }

                let [nchunk_idx, nblock_idx] =
                    terrain.get_block_indexes(nx as u32, ny as u32, nz as u32);

                // todo: can the whole block before this be removed, and just done as part
                // of the normal routine?
                if nchunk_idx == chunk_idx && nblock_idx == block_idx {
                    return true;
                }

                let npartition_id = terrain.get_partition(nchunk_idx, nblock_idx);

                // have we already visited this block?
                if npartition_id == partition_id {
                    return false;
                }

                let nblock = terrain.get_block_by_idx(nchunk_idx, nblock_idx);

                if !nblock.is_empty() {
                    return false;
                }

                let nblock_above = terrain.get_block_i32(nx, ny + 1, nz);

                if !nblock_above.is_empty() {
                    return false;
                }

                let nblock_below = terrain.get_block_i32(nx, ny - 1, nz);
                if !nblock_below.is_filled() {
                    return false;
                }

                // if the block belongs to a different chunk, we must check if
                // it already has a partition. if not, create a new non-computed
                // partition for it. We add this partition as a neighbor.
                if nchunk_idx != chunk_idx {
                    if npartition_id != Partition::NONE {
                        // a partition already exists, add it as a neighbor
                        graph.set_neighbors(partition_id, npartition_id);
                    } else {
                        // a partition does not exist, create it, and add it as
                        // a neighbor
                        let npartition_id = graph.create_partition(nchunk_idx);
                        graph.set_neighbors(partition_id, npartition_id);
                        terrain.set_partition(nchunk_idx, nblock_idx, npartition_id);
                        graph.set_block(npartition_id, nblock_idx);
                    }

                    // we do not create partitions across chunk boundaries
                    return false;
                }

                // this block is navigable, and in the same chunk, so we assign it
                // to the same partition and continue flooding.
                terrain.set_partition(nchunk_idx, nblock_idx, partition_id);
                graph.set_block(partition_id, nblock_idx);

                println!(
                    "set partition for block {} {} {}",
                    nchunk_idx, nblock_idx, partition_id
                );
                true
            });

            // we have flooded the partition, we mark it as computed
            graph.set_partition_computed(partition_id, true);
        }
    }
    println!("..done partitioning world");
}
