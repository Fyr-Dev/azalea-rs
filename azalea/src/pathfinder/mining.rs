use std::{cell::UnsafeCell, ops::RangeInclusive, collections::HashMap, time::Instant};

use azalea_block::{
    BlockState, BlockStates, block_state::BlockStateIntegerRepr, properties::Waterlogged,
};
use azalea_inventory::Menu;
use azalea_core::position::BlockPos;
use nohash_hasher::IntMap;

use super::costs::BLOCK_BREAK_ADDITIONAL_PENALTY;
use crate::auto_tool::best_tool_in_hotbar_for_block;

pub struct MiningCache {
    block_state_id_costs: UnsafeCell<IntMap<BlockStateIntegerRepr, f32>>,
    inventory_menu: Option<Menu>,

    water_block_state_range: RangeInclusive<BlockStateIntegerRepr>,
    lava_block_state_range: RangeInclusive<BlockStateIntegerRepr>,

    falling_blocks: Vec<BlockState>,
    
    // Enhanced caching for mining optimization
    preferred_tools: UnsafeCell<IntMap<BlockStateIntegerRepr, usize>>,
    mining_sequences: HashMap<BlockState, MiningSequence>,
    avoid_blocks: HashMap<BlockPos, Instant>, // Blocks to avoid due to previous failures
}

#[derive(Debug, Clone)]
pub struct MiningSequence {
    pub blocks: Vec<BlockPos>,
    pub estimated_time: f32,
    pub tool_switches: Vec<usize>,
}

impl MiningCache {
    pub fn new(inventory_menu: Option<Menu>) -> Self {
        let water_block_states = BlockStates::from(azalea_registry::Block::Water);
        let lava_block_states = BlockStates::from(azalea_registry::Block::Lava);

        let mut water_block_state_range_min = BlockStateIntegerRepr::MAX;
        let mut water_block_state_range_max = BlockStateIntegerRepr::MIN;
        for state in water_block_states {
            water_block_state_range_min = water_block_state_range_min.min(state.id());
            water_block_state_range_max = water_block_state_range_max.max(state.id());
        }
        let water_block_state_range = water_block_state_range_min..=water_block_state_range_max;

        let mut lava_block_state_range_min = BlockStateIntegerRepr::MAX;
        let mut lava_block_state_range_max = BlockStateIntegerRepr::MIN;
        for state in lava_block_states {
            lava_block_state_range_min = lava_block_state_range_min.min(state.id());
            lava_block_state_range_max = lava_block_state_range_max.max(state.id());
        }
        let lava_block_state_range = lava_block_state_range_min..=lava_block_state_range_max;

        let mut falling_blocks: Vec<BlockState> = vec![
            azalea_registry::Block::Sand.into(),
            azalea_registry::Block::RedSand.into(),
            azalea_registry::Block::Gravel.into(),
            azalea_registry::Block::Anvil.into(),
            azalea_registry::Block::ChippedAnvil.into(),
            azalea_registry::Block::DamagedAnvil.into(),
            // concrete powders
            azalea_registry::Block::WhiteConcretePowder.into(),
            azalea_registry::Block::OrangeConcretePowder.into(),
            azalea_registry::Block::MagentaConcretePowder.into(),
            azalea_registry::Block::LightBlueConcretePowder.into(),
            azalea_registry::Block::YellowConcretePowder.into(),
            azalea_registry::Block::LimeConcretePowder.into(),
            azalea_registry::Block::PinkConcretePowder.into(),
            azalea_registry::Block::GrayConcretePowder.into(),
            azalea_registry::Block::LightGrayConcretePowder.into(),
            azalea_registry::Block::CyanConcretePowder.into(),
            azalea_registry::Block::PurpleConcretePowder.into(),
            azalea_registry::Block::BlueConcretePowder.into(),
            azalea_registry::Block::BrownConcretePowder.into(),
            azalea_registry::Block::GreenConcretePowder.into(),
            azalea_registry::Block::RedConcretePowder.into(),
            azalea_registry::Block::BlackConcretePowder.into(),
        ];
        falling_blocks.sort_unstable_by_key(|block| block.id());

        Self {
            block_state_id_costs: UnsafeCell::new(IntMap::default()),
            inventory_menu,
            water_block_state_range,
            lava_block_state_range,
            falling_blocks,
            preferred_tools: UnsafeCell::new(IntMap::default()),
            mining_sequences: HashMap::new(),
            avoid_blocks: HashMap::new(),
        }
    }

