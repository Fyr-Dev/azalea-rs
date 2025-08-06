use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use azalea_block::{BlockState, BlockStates};
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_world::{Instance, Section};
use azalea_world::palette::Palette;
use azalea_core::position::ChunkSectionBlockPos;
use nohash_hasher::IntSet;

/// Advanced world scanner optimized for mining operations
pub struct WorldScanner {
    /// Cached ore locations by block type
    ore_cache: Arc<Mutex<HashMap<BlockState, Vec<CachedOreLocation>>>>,
    /// Chunks that have been scanned
    scanned_chunks: Arc<Mutex<IntSet<ChunkPos>>>,
    /// Background scanning thread handle
    scan_thread: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct CachedOreLocation {
    pub pos: BlockPos,
    pub chunk_pos: ChunkPos,
    pub last_seen: Instant,
    pub is_accessible: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub block_states: BlockStates,
    pub center_pos: BlockPos,
    pub max_radius: u32,
    pub max_results: usize,
    pub y_level_threshold: Option<i32>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub positions: Vec<BlockPos>,
    pub is_complete: bool,
    pub scan_time: Duration,
}

impl WorldScanner {
    pub fn new() -> Self {
        Self {
            ore_cache: Arc::new(Mutex::new(HashMap::new())),
            scanned_chunks: Arc::new(Mutex::new(IntSet::default())),
            scan_thread: None,
        }
    }

    /// Scan for blocks in a spiral pattern around the player, optimized for mining
    pub fn scan_for_blocks(
        &mut self,
        instance: &Instance,
        request: ScanRequest,
    ) -> ScanResult {
        let start_time = Instant::now();
        let mut positions = Vec::new();
        
        let start_chunk: ChunkPos = (&request.center_pos).into();
        let max_chunk_radius = (request.max_radius + 15) / 16; // Convert to chunk radius
        
        // Prioritize Y levels closer to player
        let player_y = request.center_pos.y;
        let y_sections = if let Some(threshold) = request.y_level_threshold {
            self.get_prioritized_y_sections(player_y, threshold)
        } else {
            (instance.chunks.min_y / 16..=(instance.chunks.min_y + instance.chunks.height as i32) / 16).collect()
        };

        // Spiral search pattern
        for radius in 0..=max_chunk_radius {
            if positions.len() >= request.max_results {
                break;
            }

            for chunk_pos in self.spiral_chunk_positions(start_chunk, radius) {
                if let Some(chunk) = instance.chunks.get(&chunk_pos) {
                    let chunk_guard = chunk.read();
                    
                    // Scan chunk sections in Y-priority order
                    for &section_y in &y_sections {
                        if let Some(section) = chunk_guard.sections.get(section_y as usize) {
                            self.scan_chunk_section(
                                &mut positions,
                                chunk_pos,
                                section,
                                section_y,
                                &request,
                                instance.chunks.min_y,
                            );
                            
                            if positions.len() >= request.max_results {
                                break;
                            }
                        }
                    }
                }
                
                if positions.len() >= request.max_results {
                    break;
                }
            }
        }

        // Sort by distance to player
        let positions_len = positions.len();
        positions.sort_by_key(|pos| {
            let dx = pos.x - request.center_pos.x;
            let dy = pos.y - request.center_pos.y;
            let dz = pos.z - request.center_pos.z;
            dx.abs() + dy.abs() + dz.abs() // Manhattan distance for performance
        });

        ScanResult {
            positions,
            is_complete: positions_len < request.max_results,
            scan_time: start_time.elapsed(),
        }
    }

    /// Generate spiral pattern of chunk positions around center
    fn spiral_chunk_positions(&self, center: ChunkPos, radius: u32) -> Vec<ChunkPos> {
        let mut positions = Vec::new();
        let r = radius as i32;

        if radius == 0 {
            return vec![center];
        }

        // Generate positions in a square spiral
        for x in -r..=r {
            for z in -r..=r {
                // Only include positions on the current radius "ring"
                if (x.abs() == r || z.abs() == r) && x.abs() <= r && z.abs() <= r {
                    positions.push(ChunkPos {
                        x: center.x + x,
                        z: center.z + z,
                    });
                }
            }
        }

        positions
    }

