use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        event::EventReader,
        query::{With, Without},
        system::{Commands, Query, Res, ResMut},
    },
    gizmos::gizmos::Gizmos,
    math::{vec3, Vec3},
    render::color::Color,
    time::Time,
    transform::components::Transform,
};
use ordered_float::*;

use crate::{
    colonists::Partition,
    common::{astar, AStarSettings, Distance},
    Terrain,
};

use super::{get_block_flags, Colonist, PartitionFlags, PartitionGraph, PathfindEvent};

#[derive(Component)]
pub struct PathfindRequest {
    pub goals: Vec<[u32; 3]>,
    pub flags: PartitionFlags,
}

#[derive(Component)]
pub struct PathSegment {
    blocks: Vec<[i32; 3]>,
    current: usize,
    flags: PartitionFlags,
    goals: Vec<[u32; 3]>,
}

#[derive(Component)]
pub struct PartitionPath {
    path: Vec<u16>,
    goals: Vec<[u32; 3]>,
    current: usize,
    flags: PartitionFlags,
}

#[derive(Component)]
pub struct BlockMove {
    pub speed: f32,
    pub target: [i32; 3],
}

pub fn path_follow_block(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &BlockMove, &mut Transform)>,
) {
    for (entity, block_move, mut transform) in query.iter_mut() {
        let pos = vec3(
            block_move.target[0] as f32,
            block_move.target[1] as f32,
            block_move.target[2] as f32,
        );

        let direction = (pos - transform.translation).normalize();
        let distance = transform.translation.distance(pos);
        let move_dist = time.delta_seconds() * block_move.speed;

        if distance < move_dist {
            transform.translation = pos;
            commands.entity(entity).remove::<BlockMove>();
        } else {
            transform.translation += direction * move_dist;
        }
    }
}

pub fn path_follow_segment(
    terrain: ResMut<Terrain>,
    mut commands: Commands,
    mut pathers: Query<(Entity, &mut PathSegment), Without<BlockMove>>,
) {
    for (entity, mut path) in pathers.iter_mut() {
        if path.current == 0 {
            commands.entity(entity).remove::<PathSegment>();
            continue;
        }

        path.current -= 1;

        let next_block = path.blocks[path.current];

        if get_block_flags(&terrain, next_block[0], next_block[1], next_block[2]) & path.flags
            == PartitionFlags::NONE
        {
            commands.entity(entity).remove::<PathSegment>();
            commands.entity(entity).remove::<PartitionPath>();
            commands.entity(entity).insert(PathfindRequest {
                goals: path.goals.clone(),
                flags: path.flags,
            });
            return;
        }

        commands.entity(entity).insert(BlockMove {
            target: next_block,
            speed: 8.,
        });
    }
}

pub fn path_follow_segment_debug(mut gizmos: Gizmos, pathers: Query<&PathSegment>) {
    for path in pathers.iter() {
        for i in 1..path.blocks.len() {
            let current = path.blocks[i - 1];
            let next = path.blocks[i];

            let mid = Vec3::new(0.5, 0.5, 0.5);

            let color = if i > path.current {
                Color::GRAY
            } else if i == path.current {
                Color::ORANGE_RED
            } else {
                Color::ORANGE
            };

            gizmos.line(
                Vec3::new(current[0] as f32, current[1] as f32, current[2] as f32) + mid,
                Vec3::new(next[0] as f32, next[1] as f32, next[2] as f32) + mid,
                color,
            );
        }

        for g in path.goals.iter() {
            let pos = Vec3::new(g[0] as f32, g[1] as f32 + 0.04, g[2] as f32);

            gizmos.line(pos, pos + Vec3::new(1., 0., 0.), Color::CYAN);
            gizmos.line(pos, pos + Vec3::new(0., 0., 1.), Color::CYAN);

            gizmos.line(pos, pos + Vec3::new(1., 0., 0.), Color::CYAN);
            gizmos.line(pos, pos + Vec3::new(0., 0., 1.), Color::CYAN);

            gizmos.line(
                pos + Vec3::new(1., 0., 1.),
                pos + Vec3::new(1., 0., 0.),
                Color::CYAN,
            );
            gizmos.line(
                pos + Vec3::new(1., 0., 1.),
                pos + Vec3::new(0., 0., 1.),
                Color::CYAN,
            );
        }
    }
}

