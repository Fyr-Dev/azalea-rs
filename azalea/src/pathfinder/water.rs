use azalea_block::BlockState;
use azalea_core::position::BlockPos;
use azalea_registry::Block;

use crate::pathfinder::world::CachedWorld;

/// Water traversal cost modifier based on Baritone's approach
pub const WATER_WALK_COST: f32 = 2.0;
pub const FLOWING_WATER_COST: f32 = 5.0;
pub const DEEP_WATER_SWIM_COST: f32 = 3.0;

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

/// Calculate water traversal cost based on water type and depth
pub fn calculate_water_cost(world: &CachedWorld, pos: BlockPos) -> f32 {
    let block = world.get_block_state_at_pos(pos);
    
    if !is_traversable_water(block) {
        return 0.0;
    }
    
    if is_still_water(block) {
        // Check if we can stand in this water (shallow)
        let below = world.get_block_state_at_pos(pos.down(1));
        if world.is_block_pos_standable(pos.down(1)) && !is_traversable_water(below) {
            WATER_WALK_COST
        } else {
            DEEP_WATER_SWIM_COST
        }
    } else {
        // Flowing water is more expensive
        FLOWING_WATER_COST
    }
}

/// Check if position is in water and determine movement type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterMovementType {
    None,           // Not in water
    WalkThrough,    // Walking through shallow water
    Swimming,       // Swimming in deep water
    Floating,       // Floating on surface
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

/// Check if we should avoid this water position
pub fn should_avoid_water(world: &CachedWorld, pos: BlockPos) -> bool {
    let block = world.get_block_state_at_pos(pos);
    
    // Avoid flowing water that could push us off course
    if is_flowing_water(block) {
        // Check if flowing water would push us into danger
        // For now, just avoid all flowing water near edges
        return true;
    }
    
    // Check for dangerous drops under water
    let mut check_pos = pos.down(1);
    let mut water_depth = 0;
    
    // Count water depth
    while is_traversable_water(world.get_block_state_at_pos(check_pos)) && water_depth < 10 {
        water_depth += 1;
        check_pos = check_pos.down(1);
    }
    
    // If water is very deep and we can't see the bottom, might be risky
    if water_depth >= 10 {
        return true;
    }
    
    false
}
