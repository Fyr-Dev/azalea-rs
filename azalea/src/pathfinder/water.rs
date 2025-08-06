use azalea_block::BlockState;
use azalea_core::position::BlockPos;
use azalea_registry::Block;

use crate::pathfinder::costs::{
    WATER_WALK_COST, SWIMMING_COST, FLOW_RESISTANCE_COST, SPRINT_SWIMMING_COST,
    WATER_ASCENT_COST, WATER_DESCENT_COST, WATER_ENTRY_COST, WATER_EXIT_COST,
    AIR_DEPLETION_PENALTY, DROWNING_AVOIDANCE_COST
};
use crate::pathfinder::world::CachedWorld;

/// Determines if a block is water that can be traversed
pub fn is_traversable_water(block: BlockState) -> bool {
    let registry_block = Block::from(block);
    registry_block == Block::Water
}

/// Determines if water is still (level 0) or flowing
pub fn is_still_water(block: BlockState) -> bool {
    if !is_traversable_water(block) {
        return false;
    }
    
    // Check water level - still water has level 0
    match block.property::<azalea_block::properties::Level>() {
        Some(level) => level == azalea_block::properties::Level::_0,
        None => false,
    }
}

/// Determines if water is flowing (level > 0)
pub fn is_flowing_water(block: BlockState) -> bool {
    if !is_traversable_water(block) {
        return false;
    }
    
    !is_still_water(block)
}

/// Check if we can walk on top of water (like with frost walker boots)
pub fn can_walk_on_water(_world: &CachedWorld, _pos: BlockPos) -> bool {
    // TODO: Check for frost walker enchantment
    false
}

/// Calculate advanced water traversal cost based on comprehensive context
pub fn calculate_advanced_water_cost(
    world: &CachedWorld, 
    from_pos: BlockPos, 
    to_pos: BlockPos,
    context: &WaterTraversalContext
) -> f32 {
    let block = world.get_block_state_at_pos(to_pos);
    
    if !is_traversable_water(block) {
        // Handle water entry/exit costs
        if context.is_exiting_water {
            return WATER_EXIT_COST;
        }
        return 0.0;
    }
    
    // Base cost calculation
    let mut cost = match context.movement_type {
        WaterMovementType::None => 0.0,
        WaterMovementType::WalkThrough => WATER_WALK_COST,
        WaterMovementType::Swimming => {
            // Use sprint swimming if we've been swimming consecutively
            if context.consecutive_swim_moves >= 3 {
                SPRINT_SWIMMING_COST
            } else {
                SWIMMING_COST
            }
        },
        WaterMovementType::Floating => SWIMMING_COST,
    };
    
    // Vertical movement modifiers
    match context.vertical_direction {
        VerticalDirection::Ascending => cost = WATER_ASCENT_COST,
        VerticalDirection::Descending => cost = WATER_DESCENT_COST,
        VerticalDirection::Level => {}, // No modifier for horizontal movement
    }
    
    // Flowing water resistance
    if context.is_flowing {
        cost += FLOW_RESISTANCE_COST;
    }
    
    // Water entry cost
    if context.is_entering_water {
        cost += WATER_ENTRY_COST;
    }
    
    // Air depletion penalties
    if context.air_remaining < 0.3 {
        cost += AIR_DEPLETION_PENALTY * (1.0 - context.air_remaining);
    }
    
    // Drowning avoidance - massive penalty when air is critically low
    if context.air_remaining < 0.1 {
        cost += DROWNING_AVOIDANCE_COST;
    }
    
    // Depth-based penalties for very deep water (risk assessment)
    if context.water_depth > 15 {
        cost *= 1.2; // 20% penalty for very deep water
    }
    
    cost
}

/// Calculate water traversal cost based on water type and depth (legacy function)
pub fn calculate_water_cost(world: &CachedWorld, pos: BlockPos) -> f32 {
    let context = analyze_water_context(world, pos, pos, 0, 1.0);
    calculate_advanced_water_cost(world, pos, pos, &context)
}

/// Analyze comprehensive water traversal context
pub fn analyze_water_context(
    world: &CachedWorld,
    from_pos: BlockPos,
    to_pos: BlockPos,
    consecutive_swim_moves: u32,
    air_remaining: f32,
) -> WaterTraversalContext {
    let current_block = world.get_block_state_at_pos(to_pos);
    let from_block = world.get_block_state_at_pos(from_pos);
    
    let movement_type = get_water_movement_type(world, to_pos);
    let is_flowing = is_flowing_water(current_block);
    
    // Determine vertical direction
    let vertical_direction = if to_pos.y > from_pos.y {
        VerticalDirection::Ascending
    } else if to_pos.y < from_pos.y {
        VerticalDirection::Descending
    } else {
        VerticalDirection::Level
    };
    
    // Check if entering or exiting water
    let is_entering_water = !is_traversable_water(from_block) && is_traversable_water(current_block);
    let is_exiting_water = is_traversable_water(from_block) && !is_traversable_water(current_block);
    
    // Calculate water depth
    let water_depth = calculate_water_depth(world, to_pos);
    
    WaterTraversalContext {
        movement_type,
        is_flowing,
        vertical_direction,
        consecutive_swim_moves,
        air_remaining,
        is_entering_water,
        is_exiting_water,
        water_depth,
    }
}

/// Calculate the depth of water at a given position
pub fn calculate_water_depth(world: &CachedWorld, pos: BlockPos) -> u32 {
    let mut depth = 0;
    let mut check_pos = pos;
    
    // Count down to find the bottom
    while is_traversable_water(world.get_block_state_at_pos(check_pos)) && depth < 50 {
        depth += 1;
        check_pos = check_pos.down(1);
    }
    
    depth
}