pub fn path_follow_partition(
    mut commands: Commands,
    graph: Res<PartitionGraph>,
    terrain: ResMut<Terrain>,
    mut pathers: Query<(Entity, &mut PartitionPath, &Transform), Without<PathSegment>>,
) {
    for (entity, mut path, transform) in pathers.iter_mut() {
        if path.current == 0 {
            println!("completed path follow!");
            commands.entity(entity).remove::<PartitionPath>();
            continue;
        }

        path.current -= 1;

        let pos = [
            transform.translation.x as i32,
            transform.translation.y as i32,
            transform.translation.z as i32,
        ];

        let is_last_partition = path.current <= 1;

        let goal_positions = if is_last_partition {
            path.current = 0;
            path.goals
                .iter()
                .map(|g| [g[0] as i32, g[1] as i32, g[2] as i32])
                .collect()
        } else {
            let idx = path.current - 1;
            let goal_partition_id = path.path[idx];
            let c = graph.get_center(goal_partition_id).unwrap();
            vec![[c[0] as i32, c[1] as i32, c[2] as i32]]
        };

        let next_partition = match is_last_partition {
            true => None,
            false => {
                let idx = path.current;
                let next_partition_id = path.path[idx];
                graph.get_partition(next_partition_id)
            }
        };

        let result = astar(AStarSettings {
            start: pos,
            is_goal: |p| {
                // assuming u32 here as we are filter oob earlier
                if is_last_partition {
                    goal_positions
                        .iter()
                        .any(|g| p[0] == g[0] && p[1] == g[1] && p[2] == g[2])
                } else {
                    let idx = path.current;
                    let next_partition_id = path.path[idx];
                    let [chunk_idx, block_idx] =
                        terrain.get_block_indexes(p[0] as u32, p[1] as u32, p[2] as u32);
                    let partition_id = terrain.get_partition_id(chunk_idx, block_idx);
                    partition_id == next_partition_id
                }
            },
            cost: |a, b| Distance::diagonal([a[0], a[1], a[2]], [b[0], b[1], b[2]]),
            heuristic: |v| {
                if is_last_partition {
                    goal_positions
                        .iter()
                        .map(|g| OrderedFloat(Distance::diagonal(v, *g)))
                        .min()
                        .unwrap()
                        .0
                } else {
                    next_partition
                        .unwrap()
                        .extents
                        .distance_to_edge(v[0], v[1], v[2])
                }
            },
            neighbors: |v| {
                // TODO: extract neighbors to block graph
                let up = [v[0], v[1] + 1, v[2]];
                let down = [v[0], v[1] - 1, v[2]];
                let left = [v[0] - 1, v[1], v[2]];
                let right = [v[0] + 1, v[1], v[2]];
                let forward = [v[0], v[1], v[2] - 1];
                let back = [v[0], v[1], v[2] + 1];

                let forward_left = [v[0] - 1, v[1], v[2] - 1];
                let forward_right = [v[0] + 1, v[1], v[2] - 1];
                let back_left = [v[0] - 1, v[1], v[2] + 1];
                let back_right = [v[0] + 1, v[1], v[2] + 1];

                let mut edges = vec![up, down, left, right, forward, back];

                let f_clear = get_block_flags(&terrain, forward[0], forward[1], forward[2])
                    & path.flags
                    != PartitionFlags::NONE;
                let r_clear = get_block_flags(&terrain, right[0], right[1], right[2]) & path.flags
                    != PartitionFlags::NONE;
                let l_clear = get_block_flags(&terrain, left[0], left[1], left[2]) & path.flags
                    != PartitionFlags::NONE;
                let b_clear = get_block_flags(&terrain, back[0], back[1], back[2]) & path.flags
                    != PartitionFlags::NONE;

                if f_clear && l_clear {
                    edges.push(forward_left);
                }
                if f_clear && r_clear {
                    edges.push(forward_right);
                }
                if b_clear && l_clear {
                    edges.push(back_left);
                }
                if b_clear && r_clear {
                    edges.push(back_right);
                }

                edges
                    .iter()
                    .filter_map(|p| {
                        let [chunk_idx, block_idx] =
                            terrain.get_block_indexes(p[0] as u32, p[1] as u32, p[2] as u32);
                        let partition_id = terrain.get_partition_id(chunk_idx, block_idx);
                        let part_flags = graph.get_flags(partition_id);

                        if part_flags & path.flags != PartitionFlags::NONE {
                            Some(*p)
                        } else {
                            None
                        }
                    })
                    .collect()
            },
            max_depth: 3000,
        });

        if !result.is_success {
            println!("no segment path found!");
            let mut cmds = commands.entity(entity);
            cmds.remove::<PartitionPath>();
            cmds.insert(PathfindRequest {
                goals: path.goals.clone(),
                flags: path.flags,
            });
            return;
        }

        if !result.is_success {
            println!("no final segment path found!");
            let mut cmds = commands.entity(entity);
            cmds.remove::<PartitionPath>();
            cmds.insert(PathfindRequest {
                goals: path.goals.clone(),
                flags: path.flags,
            });
            return;
        }

        commands.entity(entity).insert(PathSegment {
            current: result.path.len(),
            blocks: result.path,
            flags: path.flags,
            goals: path.goals.clone(),
        });
    }
}

