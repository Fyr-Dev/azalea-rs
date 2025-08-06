use azalea_core::position::BlockPos;
use azalea_world::{ChunkStorage, PartialInstance};
use azalea_registry::Block;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::pathfinder::{
    world::CachedWorld,
    moves::water::{classify_water, WaterType},
};

#[test]
fn test_water_classification() {
    // Test still water
    let still_water = Block::Water.into();
    assert_eq!(classify_water(still_water), Some(WaterType::StillWater));
    
    // Test air (not water)
    let air = azalea_block::BlockState::AIR;
    assert_eq!(classify_water(air), None);
}

#[test]
fn test_water_pathfinding_passable() {
    let mut partial_world = PartialInstance::default();
    let mut world = ChunkStorage::default();

    // Set up a simple water area
    partial_world
        .chunks
        .set(&azalea_core::position::ChunkPos { x: 0, z: 0 }, Some(azalea_world::Chunk::default()), &mut world);
    
    // Place water blocks
    partial_world.chunks.set_block_state(
        BlockPos::new(0, 0, 0),
        Block::Water.into(),
        &world,
    );
    partial_world.chunks.set_block_state(
        BlockPos::new(0, 1, 0),
        Block::Water.into(),
        &world,
    );

    let cached_world = CachedWorld::new(Arc::new(RwLock::new(world.into())), BlockPos::default());
    
    // Water should now be passable
    assert!(cached_world.is_block_pos_passable(BlockPos::new(0, 0, 0)));
    assert!(cached_world.is_block_pos_passable(BlockPos::new(0, 1, 0)));
    
    // Should be able to move through water
    assert!(cached_world.is_passable_at_block_pos(BlockPos::new(0, 0, 0)));
}

#[test]
fn test_water_standable() {
    let mut partial_world = PartialInstance::default();
    let mut world = ChunkStorage::default();

    // Set up water environment
    partial_world
        .chunks
        .set(&azalea_core::position::ChunkPos { x: 0, z: 0 }, Some(azalea_world::Chunk::default()), &mut world);
    
    // Place water with solid bottom
    partial_world.chunks.set_block_state(
        BlockPos::new(0, 0, 0),
        Block::Stone.into(),
        &world,
    );
    partial_world.chunks.set_block_state(
        BlockPos::new(0, 1, 0),
        Block::Water.into(),
        &world,
    );
    partial_world.chunks.set_block_state(
        BlockPos::new(0, 2, 0),
        Block::Water.into(),
        &world,
    );

    let cached_world = CachedWorld::new(Arc::new(RwLock::new(world.into())), BlockPos::default());
    
    // Should be able to "stand" (swim) in water
    assert!(cached_world.is_standable_at_block_pos(BlockPos::new(0, 1, 0)));
    assert!(cached_world.is_standable_at_block_pos(BlockPos::new(0, 2, 0)));
}