    /// Get Y sections prioritized by distance from player Y
    fn get_prioritized_y_sections(&self, player_y: i32, threshold: i32) -> Vec<i32> {
        let player_section = player_y / 16;
        let mut sections = Vec::new();
        
        // Add sections within threshold first
        for offset in 0..=(threshold / 16) {
            if offset == 0 {
                sections.push(player_section);
            } else {
                sections.push(player_section + offset);
                sections.push(player_section - offset);
            }
        }
        
        sections.into_iter().filter(|&y| y >= -4 && y <= 19).collect() // World height limits
    }

    /// Scan a single chunk section for target blocks
    fn scan_chunk_section(
        &self,
        results: &mut Vec<BlockPos>,
        chunk_pos: ChunkPos,
        section: &Section,
        section_y: i32,
        request: &ScanRequest,
        world_min_y: i32,
    ) {
        // Quick palette check first
        if !self.palette_contains_target(&section.states.palette, &request.block_states) {
            return;
        }

        let base_x = chunk_pos.x * 16;
        let base_z = chunk_pos.z * 16;
        let base_y = world_min_y + section_y * 16;

        // Iterate through all blocks in section
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let pos = ChunkSectionBlockPos::new(x as u8, y as u8, z as u8);
                    let block_state = section.states.get(pos);
                    
                    if request.block_states.contains(&block_state) {
                        let block_pos = BlockPos {
                            x: base_x + x as i32,
                            y: base_y + y as i32,
                            z: base_z + z as i32,
                        };
                        
                        results.push(block_pos);
                        
                        if results.len() >= request.max_results {
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Check if palette contains any of the target block states
    fn palette_contains_target(&self, palette: &Palette<BlockState>, targets: &BlockStates) -> bool {
        match palette {
            Palette::SingleValue(state) => targets.contains(state),
            Palette::Linear(states) => states.iter().any(|state| targets.contains(state)),
            Palette::Hashmap(states) => states.iter().any(|state| targets.contains(state)),
            Palette::Global => {
                // For global palette, we can't efficiently check without scanning all blocks
                // Return true to be safe and let the block-by-block scan handle it
                true
            }
        }
    }

    /// Cache ore locations for future reference
    pub fn cache_ore_locations(&self, block_state: BlockState, locations: Vec<BlockPos>) {
        let mut cache = self.ore_cache.lock().unwrap();
        let cached_locations: Vec<CachedOreLocation> = locations.into_iter().map(|pos| {
            CachedOreLocation {
                pos,
                chunk_pos: (&pos).into(),
                last_seen: Instant::now(),
                is_accessible: None,
            }
        }).collect();
        
        cache.insert(block_state, cached_locations);
    }

    /// Get cached ore locations, filtering by age and accessibility
    pub fn get_cached_ore_locations(&self, block_state: BlockState, max_age: Duration) -> Vec<BlockPos> {
        let cache = self.ore_cache.lock().unwrap();
        
        if let Some(locations) = cache.get(&block_state) {
            let now = Instant::now();
            locations.iter()
                .filter(|loc| now.duration_since(loc.last_seen) <= max_age)
                .filter(|loc| loc.is_accessible.unwrap_or(true))
                .map(|loc| loc.pos)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Mark ore location as accessible or inaccessible
    pub fn mark_ore_accessibility(&self, pos: BlockPos, accessible: bool) {
        let mut cache = self.ore_cache.lock().unwrap();
        
        for locations in cache.values_mut() {
            if let Some(location) = locations.iter_mut().find(|loc| loc.pos == pos) {
                location.is_accessible = Some(accessible);
                break;
            }
        }
    }

    /// Clear cache for chunks that are no longer loaded
    pub fn cleanup_unloaded_chunks(&self, loaded_chunks: &IntSet<ChunkPos>) {
        let mut cache = self.ore_cache.lock().unwrap();
        
        for locations in cache.values_mut() {
            locations.retain(|loc| loaded_chunks.contains(&loc.chunk_pos));
        }
    }
}

impl Default for WorldScanner {
    fn default() -> Self {
        Self::new()
    }
}
