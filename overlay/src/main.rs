//! Example overlay application demonstrating the overlays
//!
//! Run with: cargo run -p baras-overlay
//!
//! Use command line args to select overlay type:
//!   --metric  (default) - DPS meter overlay
//!   --raid    - Three raid overlays side-by-side showing all interaction modes

use std::env;
use std::time::{Duration, Instant};

mod examples {
    use super::*;
    use baras_core::context::OverlayAppearanceConfig;
    use baras_overlay::{
        colors, InteractionMode, MetricEntry, MetricOverlay, Overlay, OverlayConfig, PlayerRole,
        RaidEffect, RaidFrame, RaidGridLayout, RaidOverlay, RaidOverlayConfig,
    };

    pub fn run_metric_overlay() {
        let config = OverlayConfig {
            x: 500,
            y: 500,
            width: 280,
            height: 200,
            namespace: "baras-dps-metric".to_string(),
            click_through: false,
            target_monitor_id: None,
        };

        let appearance = OverlayAppearanceConfig::default();
        let mut metric = match MetricOverlay::new(config, "DPS Meter", appearance, 180) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to create overlay: {}", e);
                return;
            }
        };

        let entries = vec![
            MetricEntry {
                name: "Player 1".to_string(),
                value: 12500,
                max_value: 15000,
                total_value: 2_500_000,
                color: colors::dps_bar_fill(),
            },
            MetricEntry {
                name: "Player 2".to_string(),
                value: 10200,
                max_value: 15000,
                total_value: 1_800_000,
                color: colors::dps_bar_fill(),
            },
            MetricEntry {
                name: "Player 3".to_string(),
                value: 8700,
                max_value: 15000,
                total_value: 1_200_000,
                color: colors::hps_bar_fill(),
            },
            MetricEntry {
                name: "Player 4".to_string(),
                value: 6100,
                max_value: 15000,
                total_value: 800_000,
                color: colors::tank_bar_fill(),
            },
        ];

        metric.set_entries(entries);

        let start = Instant::now();
        let mut last_frame = Instant::now();
        let frame_duration = Duration::from_millis(16);

        println!("Metric overlay running. Press Ctrl+C to exit.");

        loop {
            if !metric.poll_events() {
                break;
            }

            let now = Instant::now();
            if now.duration_since(last_frame) >= frame_duration {
                let elapsed = start.elapsed().as_secs();
                metric.set_title(&format!("DPS Meter - {}:{:02}", elapsed / 60, elapsed % 60));
                metric.render();
                last_frame = now;
            }

            // Sleep based on interactive state for CPU efficiency
            let sleep_ms = if metric.is_interactive() { 1 } else { 16 };
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }
    }

    /// Run three raid overlays side-by-side demonstrating each interaction mode
    pub fn run_raid_overlay() {
        let layout = RaidGridLayout { columns: 2, rows: 4 };
        let raid_config = RaidOverlayConfig::default();
        let test_frames = create_test_frames();

        // Create three overlays in a row, each in a different mode
        // Column 1: Normal (click-through)
        let config_normal = OverlayConfig {
            x: 50,
            y: 100,
            width: 220,
            height: 200,
            namespace: "baras-raid-normal".to_string(),
            click_through: true, // Will be set by InteractionMode::Normal
            target_monitor_id: None,
        };

        // Column 2: Move mode
        let config_move = OverlayConfig {
            x: 290,
            y: 100,
            width: 220,
            height: 200,
            namespace: "baras-raid-move".to_string(),
            click_through: false,
            target_monitor_id: None,
        };

        // Column 3: Rearrange mode
        let config_rearrange = OverlayConfig {
            x: 530,
            y: 100,
            width: 220,
            height: 200,
            namespace: "baras-raid-rearrange".to_string(),
            click_through: false,
            target_monitor_id: None,
        };

        let mut overlay_normal =
            match RaidOverlay::new(config_normal, layout, raid_config.clone(), 180) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("Failed to create normal overlay: {}", e);
                    return;
                }
            };

        let mut overlay_move = match RaidOverlay::new(config_move, layout, raid_config.clone(), 180)
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to create move overlay: {}", e);
                return;
            }
        };

        let mut overlay_rearrange =
            match RaidOverlay::new(config_rearrange, layout, raid_config, 180) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("Failed to create rearrange overlay: {}", e);
                    return;
                }
            };

        // Set up each overlay with test data and its interaction mode
        overlay_normal.set_frames(test_frames.clone());
        overlay_normal.set_interaction_mode(InteractionMode::Normal);

        overlay_move.set_frames(test_frames.clone());
        overlay_move.set_interaction_mode(InteractionMode::Move);

        overlay_rearrange.set_frames(test_frames);
        overlay_rearrange.set_interaction_mode(InteractionMode::Rearrange);

        let mut last_frame = Instant::now();
        let frame_duration = Duration::from_millis(16); // ~60fps

        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│          Raid Overlay Demo - Three Interaction Modes        │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│  LEFT:   Normal Mode   - Clicks pass through to game        │");
        println!("│  MIDDLE: Move Mode     - Drag to move, resize corner works  │");
        println!("│  RIGHT:  Rearrange Mode- Click frames to swap positions     │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│  Press Ctrl+C to exit                                       │");
        println!("└─────────────────────────────────────────────────────────────┘");

        loop {
            // Poll all overlays - if any closes, exit
            if !overlay_normal.poll_events()
             //   || !overlay_move.poll_events()
                || !overlay_rearrange.poll_events()
            {
                break;
            }

            let now = Instant::now();
            if now.duration_since(last_frame) >= frame_duration {
                overlay_normal.render();
            //    overlay_move.render();
                overlay_rearrange.render();
                last_frame = now;
            }

            // Adaptive sleep: faster polling when any overlay is interactive
            // In production, you'd also check for data changes (dirty flag)
            let any_interactive = overlay_move.is_interactive() || overlay_rearrange.is_interactive();
            let sleep_ms = if any_interactive { 4 } else { 16 };
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }
    }

    fn create_test_frames() -> Vec<RaidFrame> {
        vec![
            // Slot 0: Self (Tank)
            RaidFrame {
                slot: 0,
                player_id: Some(1001),
                name: "Tanky McTank".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Tank,
                effects: vec![RaidEffect::new(100, "Guard")
                    .with_color(tiny_skia::Color::from_rgba8(100, 150, 220, 255))],
                is_self: true,
            },
            // Slot 1: Healer
            RaidFrame {
                slot: 1,
                player_id: Some(1002),
                name: "Healz4Days".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Healer,
                effects: vec![RaidEffect::new(200, "Resurgence")
                    .with_color(tiny_skia::Color::from_rgba8(100, 220, 100, 255))
                    .with_charges(2)],
                is_self: false,
            },
            // Slot 2: DPS
            RaidFrame {
                slot: 2,
                player_id: Some(1003),
                name: "PewPewLazors".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Dps,
                effects: vec![
                    RaidEffect::new(300, "Kolto Probe")
                        .with_color(tiny_skia::Color::from_rgba8(150, 255, 150, 255)),
                    RaidEffect::new(301, "Force Armor")
                        .with_color(tiny_skia::Color::from_rgba8(200, 200, 100, 255)),
                ],
                is_self: false,
            },
            // Slot 3: DPS (no effects)
            RaidFrame {
                slot: 3,
                player_id: Some(1004),
                name: "StabbySith".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Dps,
                effects: vec![],
                is_self: false,
            },
            // Slot 4: Off-tank
            RaidFrame {
                slot: 4,
                player_id: Some(1005),
                name: "OffTankOT".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Tank,
                effects: vec![RaidEffect::new(400, "Saber Ward")
                    .with_color(tiny_skia::Color::from_rgba8(255, 200, 100, 255))],
                is_self: false,
            },
            // Slot 5: Healer (no effects)
            RaidFrame {
                slot: 5,
                player_id: Some(1006),
                name: "HoTsOnYou".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Healer,
                effects: vec![],
                is_self: false,
            },
            // Slot 6: DPS with debuff
            RaidFrame {
                slot: 6,
                player_id: Some(1007),
                name: "StandInFire".to_string(),
                hp_percent: 0.0,
                role: PlayerRole::Dps,
                effects: vec![RaidEffect::new(500, "Burning")
                    .with_color(tiny_skia::Color::from_rgba8(255, 100, 50, 255))
                    .with_is_buff(false)],
                is_self: false,
            },
            // Slot 7: Empty slot
            RaidFrame::empty(7),
        ]
    }

    /// Run a single raid overlay in Normal mode with 16 frames × 2 effects = 32 ticking timers
    /// This demonstrates the 10 FPS frame rate limiting for effect countdown rendering
    ///
    /// Layout shows 4 different effect styles (one per row):
    /// - Row 0: Non-opaque, no text
    /// - Row 1: Opaque, no text
    /// - Row 2: Non-opaque, with text
    /// - Row 3: Opaque, with text
    pub fn run_raid_timer_stress_test() {
        // 4x4 grid = 16 frames
        let layout = RaidGridLayout { columns: 4, rows: 4 };
        let raid_config = RaidOverlayConfig {
            max_effects_per_frame: 4,
            effect_size: 20.0,  // 1.4x scale (default 14 * 1.4 ≈ 20)
            ..Default::default()
        };

        let config = OverlayConfig {
            x: 100,
            y: 100,
            width: 500,  // Wider to accommodate larger effects
            height: 450,
            namespace: "baras-raid-timer-test".to_string(),
            click_through: true,
            target_monitor_id: None,
        };

        let mut overlay = match RaidOverlay::new(config, layout, raid_config, 180) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to create overlay: {}", e);
                return;
            }
        };

        // Create 16 frames with 2 effects each (32 total effects with duration timers)
        let frames = create_timer_stress_frames();
        overlay.set_frames(frames);
        overlay.set_interaction_mode(InteractionMode::Normal);

        let mut last_frame = Instant::now();
        let frame_duration = Duration::from_millis(16);

        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│       Raid Timer Stress Test - 32 Ticking Effect Timers     │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│  16 frames × 2 effects = 32 duration countdown bars         │");
        println!("│  Render rate: 10 FPS (capped for CPU efficiency)            │");
        println!("│  Effect size: 20px (1.4x scale)                             │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│  Row 0: Non-opaque, no text                                 │");
        println!("│  Row 1: Opaque, no text                                     │");
        println!("│  Row 2: Non-opaque, with text (stack count)                 │");
        println!("│  Row 3: Opaque, with text (stack count)                     │");
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│  Press Ctrl+C to exit                                       │");
        println!("└─────────────────────────────────────────────────────────────┘");

        loop {
            if !overlay.poll_events() {
                break;
            }

            let now = Instant::now();
            if now.duration_since(last_frame) >= frame_duration {
                overlay.render();
                last_frame = now;
            }

            std::thread::sleep(Duration::from_millis(16));
        }
    }

    /// Create 16 frames with 2 ticking effects each for the stress test
    /// Demonstrates 4 different effect visual styles across the 4 rows
    fn create_timer_stress_frames() -> Vec<RaidFrame> {
        let player_names = [
            // Row 0: Non-opaque, no text
            "TankOne", "TankTwo", "HealerA", "HealerB",
            // Row 1: Opaque, no text
            "DpsAlpha", "DpsBeta", "DpsGamma", "DpsDelta",
            // Row 2: Non-opaque, with text
            "RangedOne", "RangedTwo", "MeleeOne", "MeleeTwo",
            // Row 3: Opaque, with text
            "SupportA", "SupportB", "OffTank", "FlexDps",
        ];

        let roles = [
            PlayerRole::Tank, PlayerRole::Tank, PlayerRole::Healer, PlayerRole::Healer,
            PlayerRole::Dps, PlayerRole::Dps, PlayerRole::Dps, PlayerRole::Dps,
            PlayerRole::Dps, PlayerRole::Dps, PlayerRole::Dps, PlayerRole::Dps,
            PlayerRole::Healer, PlayerRole::Healer, PlayerRole::Tank, PlayerRole::Dps,
        ];

        // Base colors (will be modified with alpha for opacity variations)
        let base_colors = [
            (100, 220, 100), // Green (HoT)
            (100, 150, 220), // Blue (Shield)
            (220, 180, 50),  // Yellow (Buff)
            (180, 100, 220), // Purple (Debuff)
        ];

        (0..16).map(|slot| {
            let row = slot / 4;  // 0, 1, 2, or 3

            // Determine opacity based on row (0,2 = non-opaque, 1,3 = opaque)
            // Non-opaque at 100 alpha allows icons to show through clearly
            let is_opaque = row == 1 || row == 3;
            let alpha: u8 = if is_opaque { 255 } else { 100 };

            // Determine if text should show (row 2,3 = with text via charges)
            // Vary digit counts: 1-digit, 2-digit, and 3-digit examples
            let has_text = row >= 2;
            let charges: u8 = if has_text {
                match slot % 4 {
                    0 => 3,    // 1 digit
                    1 => 42,   // 2 digits
                    2 => 127,  // 3 digits
                    _ => 8,    // 1 digit
                }
            } else { 0 };

            // Stagger durations so effects expire at different times
            let base_duration_1 = Duration::from_secs(15 + (slot as u64 * 2));
            let base_duration_2 = Duration::from_secs(20 + (slot as u64 * 2));

            let (r1, g1, b1) = base_colors[slot % 4];
            let (r2, g2, b2) = base_colors[(slot + 1) % 4];

            let effect1 = RaidEffect::new(slot as u64 * 10, format!("Effect{}", slot * 2))
                .with_duration_from_now(base_duration_1)
                .with_color(tiny_skia::Color::from_rgba8(r1, g1, b1, alpha))
                .with_charges(charges);

            // Second effect gets different digit count
            let charges2: u8 = if has_text {
                match slot % 4 {
                    0 => 15,   // 2 digits
                    1 => 99,   // 2 digits
                    2 => 5,    // 1 digit
                    _ => 255,  // 3 digits (max u8)
                }
            } else { 0 };

            let effect2 = RaidEffect::new(slot as u64 * 10 + 1, format!("Effect{}", slot * 2 + 1))
                .with_duration_from_now(base_duration_2)
                .with_color(tiny_skia::Color::from_rgba8(r2, g2, b2, alpha))
                .with_charges(charges2);

            RaidFrame {
                slot: slot as u8,
                player_id: Some(2000 + slot as i64),
                name: player_names[slot].to_string(),
                hp_percent: 1.0,
                role: roles[slot],
                effects: vec![effect1, effect2],
                is_self: slot == 0,
            }
        }).collect()
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let overlay_type = args.get(1).map(|s| s.as_str()).unwrap_or("--metric");

    match overlay_type {
        "--raid" => examples::run_raid_overlay(),
        "--raid-timers" => examples::run_raid_timer_stress_test(),
        "--metric" => examples::run_metric_overlay(),
        _ => ()
    }
}
