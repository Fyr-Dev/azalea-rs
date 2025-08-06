use azalea_core::position::BlockPos;
use crate::pathfinder::goals::Goal;
use std::collections::HashSet;

/// Goal for mining operations with intelligent positioning
#[derive(Debug, Clone)]
pub enum MiningGoal {
    /// Mine a single block - positions bot optimally for mining
    SingleBlock { 
        target: BlockPos,
        prefer_y_level: Option<i32>,  // Prefer mining from this Y level
    },
    
    /// Mine multiple blocks efficiently, optimizing for sequence
    MultipleBlocks {
        targets: Vec<BlockPos>,
        allow_internal_mining: bool,  // Allow mining from inside block clusters
    },
    
    /// Mine an ore vein by positioning to access maximum blocks
    OreVein {
        blocks: HashSet<BlockPos>,
        center: BlockPos,
        max_reach: f32,
    },
    
    /// Goal for strip mining - mine in a pattern
    StripMine {
        start: BlockPos,
        direction: StripMineDirection,
        length: u32,
        height: u32,
        width: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum StripMineDirection {
    North, South, East, West,
}

impl Goal for MiningGoal {
    fn heuristic(&self, pos: BlockPos) -> f32 {
        match self {
            MiningGoal::SingleBlock { target, prefer_y_level } => {
                let base_distance = target.distance_squared_to(pos) as f32;
                
                // Prefer positions that allow mining from the preferred Y level
                if let Some(preferred_y) = prefer_y_level {
                    let y_penalty = ((pos.y - preferred_y).abs() as f32) * 2.0;
                    base_distance + y_penalty
                } else {
                    base_distance
                }
            },
            
            MiningGoal::MultipleBlocks { targets, .. } => {
                // Find distance to closest target
                targets.iter()
                    .map(|target| target.distance_squared_to(pos) as f32)
                    .fold(f32::INFINITY, f32::min)
            },
            
            MiningGoal::OreVein { center, .. } => {
                center.distance_squared_to(pos) as f32
            },
            
            MiningGoal::StripMine { start, .. } => {
                start.distance_squared_to(pos) as f32
            },
        }
    }

    fn success(&self, pos: BlockPos) -> bool {
        match self {
            MiningGoal::SingleBlock { target, .. } => {
                // Position is reached if we can mine the target block
                self.can_mine_from_position(pos, *target)
            },
            
            MiningGoal::MultipleBlocks { targets, allow_internal_mining } => {
                if *allow_internal_mining {
                    // Can mine from inside the cluster
                    targets.iter().any(|&target| self.can_mine_from_position(pos, target))
                } else {
                    // Must be outside the cluster but able to mine at least one block
                    !targets.contains(&pos) && 
                    targets.iter().any(|&target| self.can_mine_from_position(pos, target))
                }
            },
            
            MiningGoal::OreVein { blocks, center: _, max_reach } => {
                // Position is good if we can mine multiple blocks from here
                let reachable_count = blocks.iter()
                    .filter(|&&block_pos| {
                        let distance = ((pos.x - block_pos.x).pow(2) + 
                                       (pos.y - block_pos.y).pow(2) + 
                                       (pos.z - block_pos.z).pow(2)) as f32;
                        distance.sqrt() <= *max_reach
                    })
                    .count();
                
                reachable_count >= 3 // Can mine at least 3 blocks from this position
            },
            
            MiningGoal::StripMine { start, .. } => {
                // For strip mining, we just need to reach the starting position
                pos.distance_squared_to(*start) <= 2
            },
        }
    }
}

impl MiningGoal {
    /// Check if we can mine a target block from the given position
    fn can_mine_from_position(&self, pos: BlockPos, target: BlockPos) -> bool {
        let dx = (pos.x - target.x).abs();
        let dy = (pos.y - target.y).abs();
        let dz = (pos.z - target.z).abs();
        
        // Standard mining reach is about 4.5 blocks
        let distance_squared = dx * dx + dy * dy + dz * dz;
        distance_squared <= 20 // ~4.47 blocks
    }

    /// Get all target positions for this mining goal
    pub fn get_target_positions(&self) -> Vec<BlockPos> {
        match self {
            MiningGoal::SingleBlock { target, .. } => vec![*target],
            MiningGoal::MultipleBlocks { targets, .. } => targets.clone(),
            MiningGoal::OreVein { blocks, .. } => blocks.iter().copied().collect(),
            MiningGoal::StripMine { start, direction, length, height, width } => {
                self.generate_strip_mine_positions(*start, *direction, *length, *height, *width)
            },
        }
    }

    /// Generate positions for strip mining pattern
    fn generate_strip_mine_positions(
        &self, 
        start: BlockPos, 
        direction: StripMineDirection, 
        length: u32, 
        height: u32, 
        width: u32
    ) -> Vec<BlockPos> {
        let mut positions = Vec::new();
        
        let (dx, dz) = match direction {
            StripMineDirection::North => (0, -1),
            StripMineDirection::South => (0, 1),
            StripMineDirection::East => (1, 0),
            StripMineDirection::West => (-1, 0),
        };

        for l in 0..length {
            for h in 0..height {
                for w in 0..width {
                    let pos = BlockPos {
                        x: start.x + (dx * l as i32) + if dx == 0 { w as i32 - (width as i32 / 2) } else { 0 },
                        y: start.y + h as i32,
                        z: start.z + (dz * l as i32) + if dz == 0 { w as i32 - (width as i32 / 2) } else { 0 },
                    };
                    positions.push(pos);
                }
            }
        }
        
        positions
    }

    /// Create a mining goal for an ore vein, automatically detecting connected ores
    pub fn for_ore_vein(initial_ore: BlockPos, all_ores: &[BlockPos], max_vein_distance: f32) -> Self {
        let mut vein_blocks = HashSet::new();
        let mut to_check = vec![initial_ore];
        let mut checked = HashSet::new();

        // Flood-fill to find connected ore blocks
        while let Some(current) = to_check.pop() {
            if checked.contains(&current) {
                continue;
            }
            checked.insert(current);
            vein_blocks.insert(current);

            // Find nearby ores to add to the vein
            for &ore_pos in all_ores {
                if !checked.contains(&ore_pos) {
                    let distance = ((current.x - ore_pos.x).pow(2) + 
                                   (current.y - ore_pos.y).pow(2) + 
                                   (current.z - ore_pos.z).pow(2)) as f32;
                    
                    if distance.sqrt() <= max_vein_distance {
                        to_check.push(ore_pos);
                    }
                }
            }
        }

        // Calculate center of the vein
        let center = if vein_blocks.is_empty() {
            initial_ore
        } else {
            let sum_x: i32 = vein_blocks.iter().map(|p| p.x).sum();
            let sum_y: i32 = vein_blocks.iter().map(|p| p.y).sum();
            let sum_z: i32 = vein_blocks.iter().map(|p| p.z).sum();
            let count = vein_blocks.len() as i32;
            
            BlockPos {
                x: sum_x / count,
                y: sum_y / count,
                z: sum_z / count,
            }
        };

        MiningGoal::OreVein {
            blocks: vein_blocks,
            center,
            max_reach: 4.5, // Standard mining reach
        }
    }

    /// Create an optimized goal for mining multiple scattered blocks
    pub fn for_scattered_blocks(blocks: Vec<BlockPos>, allow_internal: bool) -> Self {
        MiningGoal::MultipleBlocks {
            targets: blocks,
            allow_internal_mining: allow_internal,
        }
    }
}

/// Composite goal that combines multiple mining goals with priority
#[derive(Debug, Clone)]
pub struct PriorizedMiningGoal {
    pub goals: Vec<(MiningGoal, f32)>, // (goal, priority_weight)
}

impl Goal for PriorizedMiningGoal {
    fn heuristic(&self, pos: BlockPos) -> f32 {
        let mut best_weighted_heuristic = f32::INFINITY;
        
        for (goal, weight) in &self.goals {
            let heuristic = goal.heuristic(pos) * weight;
            if heuristic < best_weighted_heuristic {
                best_weighted_heuristic = heuristic;
            }
        }
        
        best_weighted_heuristic
    }

    fn success(&self, pos: BlockPos) -> bool {
        // Goal is reached if any of the mining goals is reached
        self.goals.iter().any(|(goal, _)| goal.success(pos))
    }
}