/// Check if position is in water and determine movement type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterMovementType {
    None,           // Not in water
    WalkThrough,    // Walking through shallow water
    Swimming,       // Swimming in deep water
    Floating,       // Floating on surface
}

/// Advanced water traversal context for sophisticated pathfinding
#[derive(Debug, Clone, Copy)]
pub struct WaterTraversalContext {
    pub movement_type: WaterMovementType,
    pub is_flowing: bool,
    pub vertical_direction: VerticalDirection,
    pub consecutive_swim_moves: u32,
    pub air_remaining: f32, // 0.0 to 1.0 (1.0 = full air, 0.0 = drowning)
    pub is_entering_water: bool,
    pub is_exiting_water: bool,
    pub water_depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalDirection {
    Level,    // Moving horizontally
    Ascending, // Swimming up
    Descending, // Swimming down
}

impl Default for WaterTraversalContext {
    fn default() -> Self {
        Self {
            movement_type: WaterMovementType::None,
            is_flowing: false,
            vertical_direction: VerticalDirection::Level,
            consecutive_swim_moves: 0,
            air_remaining: 1.0,
            is_entering_water: false,
            is_exiting_water: false,
            water_depth: 0,
        }
    }
}

pub fn get_water_movement_type(world: &CachedWorld, pos: BlockPos) -> WaterMovementType {
    let current_block = world.get_block_state_at_pos(pos);
    let below_block = world.get_block_state_at_pos(pos.down(1));
    
    if !is_traversable_water(current_block) {
        return WaterMovementType::None;
    }
    
    // If we can stand on the block below and it's not water, we can walk through
    if world.is_block_pos_standable(pos.down(1)) && !is_traversable_water(below_block) {
        WaterMovementType::WalkThrough
    } else if is_traversable_water(below_block) {
        // Deep water - need to swim
        WaterMovementType::Swimming
    } else {
        // At surface level
        WaterMovementType::Floating
    }
}

/// Check if we should avoid this water position with advanced context analysis
pub fn should_avoid_water_advanced(
    world: &CachedWorld, 
    pos: BlockPos, 
    air_remaining: f32
) -> bool {
    let block = world.get_block_state_at_pos(pos);
    
    // Avoid if not enough air for safe traversal
    if air_remaining < 0.2 && is_traversable_water(block) {
        return true;
    }
    
    // Avoid flowing water that could push us into danger
    if is_flowing_water(block) {
        // Check surrounding area for dangerous drops or obstacles
        for dx in -1..=1 {
            for dz in -1..=1 {
                let check_pos = BlockPos::new(pos.x + dx, pos.y - 2, pos.z + dz);
                let check_block = world.get_block_state_at_pos(check_pos);
                if !world.is_block_pos_standable(check_pos) && !is_traversable_water(check_block) {
                    return true; // Dangerous drop nearby
                }
            }
        }
    }
    
    // Calculate water depth and avoid if too deep without enough air
    let water_depth = calculate_water_depth(world, pos);
    
    // If water is very deep and we have low air, avoid
    if water_depth > 20 && air_remaining < 0.5 {
        return true;
    }
    
    // If water is extremely deep (potential ocean), be cautious
    if water_depth >= 40 {
        return true;
    }
    
    false
}

/// Check if we should avoid this water position (legacy function)
pub fn should_avoid_water(world: &CachedWorld, pos: BlockPos) -> bool {
    should_avoid_water_advanced(world, pos, 1.0) // Assume full air for legacy calls
}

/// Estimate air consumption for a water traversal move
pub fn estimate_air_consumption(context: &WaterTraversalContext, distance: f32) -> f32 {
    match context.movement_type {
        WaterMovementType::None | WaterMovementType::WalkThrough => 0.0,
        WaterMovementType::Swimming | WaterMovementType::Floating => {
            let base_consumption = distance * 0.02; // Base air consumption per block
            
            // Increased consumption when ascending (more effort)
            let vertical_modifier = match context.vertical_direction {
                VerticalDirection::Ascending => 1.5,
                VerticalDirection::Descending => 0.8,
                VerticalDirection::Level => 1.0,
            };
            
            // Sprint swimming uses more air
            let sprint_modifier = if context.consecutive_swim_moves >= 3 { 1.3 } else { 1.0 };
            
            // Flowing water requires more effort
            let flow_modifier = if context.is_flowing { 1.4 } else { 1.0 };
            
            base_consumption * vertical_modifier * sprint_modifier * flow_modifier
        }
    }
}

/// Check if a path through water is safe given current air levels
pub fn is_water_path_safe(
    world: &CachedWorld,
    path: &[BlockPos],
    current_air: f32,
    consecutive_swim_moves: u32,
) -> bool {
    let mut air_remaining = current_air;
    let mut swim_moves = consecutive_swim_moves;
    
    for window in path.windows(2) {
        let from_pos = window[0];
        let to_pos = window[1];
        
        let context = analyze_water_context(world, from_pos, to_pos, swim_moves, air_remaining);
        
        // Calculate air consumption for this move
        let distance = ((to_pos.x - from_pos.x).pow(2) + 
                       (to_pos.y - from_pos.y).pow(2) + 
                       (to_pos.z - from_pos.z).pow(2)) as f32).sqrt();
        
        let air_consumed = estimate_air_consumption(&context, distance);
        air_remaining -= air_consumed;
        
        // Update swim move counter
        if context.movement_type == WaterMovementType::Swimming {
            swim_moves += 1;
        } else {
            swim_moves = 0;
        }
        
        // Check if we'd run out of air
        if air_remaining < 0.1 {
            return false;
        }
    }
    
    true
}
