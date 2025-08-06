# Azalea Pathfinding Water Traversal Enhancement Plan

## Overview
This document outlines the changes needed to improve Azalea's pathfinding capabilities to handle water traversal, based on analysis of both Azalea's current implementation and Baritone's advanced water handling.

## Current Limitations
1. Water blocks are treated as completely impassable
2. No distinction between still and flowing water
3. Missing swimming movement calculations
4. No water-specific pathfinding costs
5. Limited fluid state analysis

## Proposed Changes

### 1. Water Classification System
- Add `WaterType` enum to distinguish:
  - `StillWater`: Safe for navigation
  - `FlowingWater`: Generally avoid unless controlled
  - `Waterlogged`: Blocks that contain water
  - `Dangerous`: Near lava or other hazards

### 2. Enhanced Block Analysis
- Implement `is_water_navigable()` function
- Add flow direction detection
- Check for safe water entry/exit points
- Analyze water depth for swimming vs walking

### 3. Swimming Movement Implementation
- Add `swimming_move()` functions for basic water navigation
- Implement water ascent/descent pathfinding
- Add breath management considerations
- Handle underwater block breaking

### 4. Cost System Updates
- Add water-specific movement costs:
  - `WATER_WALK_COST`: Moving through shallow water
  - `SWIMMING_COST`: Swimming through deep water
  - `WATER_ASCENT_COST`: Swimming upward
  - `WATER_DESCENT_COST`: Swimming downward
  - `FLOW_RESISTANCE_COST`: Moving against water flow

### 5. New Movement Types
- `WaterTraverse`: Horizontal movement through water
- `WaterAscend`: Swimming upward in water
- `WaterDescend`: Swimming downward in water
- `WaterExit`: Exiting water onto land
- `WaterEntry`: Entering water from land

## Implementation Priority
1. **Phase 1**: Basic water traversal (still water only)
2. **Phase 2**: Flow detection and swimming mechanics
3. **Phase 3**: Advanced features (water buckets, complex navigation)

## Files to Modify
- `azalea/src/pathfinder/world.rs` - Water classification
- `azalea/src/pathfinder/costs.rs` - New cost constants
- `azalea/src/pathfinder/moves/` - New water movement modules
- `azalea/src/pathfinder/moves/mod.rs` - Integration

## Benefits
1. Enables navigation across rivers, lakes, and oceans
2. Allows underwater exploration and mining
3. Provides more efficient pathfinding in water-rich environments
4. Maintains safety by avoiding dangerous water situations