    pub fn cost_for(&self, block: BlockState) -> f32 {
        let Some(inventory_menu) = &self.inventory_menu else {
            return f32::INFINITY;
        };

        // SAFETY: mining is single-threaded, so this is safe
        let block_state_id_costs = unsafe { &mut *self.block_state_id_costs.get() };

        if let Some(cost) = block_state_id_costs.get(&block.id()) {
            *cost
        } else {
            let best_tool_result = best_tool_in_hotbar_for_block(block, inventory_menu);
            let mut cost = 1. / best_tool_result.percentage_per_tick;

            cost += BLOCK_BREAK_ADDITIONAL_PENALTY;

            block_state_id_costs.insert(block.id(), cost);
            
            // Cache the preferred tool for this block
            let preferred_tools = unsafe { &mut *self.preferred_tools.get() };
            preferred_tools.insert(block.id(), best_tool_result.index);
            
            cost
        }
    }

    /// Get the preferred tool index for a block (cached)
    pub fn preferred_tool_for(&self, block: BlockState) -> Option<usize> {
        let preferred_tools = unsafe { &*self.preferred_tools.get() };
        preferred_tools.get(&block.id()).copied()
    }

    /// Calculate the cost of mining a sequence of blocks with optimal tool switching
    pub fn sequence_cost(&mut self, blocks: &[BlockPos], world: &impl BlockStateProvider) -> f32 {
        let mut total_cost = 0.0;
        let mut current_tool: Option<usize> = None;
        
        for &pos in blocks {
            let block_state = world.get_block_state(pos);
            let block_cost = self.cost_for(block_state);
            
            if block_cost == f32::INFINITY {
                return f32::INFINITY;
            }
            
            let preferred_tool = self.preferred_tool_for(block_state);
            
            // Add tool switch cost if needed
            if let Some(tool) = preferred_tool {
                if current_tool != Some(tool) {
                    total_cost += 1.0; // Tool switch penalty
                    current_tool = Some(tool);
                }
            }
            
            total_cost += block_cost;
        }
        
        total_cost
    }

    /// Mark a block position as temporarily inaccessible
    pub fn mark_block_inaccessible(&mut self, pos: BlockPos, duration_seconds: u64) {
        let avoid_until = Instant::now() + std::time::Duration::from_secs(duration_seconds);
        self.avoid_blocks.insert(pos, avoid_until);
    }

    /// Check if a block should be avoided due to previous failures
    pub fn should_avoid_block(&self, pos: BlockPos) -> bool {
        if let Some(avoid_until) = self.avoid_blocks.get(&pos) {
            Instant::now() < *avoid_until
        } else {
            false
        }
    }

    /// Clean up expired avoid entries
    pub fn cleanup_avoid_list(&mut self) {
        let now = Instant::now();
        self.avoid_blocks.retain(|_, avoid_until| now < *avoid_until);
    }

    pub fn is_liquid(&self, block: BlockState) -> bool {
        // this already runs in about 1 nanosecond, so if you wanna try optimizing it at
        // least run the benchmarks (in benches/checks.rs)

        self.water_block_state_range.contains(&block.id())
            || self.lava_block_state_range.contains(&block.id())
            || is_waterlogged(block)
    }

    pub fn is_falling_block(&self, block: BlockState) -> bool {
        self.falling_blocks
            .binary_search_by_key(&block.id(), |block| block.id())
            .is_ok()
    }
}

pub fn is_waterlogged(block: BlockState) -> bool {
    block.property::<Waterlogged>().unwrap_or_default()
}

/// Trait for providing block states from world data
pub trait BlockStateProvider {
    fn get_block_state(&self, pos: BlockPos) -> BlockState;
}
