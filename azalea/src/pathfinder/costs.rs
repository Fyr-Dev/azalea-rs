use std::sync::LazyLock;

use num_traits::Float;

// based on https://github.com/cabaletta/baritone/blob/1.20.1/src/api/java/baritone/api/pathing/movement/ActionCosts.java
pub const WALK_ONE_BLOCK_COST: f32 = 20. / 4.317; // 4.633
pub const SPRINT_ONE_BLOCK_COST: f32 = 20. / 5.612; // 3.564
pub const WALK_OFF_BLOCK_COST: f32 = WALK_ONE_BLOCK_COST * 0.8;
pub const SPRINT_MULTIPLIER: f32 = SPRINT_ONE_BLOCK_COST / WALK_ONE_BLOCK_COST;
pub const JUMP_PENALTY: f32 = 2.;
pub const CENTER_AFTER_FALL_COST: f32 = WALK_ONE_BLOCK_COST - WALK_OFF_BLOCK_COST; // 0.927

// explanation here:
// https://github.com/cabaletta/baritone/blob/f147519a5c291015d4f18c94558a3f1bdcdb9588/src/api/java/baritone/api/Settings.java#L405
// it's basically just the heuristic multiplier
pub const COST_HEURISTIC: f32 = 3.563;

// this one is also from baritone, it's helpful as a tiebreaker to avoid
// breaking blocks if it can be avoided
pub const BLOCK_BREAK_ADDITIONAL_PENALTY: f32 = 2.;

// Water-related movement costs
// Based on Minecraft's actual swimming mechanics (1.97 m/s swimming vs 4.32 m/s sprinting)
// Optimized for efficient water traversal rather than avoidance
pub const WATER_WALK_COST: f32 = WALK_ONE_BLOCK_COST * 1.1; // Walking in shallow water
pub const SWIMMING_COST: f32 = WALK_ONE_BLOCK_COST * 1.8; // Regular swimming (more realistic ratio)
pub const WATER_ASCENT_COST: f32 = SWIMMING_COST * 1.3; // Swimming upward
pub const WATER_DESCENT_COST: f32 = SWIMMING_COST * 0.9; // Swimming downward is easier
pub const SPRINT_SWIMMING_COST: f32 = WALK_ONE_BLOCK_COST * 1.5; // Sprint swimming underwater
pub const FLOW_RESISTANCE_COST: f32 = SWIMMING_COST * 0.2; // Reduced resistance penalty
pub const WATER_ENTRY_COST: f32 = 2.0; // Lower entry cost to encourage water use
pub const WATER_EXIT_COST: f32 = 1.5; // Lower exit cost
pub const AIR_DEPLETION_PENALTY: f32 = 10.0; // Heavy penalty for running out of air
pub const DROWNING_AVOIDANCE_COST: f32 = 50.0; // Very high cost to prevent drowning

pub static FALL_1_25_BLOCKS_COST: LazyLock<f32> = LazyLock::new(|| distance_to_ticks(1.25));
pub static FALL_0_25_BLOCKS_COST: LazyLock<f32> = LazyLock::new(|| distance_to_ticks(0.25));
pub static JUMP_ONE_BLOCK_COST: LazyLock<f32> =
    LazyLock::new(|| *FALL_1_25_BLOCKS_COST - *FALL_0_25_BLOCKS_COST); // 3.163

pub static FALL_N_BLOCKS_COST: LazyLock<[f32; 4097]> = LazyLock::new(|| {
    let mut fall_n_blocks_cost = [0.; 4097];

    let mut distance = 0;

    // this is the same as calling distance_to_ticks a bunch of times but more
    // efficient
    let mut temp_distance = distance as f32;
    let mut tick_count = 0;
    loop {
        let fall_distance = velocity(tick_count);
        if temp_distance <= fall_distance {
            fall_n_blocks_cost[distance] = tick_count as f32 + temp_distance / fall_distance;
            distance += 1;
            if distance >= fall_n_blocks_cost.len() {
                break;
            }
        }
        temp_distance -= fall_distance;
        tick_count += 1;
    }

    fall_n_blocks_cost
});

fn velocity(ticks: usize) -> f32 {
    (0.98.powi(ticks.try_into().unwrap()) - 1.) * -3.92
}

fn distance_to_ticks(distance: f32) -> f32 {
    if distance == 0. {
        // Avoid 0/0 NaN
        return 0.;
    }
    let mut temp_distance = distance;
    let mut tick_count = 0;
    loop {
        let fall_distance = velocity(tick_count);
        if temp_distance <= fall_distance {
            return tick_count as f32 + temp_distance / fall_distance;
        }
        temp_distance -= fall_distance;
        tick_count += 1;
    }
}
