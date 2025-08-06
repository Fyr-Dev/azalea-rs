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
    // Standard water movement
    water_traverse_move(ctx, node);
    water_ascend_move(ctx, node);
    water_descend_move(ctx, node);
    
    // Water entry from land
    water_entry_moves(ctx, node);
}

/// Horizontal movement through water
fn water_traverse_move(ctx: &mut PathfinderCtx, pos: RelBlockPos) {
    // Check if current position is in water
    let current_block = ctx.world.get_block_state(pos);
    let current_water = classify_water(current_block);
    
    if current_water.is_none() {
        return;
    }
    
    // Create swimming state for this path node
    let mut swimming_state = SwimmingState::default();
    
    // TODO: In a real implementation, we'd track swimming state through the pathfinding
    // For now, we'll estimate based on the local water environment
    let current_above = ctx.world.get_block_state(pos.up(1));
    if classify_water(current_above).is_some() {
        swimming_state.consecutive_swim_moves = 4; // Assume we've been swimming
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), 0, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Handle different target types
        match target_water {
            Some(target_water_type) => {
                if !is_water_navigable(target_water_type) {
                    continue;
                }
            }
            None => {
                // Target is not water - this could be a water exit move
                if !crate::pathfinder::world::is_block_state_passable(target_block) {
                    continue; // Solid block, can't move there
                }
                // Allow moving from water to air (water exit)
            }
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
        
        // Calculate cost based on movement type and swimming state
        let mut cost = if target_water.is_some() {
            // Water to water movement
            calculate_swimming_cost(ctx, pos, target_pos, swimming_state)
        } else {
            // Water to air movement (exit water)
            WATER_EXIT_COST
        };
        
        // Add flow resistance if moving against current
        if let Some(WaterType::FlowingWater) = target_water {
            // Only apply resistance if actually moving against the flow
            // TODO: Implement proper flow direction checking
            cost += FLOW_RESISTANCE_COST;
        }
        
        // Reduce cost if we have good air access nearby
        if has_nearby_air_access(ctx, target_pos, 3) {
            cost *= 0.9; // 10% reduction for having air access
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
    
    // Swimming state for ascent
    let mut swimming_state = SwimmingState::default();
    let current_above = ctx.world.get_block_state(pos.up(1));
    if classify_water(current_above).is_some() {
        swimming_state.consecutive_swim_moves = 2; // Moderate swimming state
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), 1, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Target must be water or passable (air)
        match target_water {
            Some(target_water_type) => {
                if !is_water_navigable(target_water_type) {
                    continue;
                }
            }
            None => {
                if !crate::pathfinder::world::is_block_state_passable(target_block) {
                    continue;
                }
                // Allow swimming up to air (good for surfacing)
            }
        }
        
        if !is_water_safe(ctx, target_pos) {
            continue;
        }
        
        // Calculate ascent cost
        let mut cost = if target_water.is_some() {
            // Swimming up in water
            let base_cost = calculate_swimming_cost(ctx, pos, target_pos, swimming_state);
            base_cost * 1.3 // Ascent multiplier from costs.rs ratio
        } else {
            // Swimming up to surface - very good for air access
            SWIMMING_COST * 0.8 // Encourage surfacing
        };
        
        // Prioritize moves that lead to air access
        if target_block.is_air() || has_nearby_air_access(ctx, target_pos, 2) {
            cost *= 0.7; // Strong incentive to reach air
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
    
    // Swimming state for descent
    let mut swimming_state = SwimmingState::default();
    let current_above = ctx.world.get_block_state(pos.up(1));
    if classify_water(current_above).is_some() {
        swimming_state.consecutive_swim_moves = 3; // Assume deeper swimming
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), -1, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Target must be water for descending
        if let Some(target_water_type) = target_water {
            if !is_water_navigable(target_water_type) {
                continue;
            }
        } else {
            continue; // Can't descend into non-water
        }
        
        if !is_water_safe(ctx, target_pos) {
            continue;
        }
        
        // Calculate descent cost with air consideration
        let base_cost = calculate_swimming_cost(ctx, pos, target_pos, swimming_state);
        let mut cost = base_cost * 0.9; // Descent multiplier from costs.rs
        
        // Penalize going deeper if air is getting low
        if swimming_state.estimated_air < 100 {
            cost *= 1.5; // Discourage going deeper when air is low
        }
        
        // Penalize moves that take us further from air access
        if !has_nearby_air_access(ctx, target_pos, 4) {
            cost *= 1.2; // Prefer staying near air access
        }
        
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

/// Water entry moves - entering water from land
pub fn water_entry_moves(ctx: &mut PathfinderCtx, pos: RelBlockPos) {
    let current_block = ctx.world.get_block_state(pos);
    
    // Only add entry moves if we're not already in water
    if classify_water(current_block).is_some() {
        return;
    }
    
    for dir in CardinalDirection::iter() {
        let offset = RelBlockPos::new(dir.x(), 0, dir.z());
        let target_pos = pos + offset;
        
        let target_block = ctx.world.get_block_state(target_pos);
        let target_water = classify_water(target_block);
        
        // Target must be navigable water
        if let Some(target_water_type) = target_water {
            if !is_water_navigable(target_water_type) {
                continue;
            }
        } else {
            continue; // Not water
        }
        
        if !is_water_safe(ctx, target_pos) {
            continue;
        }
        
        // Entry cost is now much lower to encourage water use
        let cost = WATER_ENTRY_COST;
        
        ctx.edges.push(Edge {
            movement: astar::Movement {
                target: target_pos,
                data: MoveData {
                    execute: &execute_water_entry,
                    is_reached: &default_is_reached,
                },
            },
            cost,
        });
    }
}

/// Swimming state tracking for consecutive underwater moves
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwimmingState {
    /// Number of consecutive underwater moves
    pub consecutive_swim_moves: u32,
    /// Estimated air supply remaining (ticks)
    pub estimated_air: i32,
    /// Whether the bot is in a sprint swimming state
    pub is_sprint_swimming: bool,
}

impl Default for SwimmingState {
    fn default() -> Self {
        Self {
            consecutive_swim_moves: 0,
            estimated_air: 300, // Default max air in Minecraft (20 seconds * 15 ticks/second)
            is_sprint_swimming: false,
        }
    }
}

/// Calculate the optimal swimming cost based on state and conditions
pub fn calculate_swimming_cost(
    ctx: &PathfinderCtx,
    current_pos: RelBlockPos,
    target_pos: RelBlockPos,
    swimming_state: SwimmingState,
) -> f32 {
    let mut base_cost = SWIMMING_COST;
    
    // Check if both positions are fully underwater (submerged)
    let current_above = ctx.world.get_block_state(current_pos.up(1));
    let target_above = ctx.world.get_block_state(target_pos.up(1));
    let current_submerged = classify_water(current_above).is_some();
    let target_submerged = classify_water(target_above).is_some();
    
    // Sprint swimming when fully submerged for consecutive moves
    if current_submerged && target_submerged && swimming_state.consecutive_swim_moves >= 3 {
        base_cost = SPRINT_SWIMMING_COST; // Much more efficient underwater
    }
    
    // Air supply penalty - gets exponentially worse as air runs low
    let air_ratio = swimming_state.estimated_air as f32 / 300.0; // Normalize to 0-1
    if air_ratio < 0.3 {
        // Below 30% air, start adding heavy penalties
        let air_penalty = AIR_DEPLETION_PENALTY * (1.0 - air_ratio).powi(2);
        base_cost += air_penalty;
    }
    
    // Critical air level - avoid drowning at all costs
    if swimming_state.estimated_air <= 20 {
        base_cost += DROWNING_AVOIDANCE_COST;
    }
    
    base_cost
}

/// Check if there's an air pocket (surface) within reasonable distance
pub fn has_nearby_air_access(ctx: &PathfinderCtx, pos: RelBlockPos, max_distance: i32) -> bool {
    // Check upward for air access
    for y_offset in 1..=max_distance {
        let check_pos = pos.up(y_offset);
        let block_state = ctx.world.get_block_state(check_pos);
        
        // Found air or a block with air above
        if block_state.is_air() {
            return true;
        }
        
        // If we hit a solid block, no air access this way
        if !crate::pathfinder::world::is_block_state_passable(block_state) 
            && classify_water(block_state).is_none() {
            break;
        }
    }
    
    false
}

/// Estimate air consumption for a move (in ticks)
pub fn estimate_air_consumption(current_pos: RelBlockPos, target_pos: RelBlockPos, ctx: &PathfinderCtx) -> i32 {
    let current_above = ctx.world.get_block_state(current_pos.up(1));
    let target_above = ctx.world.get_block_state(target_pos.up(1));
    
    // If either position has air above, no air consumption
    if current_above.is_air() || target_above.is_air() {
        return 0;
    }
    
    // Both positions are underwater, consume air
    // Roughly 1 air per tick when swimming underwater
    1
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

/// Execute water entry from land
fn execute_water_entry(mut ctx: ExecuteCtx) {
    let center = ctx.target.center();
    
    ctx.look_at(center);
    ctx.walk(WalkDirection::Forward); // Walk into water
}
