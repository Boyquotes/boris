use std::cmp::min;

use crate::{common::FractalNoise, BlockType, Terrain};
use bevy::ecs::system::ResMut;

pub fn setup_terrain(mut terrain: ResMut<Terrain>) {
    let seed = 3;
    let mut height = FractalNoise::new(seed, 0.01, 8);
    let mut caverns = FractalNoise::new(seed + 1, 0.01, 4);
    let mut caves = FractalNoise::new(seed + 1, 0.02, 3);

    let top = terrain.world_size_y() - 1;
    let mountain_height = min(top - 4, 49);
    let magma_level = 3;
    let dirt_depth = 3;
    let cavern_depth = 0.35;

    for chunk_idx in 0..terrain.chunk_count {
        terrain.init_chunk(chunk_idx);
    }

    println!("generating world..");

    for x in 0..terrain.world_size_x() {
        for y in 0..terrain.world_size_y() {
            for z in 0..terrain.world_size_z() {
                let x_f32 = x as f32;
                let y_f32 = y as f32;
                let z_f32 = z as f32;
                let h = height.get_2d(x_f32, z_f32);

                let surface = top - (((h.clamp(0.1, 0.5)) * (mountain_height) as f32) as u32); // 0 to 28

                // above ground
                if y > surface {
                    terrain.init_block(x, y, z, BlockType::EMPTY);
                    if y == surface + 1 {
                        terrain.add_sunlight(x, y, z, 15);
                    } else {
                        terrain.set_sunlight(x, y, z, 15);
                    }
                    continue;
                }

                if y <= magma_level {
                    terrain.init_block(x, y, z, BlockType::MAGMA);
                    continue;
                }

                // below ground
                let c = caverns.get_3d(x_f32, y_f32, z_f32);

                let c_depth = cavern_depth * terrain.world_size_y() as f32;
                let depth = ((c_depth - (y + 6) as f32) / c_depth).abs();

                if c > depth {
                    let cave = caves.get_3d(x_f32, y_f32, z_f32);
                    if cave < 0.5 {
                        terrain.init_block(x, y, z, BlockType::EMPTY);
                        continue;
                    }
                }

                if y == surface {
                    terrain.init_block(x, y, z, BlockType::GRASS);
                } else if y > surface - dirt_depth {
                    terrain.init_block(x, y, z, BlockType::DIRT);
                } else {
                    terrain.init_block(x, y, z, BlockType::STONE);
                }
            }
        }
    }

    println!("..done generating world");
}
