use azalea::prelude::*;
use azalea::auto_tool::AutoToolPlugin;
use azalea::pathfinder::{
    mining::MiningCache,
    mining_goals::MiningGoal,
    mining_process::MiningProcess,
    world_scanner::WorldScanner,
};
use azalea_core::position::BlockPos;
use azalea_block::{blocks, BlockStates};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration for the enhanced mining bot
    let account = Account::offline("EnhancedMiner");
    
    ClientBuilder::new()
        .set_handler(mining_bot)
        .add_plugin(AutoToolPlugin)
        .start(account, "localhost")
        .await?;

    Ok(())
}

#[derive(Default, Component)]
pub struct MiningBotState {
    mining_process: Option<MiningProcess>,
    world_scanner: WorldScanner,
    mining_cache: MiningCache,
}

async fn mining_bot(bot: Client, event: Event, state: &mut MiningBotState) -> anyhow::Result<()> {
    match event {
        Event::Init => {
            println!("🤖 Enhanced Mining Bot initialized!");
            println!("🔍 Powered by Baritone-inspired mining algorithms");
            
            // Initialize the enhanced mining system
            state.world_scanner = WorldScanner::new();
            state.mining_cache = MiningCache::new();
            
            // Create a mining process to look for diamond ore
            let mut target_blocks = HashSet::new();
            target_blocks.insert(blocks::DiamondOre.into());
            target_blocks.insert(blocks::DeepslateQuartzOre.into());
            
            let mut mining_process = MiningProcess::new(BlockStates::from(target_blocks));
            mining_process.set_desired_quantity(64); // Mine up to 64 diamond ore
            mining_process.set_max_search_distance(100.0); // Search within 100 blocks
            
            state.mining_process = Some(mining_process);
            
            println!("💎 Configured to mine diamond ore and deepslate diamond ore");
            println!("📍 Search radius: 100 blocks");
            println!("🎯 Target quantity: 64 blocks");
        }
        
        Event::Chat(message) => {
            // Listen for commands
            if message.username() == Some(&bot.profile.name) {
                return Ok(());
            }
            
            let content = message.content();
            if content.starts_with("!mine ") {
                let block_type = &content[6..];
                handle_mine_command(&bot, state, block_type).await?;
            } else if content == "!status" {
                show_mining_status(&bot, state).await?;
            } else if content == "!stop" {
                state.mining_process = None;
                bot.chat("🛑 Mining stopped").await?;
            }
        }
        
        Event::Tick => {
            // Regular mining process updates
            if let Some(mining_process) = &mut state.mining_process {
                if let Some(position) = bot.position() {
                    let player_pos = BlockPos::from(position);
                    
                    // Update the mining process with current world state
                    mining_process.update_scan_results(player_pos, &bot.world);
                    
                    // Get the next mining goal
                    if let Some(goal) = mining_process.update_mining_goal(player_pos, &bot.world) {
                        // Check if we need to pathfind to the mining location
                        if !bot.physics.position.is_same_block_position(&goal.is_reached_on(&bot.world, &bot.entity.position)) {
                            // Set the pathfinding goal
                            bot.goto(goal);
                        }
                    }
                    
                    // Cleanup expired blacklisted positions
                    mining_process.cleanup_blacklist();
                }
            }
        }
        
        _ => {}
    }
    
    Ok(())
}

async fn handle_mine_command(
    bot: &Client,
    state: &mut MiningBotState,
    block_type: &str,
) -> anyhow::Result<()> {
    let target_block = match block_type.to_lowercase().as_str() {
        "diamond" | "diamonds" => blocks::DiamondOre.into(),
        "iron" => blocks::IronOre.into(),
        "coal" => blocks::CoalOre.into(),
        "gold" => blocks::GoldOre.into(),
        "emerald" => blocks::EmeraldOre.into(),
        "copper" => blocks::CopperOre.into(),
        "redstone" => blocks::RedstoneOre.into(),
        "lapis" => blocks::LapisOre.into(),
        _ => {
            bot.chat(&format!("❌ Unknown block type: {}", block_type)).await?;
            return Ok(());
        }
    };
    
    // Create new mining process for the requested block
    let mut target_blocks = HashSet::new();
    target_blocks.insert(target_block);
    
    // Also add deepslate variants if they exist
    let deepslate_variant = match block_type.to_lowercase().as_str() {
        "diamond" | "diamonds" => Some(blocks::DeepslateDiamondOre.into()),
        "iron" => Some(blocks::DeepslateIronOre.into()),
        "coal" => Some(blocks::DeepslateCoalOre.into()),
        "gold" => Some(blocks::DeepslateGoldOre.into()),
        "emerald" => Some(blocks::DeepslateEmeraldOre.into()),
        "copper" => Some(blocks::DeepslateCopperOre.into()),
        "redstone" => Some(blocks::DeepslateRedstoneOre.into()),
        "lapis" => Some(blocks::DeepslateLapisOre.into()),
        _ => None,
    };
    
    if let Some(deepslate) = deepslate_variant {
        target_blocks.insert(deepslate);
    }
    
    let mut mining_process = MiningProcess::new(BlockStates::from(target_blocks));
    mining_process.set_desired_quantity(32); // Mine up to 32 blocks
    mining_process.set_max_search_distance(80.0); // Search within 80 blocks
    
    state.mining_process = Some(mining_process);
    
    bot.chat(&format!("⛏️ Now mining {} ore! Target: 32 blocks", block_type)).await?;
    
    Ok(())
}

async fn show_mining_status(bot: &Client, state: &MiningBotState) -> anyhow::Result<()> {
    if let Some(mining_process) = &state.mining_process {
        let known_locations = mining_process.current_known_locations.len();
        let blacklisted = mining_process.blacklisted_positions.len();
        
        bot.chat(&format!(
            "📊 Mining Status: {} locations found, {} blacklisted",
            known_locations, blacklisted
        )).await?;
        
        if let Some(quantity) = mining_process.desired_quantity {
            bot.chat(&format!("🎯 Target quantity: {}", quantity)).await?;
        }
    } else {
        bot.chat("💤 No active mining process").await?;
    }
    
    Ok(())
}