pub fn is_reachable(
    start_id: u16,
    goal_ids: Vec<u16>,
    graph: PartitionGraph,
    flags: PartitionFlags,
) -> bool {
    if goal_ids.contains(&start_id) {
        return true;
    }

    let partition_path = astar(AStarSettings {
        start: start_id,
        is_goal: |p| goal_ids.contains(&p),
        max_depth: 2000,
        neighbors: |v| {
            if let Some(p) = graph.get_partition(v) {
                return p
                    .neighbors
                    .iter()
                    .filter(|n| graph.get_flags(**n) & flags != PartitionFlags::NONE)
                    .copied()
                    .collect();
            }
            vec![]
        },
        heuristic: |a| {
            let [ax, ay, az] = graph.get_partition(a).unwrap().extents.center();

            goal_ids
                .iter()
                .filter_map(|g_id| {
                    if let Some(g) = graph.get_center(*g_id) {
                        return Some(OrderedFloat(Distance::diagonal(
                            [ax as i32, ay as i32, az as i32],
                            [g[0] as i32, g[1] as i32, g[2] as i32],
                        )));
                    }
                    None
                })
                .min()
                .unwrap()
                .0
        },
        cost: |a, b| {
            let [ax, ay, az] = graph.get_partition(a).unwrap().extents.center();
            let [bx, by, bz] = graph.get_partition(b).unwrap().extents.center();

            Distance::diagonal(
                [ax as i32, ay as i32, az as i32],
                [bx as i32, by as i32, bz as i32],
            )
        },
    });

    partition_path.is_success
}

pub fn pathfinding(
    terrain: Res<Terrain>,
    graph: Res<PartitionGraph>,
    mut commands: Commands,
    pathfinders: Query<(Entity, &PathfindRequest, &Transform)>,
) {
    for (e, request, transform) in pathfinders.iter() {
        let start = [
            transform.translation.x as u32,
            transform.translation.y as u32,
            transform.translation.z as u32,
        ];
        println!(
            "find path {},{},{}->{},{},{}",
            start[0],
            start[1],
            start[2],
            request.goals[0][0],
            request.goals[0][1],
            request.goals[0][2]
        );

        commands.entity(e).remove::<PathfindRequest>();

        let [start_chunk_idx, start_block_idx] =
            terrain.get_block_indexes(start[0], start[1], start[2]);

        let goals: Vec<([u32; 3], u16)> = request
            .goals
            .iter()
            .map(|g| (*g, terrain.get_block_indexes(g[0], g[1], g[2])))
            .map(|(g, [g_chunk_idx, g_block_idx])| {
                (g, terrain.get_partition_id(g_chunk_idx, g_block_idx))
            })
            .filter(|(g, p_id)| *p_id != Partition::NONE)
            .collect();

        let mut goal_partition_ids: Vec<u16> = goals.iter().map(|(g, pid)| *pid).collect();
        goal_partition_ids.sort();
        goal_partition_ids.dedup();

        let starting_partition_id = terrain.get_partition_id(start_chunk_idx, start_block_idx);

        for (g, pid) in goals.iter() {
            println!("GOAL: {}, {}, {} -> {}", g[0], g[1], g[2], pid);
        }

        if starting_partition_id == Partition::NONE {
            println!("cannot find path, no starting partition!");
            commands.entity(e).remove::<PathfindRequest>();
            continue;
        }

        if goals.is_empty() {
            println!("cannot find path, no goal partition!");
            commands.entity(e).remove::<PathfindRequest>();
            continue;
        }

        if goal_partition_ids.contains(&starting_partition_id) {
            commands.entity(e).insert(PartitionPath {
                current: 1,
                path: vec![starting_partition_id],
                goals: request.goals.clone(),
                flags: request.flags,
            });

            continue;
        }

        let partition_path = astar(AStarSettings {
            start: starting_partition_id,
            is_goal: |p| goal_partition_ids.contains(&p),
            max_depth: 2000,
            neighbors: |v| {
                if let Some(p) = graph.get_partition(v) {
                    return p
                        .neighbors
                        .iter()
                        .filter(|n| graph.get_flags(**n) & request.flags != PartitionFlags::NONE)
                        .copied()
                        .collect();
                }
                vec![]
            },
            heuristic: |a| {
                let [ax, ay, az] = graph.get_partition(a).unwrap().extents.center();

                goals
                    .iter()
                    .map(|(g, _pid)| {
                        OrderedFloat(Distance::diagonal(
                            [ax as i32, ay as i32, az as i32],
                            [g[0] as i32, g[1] as i32, g[2] as i32],
                        ))
                    })
                    .min()
                    .unwrap()
                    .0
            },
            cost: |a, b| {
                let [ax, ay, az] = graph.get_partition(a).unwrap().extents.center();
                let [bx, by, bz] = graph.get_partition(b).unwrap().extents.center();

                Distance::diagonal(
                    [ax as i32, ay as i32, az as i32],
                    [bx as i32, by as i32, bz as i32],
                )
            },
        });

        if !partition_path.is_success {
            println!("could not find path");
            return;
        }

        commands.entity(e).insert(PartitionPath {
            current: partition_path.path.len() - 1, // first one is the starting position
            path: partition_path.path,
            goals: request.goals.clone(),
            flags: request.flags,
        });
    }
}
