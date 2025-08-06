use azalea_block::BlockState;
use azalea_client::WalkDirection;
use azalea_core::direction::CardinalDirection;

use super::{Edge, ExecuteCtx, MoveData, PathfinderCtx, default_is_reached};
use crate::pathfinder::{astar, costs::*, rel_block_pos::RelBlockPos};

/// Types of water navigation scenarios
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaterType {
    /// Still water that's safe to navigate
    StillWater,
    /// Flowing water that should generally be avoided
    FlowingWater,
    /// Waterlogged blocks (like waterlogged stairs)
    Waterlogged,
    /// Dangerous water (near lava, etc.)
    Dangerous,
}

/// Analyze a block state to determine if it's water and what type
pub fn classify_water(block_state: BlockState) -> Option<WaterType> {
    let registry_block = azalea_registry::Block::from(block_state);
    
    // Direct water blocks
    if registry_block == azalea_registry::Block::Water {
        // Check if it's a source block (level 0) or flowing
        match block_state.property::<azalea_block::properties::WaterLevel>() {
            Some(azalea_block::properties::WaterLevel::_0) => Some(WaterType::StillWater),
            Some(_) => Some(WaterType::FlowingWater),
            None => Some(WaterType::StillWater), // Default to still if no level property
        }
    } else if block_state
        .property::<azalea_block::properties::Waterlogged>()
        .unwrap_or_default()
    {
        // Waterlogged blocks
        Some(WaterType::Waterlogged)
    } else if registry_block == azalea_registry::Block::Seagrass 
        || registry_block == azalea_registry::Block::TallSeagrass
        || registry_block == azalea_registry::Block::Kelp
        || registry_block == azalea_registry::Block::KelpPlant {
        // Seagrass and similar water-containing blocks
        Some(WaterType::StillWater)
    } else {
        None
    }
}

/// Check if a water block is safe to navigate through
pub fn is_water_navigable(water_type: WaterType) -> bool {
    match water_type {
        WaterType::StillWater | WaterType::Waterlogged => true,
        WaterType::FlowingWater => false, // For now, avoid flowing water
        WaterType::Dangerous => false,
    }
}

/// Check if there are dangerous blocks adjacent to this water position
pub fn is_water_safe(ctx: &PathfinderCtx, pos: RelBlockPos) -> bool {
    // Check for lava adjacent to water
    for dir in CardinalDirection::iter() {
        let adjacent_pos = pos + RelBlockPos::new(dir.x(), 0, dir.z());
        let block_state = ctx.world.get_block_state(adjacent_pos);
        let registry_block = azalea_registry::Block::from(block_state);
        
        if registry_block == azalea_registry::Block::Lava {
            return false;
        }
    }
    
    // Check above and below for lava
    let up_block = ctx.world.get_block_state(pos.up(1));
    let down_block = ctx.world.get_block_state(pos.down(1));
    
    if azalea_registry::Block::from(up_block) == azalea_registry::Block::Lava
        || azalea_registry::Block::from(down_block) == azalea_registry::Block::Lava {
        return false;
    }
    
    true
}

/// Add water traversal moves to the pathfinding context
pub fn water_moves(ctx: &mut PathfinderCtx, node: RelBlockPos) {
    water_traverse_move(ctx, node);
    water_ascend_move(ctx, node);
    water_descend_move(ctx, node);
}

/// Horizontal movement through water
fn water_traverse_move(ctx: &mut PathfinderCtx, pos: RelBlockPos) {
    // Check if current position is in water
    let current_block = ctx.world.get_block_state(pos);
    let current_water = classify_water(current_block);
    
    if current_water.is_none() {
        return;
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), 0, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Only traverse if target is also water and both are navigable
        if let Some(target_water_type) = target_water {
            if !is_water_navigable(target_water_type) {
                continue;
            }
        } else {
            // Target is not water, skip for now (could be water exit later)
            continue;
        }
        
        if !is_water_safe(ctx, target_pos) {
            continue;
        }
        
        // Check if path above is clear (need space to swim)
        let above_target = ctx.world.get_block_state(target_pos.up(1));
        if !crate::pathfinder::world::is_block_state_passable(above_target) 
            && classify_water(above_target).is_none() {
            continue;
        }
        
        let mut cost = SWIMMING_COST;
        
        // Add flow resistance if moving against current
        if let Some(WaterType::FlowingWater) = target_water {
            cost += FLOW_RESISTANCE_COST;
        }
        
        ctx.edges.push(Edge {
            movement: astar::Movement {
                target: target_pos,
                data: MoveData {
                    execute: &execute_water_traverse,
                    is_reached: &default_is_reached,
                },
            },
            cost,
        });
    }
}

/// Swimming upward in water
fn water_ascend_move(ctx: &mut PathfinderCtx, pos: RelBlockPos) {
    let current_block = ctx.world.get_block_state(pos);
    if classify_water(current_block).is_none() {
        return;
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), 1, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Target must be water or passable
        if let Some(target_water_type) = target_water {
            if !is_water_navigable(target_water_type) {
                continue;
            }
        } else if !crate::pathfinder::world::is_block_state_passable(target_block) {
            continue;
        }
        
        if !is_water_safe(ctx, target_pos) {
            continue;
        }
        
        let mut cost = WATER_ASCENT_COST;
        
        // Add extra cost if exiting water to land
        if target_water.is_none() {
            cost += WATER_EXIT_COST;
        }
        
        ctx.edges.push(Edge {
            movement: astar::Movement {
                target: target_pos,
                data: MoveData {
                    execute: &execute_water_ascend,
                    is_reached: &default_is_reached,
                },
            },
            cost,
        });
    }
}

/// Swimming downward in water
fn water_descend_move(ctx: &mut PathfinderCtx, pos: RelBlockPos) {
    let current_block = ctx.world.get_block_state(pos);
    if classify_water(current_block).is_none() {
        return;
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), -1, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Target must be water
        if let Some(target_water_type) = target_water {
            if !is_water_navigable(target_water_type) {
                continue;
            }
        } else {
            continue;
        }
        
        if !is_water_safe(ctx, target_pos) {
            continue;
        }
        
        let cost = WATER_DESCENT_COST;
        
        ctx.edges.push(Edge {
            movement: astar::Movement {
                target: target_pos,
                data: MoveData {
                    execute: &execute_water_descend,
                    is_reached: &default_is_reached,
                },
            },
            cost,
        });
    }
}

/// Execute horizontal water traversal
fn execute_water_traverse(mut ctx: ExecuteCtx) {
    let center = ctx.target.center();
    
    ctx.look_at(center);
    ctx.jump(); // Swimming motion in water
    ctx.walk(WalkDirection::Forward);
}

/// Execute swimming upward
fn execute_water_ascend(mut ctx: ExecuteCtx) {
    let center = ctx.target.center();
    
    ctx.look_at(center + azalea_core::position::Vec3::new(0.0, 0.5, 0.0)); // Look slightly upward
    ctx.jump(); // Jump to swim upward
    ctx.walk(WalkDirection::Forward);
}

/// Execute swimming downward  
fn execute_water_descend(mut ctx: ExecuteCtx) {
    let center = ctx.target.center();
    
    ctx.look_at(center + azalea_core::position::Vec3::new(0.0, -0.5, 0.0)); // Look slightly downward
    ctx.walk(WalkDirection::Forward); // Move forward while looking down
}
