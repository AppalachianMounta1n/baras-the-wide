//! Challenge tracking overlay
//!
//! Displays challenge metrics during boss encounters. Each challenge is rendered
//! as its own "card" showing the challenge title, duration, and per-player bars
//! with contribution percentages.

use std::collections::HashMap;

use baras_core::context::{ChallengeColumns, ChallengeLayout, ChallengeOverlayConfig};
use tiny_skia::Color;

use super::{Overlay, OverlayConfigUpdate, OverlayData};
use crate::frame::OverlayFrame;
use crate::platform::{OverlayConfig, PlatformError};
use crate::utils::{color_from_rgba, format_duration_short, truncate_name};
use crate::widgets::{colors, Footer, ProgressBar};
use baras_types::formatting;

/// Data for the challenges overlay
#[derive(Debug, Clone, Default)]
pub struct ChallengeData {
    /// Challenge entries to display
    pub entries: Vec<ChallengeEntry>,
    /// Boss encounter name (for header)
    pub boss_name: Option<String>,
    /// Total encounter duration in seconds
    pub duration_secs: f32,
    /// Phase durations (phase_id → seconds)
    pub phase_durations: HashMap<String, f32>,
}

/// Single challenge entry for display
#[derive(Debug, Clone)]
pub struct ChallengeEntry {
    /// Challenge display name
    pub name: String,
    /// Current total value
    pub value: i64,
    /// Number of events contributing
    pub event_count: u32,
    /// Value per second (if time-based)
    pub per_second: Option<f32>,
    /// Per-player breakdown (sorted by value descending)
    pub by_player: Vec<PlayerContribution>,
    /// Challenge duration in seconds (may differ from encounter duration for phase-specific)
    pub duration_secs: f32,
    /// Whether this challenge is enabled for display
    pub enabled: bool,
    /// Bar color for this challenge (optional, uses default if None)
    pub color: Option<Color>,
    /// Which columns to display for this challenge
    pub columns: ChallengeColumns,
}

impl Default for ChallengeEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            value: 0,
            event_count: 0,
            per_second: None,
            by_player: Vec::new(),
            duration_secs: 0.0,
            enabled: true,
            color: None,
            columns: ChallengeColumns::default(),
        }
    }
}

/// A player's contribution to a challenge
#[derive(Debug, Clone)]
pub struct PlayerContribution {
    /// Player entity ID (for linking to encounter data)
    pub entity_id: i64,
    /// Player name (resolved from encounter)
    pub name: String,
    /// Player's value contribution
    pub value: i64,
    /// Percentage of total (0.0-100.0)
    pub percent: f32,
    /// Value per second (if applicable)
    pub per_second: Option<f32>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Layout Constants
// ═══════════════════════════════════════════════════════════════════════════════

const BASE_WIDTH: f32 = 320.0;
const BASE_HEIGHT: f32 = 400.0;

const BASE_PADDING: f32 = 6.0;
const BASE_CARD_SPACING: f32 = 8.0;
const BASE_BAR_HEIGHT: f32 = 18.0;
const BASE_BAR_SPACING: f32 = 3.0;
const BASE_FONT_SIZE: f32 = 13.0;
const BASE_HEADER_FONT_SIZE: f32 = 12.0;
const BASE_DURATION_FONT_SIZE: f32 = 10.0; // Smaller than header

const MAX_NAME_CHARS: usize = 14;
const MAX_PLAYERS: usize = 8;

// ═══════════════════════════════════════════════════════════════════════════════
// Challenge Overlay
// ═══════════════════════════════════════════════════════════════════════════════

/// Overlay displaying multiple challenge metrics as stacked cards
pub struct ChallengeOverlay {
    frame: OverlayFrame,
    data: ChallengeData,
    background_alpha: u8,
    config: ChallengeOverlayConfig,
    european_number_format: bool,
}

impl ChallengeOverlay {
    pub fn new(
        overlay_config: OverlayConfig,
        config: ChallengeOverlayConfig,
        background_alpha: u8,
    ) -> Result<Self, PlatformError> {
        let mut frame = OverlayFrame::new(overlay_config, BASE_WIDTH, BASE_HEIGHT)?;
        frame.set_background_alpha(background_alpha);
        frame.set_label("Challenges");

        Ok(Self {
            frame,
            data: ChallengeData::default(),
            background_alpha,
            config,
            european_number_format: false,
        })
    }

    pub fn set_data(&mut self, data: ChallengeData) {
        self.data = data;
    }

    pub fn set_config(&mut self, config: ChallengeOverlayConfig) {
        self.config = config;
    }

    pub fn set_background_alpha(&mut self, alpha: u8) {
        self.background_alpha = alpha;
        self.frame.set_background_alpha(alpha);
    }

    pub fn render_overlay(&mut self) {
        let width = self.frame.width() as f32;
        let height = self.frame.height() as f32;
        let scale = self.frame.scale_factor();

        let padding = self.frame.scaled(BASE_PADDING);
        let card_spacing = self.frame.scaled(BASE_CARD_SPACING);
        let mut bar_height = self.frame.scaled(BASE_BAR_HEIGHT);
        let mut bar_spacing = self.frame.scaled(BASE_BAR_SPACING);
        let font_scale = self.config.font_scale.clamp(1.0, 2.0);
        let mut font_size = self.frame.scaled(BASE_FONT_SIZE * font_scale);
        let mut header_font_size = self.frame.scaled(BASE_HEADER_FONT_SIZE * font_scale);
        let mut duration_font_size = self.frame.scaled(BASE_DURATION_FONT_SIZE * font_scale);
        let mut bar_radius = 3.0 * scale;

        let font_color = color_from_rgba(self.config.font_color);
        let default_bar_color = color_from_rgba(self.config.default_bar_color);

        let show_duration = self.config.show_duration;
        let show_footer = self.config.show_footer;
        let max_display = self.config.max_display as usize;
        let layout = self.config.layout;

        // Filter to enabled challenges only - clone to avoid borrow issues
        // (must happen before begin_frame so we can compute content height for dynamic background)
        let enabled_challenges: Vec<ChallengeEntry> = self
            .data
            .entries
            .iter()
            .filter(|c| c.enabled)
            .take(max_display)
            .cloned()
            .collect();

        let num_visible = enabled_challenges.len();
        if num_visible > 0 {
            // Scale content up to fill available space when fewer challenges are shown.
            // Estimate per-card height: header + separator + player bars + optional footer
            let sep_overhead = bar_spacing * 2.0 + 4.0 * scale;
            let card_height_est = header_font_size
                + sep_overhead
                + MAX_PLAYERS as f32 * (bar_height + bar_spacing)
                + if show_footer {
                    font_size + bar_spacing
                } else {
                    0.0
                };

            let content_height_est = match layout {
                ChallengeLayout::Vertical => {
                    num_visible as f32 * card_height_est
                        + (num_visible.saturating_sub(1)) as f32 * card_spacing
                        + padding * 2.0
                }
                ChallengeLayout::Horizontal => card_height_est + padding * 2.0,
            };

            let content_scale = (height / content_height_est).clamp(1.0, 2.0);
            bar_height *= content_scale;
            bar_spacing *= content_scale;
            font_size *= content_scale;
            header_font_size *= content_scale;
            duration_font_size *= content_scale;
            bar_radius *= content_scale;
        }

        // Compute actual content height from final (scaled) dimensions for dynamic background
        let content_height = if num_visible == 0 {
            0.0
        } else {
            match layout {
                ChallengeLayout::Vertical => {
                    let mut h = padding;
                    for (idx, challenge) in enabled_challenges.iter().enumerate() {
                        if idx > 0 {
                            h += card_spacing;
                        }
                        // Card header: title + separator
                        h += header_font_size + bar_spacing * 2.0 + 2.0 + 4.0 * scale;
                        // Player bars
                        let num_players = challenge.by_player.len().min(MAX_PLAYERS);
                        h += num_players as f32 * (bar_height + bar_spacing);
                        // Footer
                        if show_footer {
                            h += 2.0 + bar_spacing + font_size + 6.0 * scale;
                        }
                    }
                    h + padding
                }
                ChallengeLayout::Horizontal => {
                    // All cards are same height (tallest card), side by side
                    let tallest_card = enabled_challenges
                        .iter()
                        .map(|c| {
                            let num_players = c.by_player.len().min(MAX_PLAYERS);
                            let card_h = header_font_size
                                + bar_spacing * 2.0
                                + 2.0
                                + 4.0 * scale
                                + num_players as f32 * (bar_height + bar_spacing)
                                + if show_footer {
                                    2.0 + bar_spacing + font_size + 6.0 * scale
                                } else {
                                    0.0
                                };
                            card_h
                        })
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(0.0);
                    padding * 2.0 + tallest_card
                }
            }
        };

        if self.config.dynamic_background {
            self.frame.begin_frame_with_content_height(content_height);
        } else {
            self.frame.begin_frame();
        }

        match layout {
            ChallengeLayout::Vertical => {
                self.render_vertical(
                    &enabled_challenges,
                    padding,
                    card_spacing,
                    bar_height,
                    bar_spacing,
                    font_size,
                    header_font_size,
                    duration_font_size,
                    bar_radius,
                    font_color,
                    default_bar_color,
                    show_duration,
                    show_footer,
                    width,
                    height,
                );
            }
            ChallengeLayout::Horizontal => {
                self.render_horizontal(
                    &enabled_challenges,
                    padding,
                    card_spacing,
                    bar_height,
                    bar_spacing,
                    font_size,
                    header_font_size,
                    duration_font_size,
                    bar_radius,
                    font_color,
                    default_bar_color,
                    show_duration,
                    show_footer,
                    width,
                    height,
                );
            }
        }

        self.frame.end_frame();
    }

    #[allow(clippy::too_many_arguments)]
    fn render_vertical(
        &mut self,
        challenges: &[ChallengeEntry],
        padding: f32,
        card_spacing: f32,
        bar_height: f32,
        bar_spacing: f32,
        font_size: f32,
        header_font_size: f32,
        duration_font_size: f32,
        bar_radius: f32,
        font_color: Color,
        default_bar_color: Color,
        show_duration: bool,
        show_footer: bool,
        width: f32,
        _height: f32,
    ) {
        let content_width = width - padding * 2.0;
        let mut y = padding;

        for (idx, challenge) in challenges.iter().enumerate() {
            if idx > 0 {
                y += card_spacing;
            }

            let bar_color = challenge.color.unwrap_or(default_bar_color);

            // Render challenge card header
            y = self.render_challenge_header(
                challenge,
                padding,
                y,
                content_width,
                header_font_size,
                duration_font_size,
                bar_spacing,
                font_color,
                show_duration,
            );

            // Render player bars (uses per-challenge columns setting)
            y = self.render_player_bars(
                challenge,
                padding,
                y,
                content_width,
                bar_height,
                bar_spacing,
                font_size,
                bar_radius,
                font_color,
                bar_color,
            );

            // Render footer for this challenge
            if show_footer {
                y = self.render_challenge_footer(
                    challenge,
                    padding,
                    y,
                    content_width,
                    font_size - 2.0,
                    bar_spacing,
                    font_color,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_horizontal(
        &mut self,
        challenges: &[ChallengeEntry],
        padding: f32,
        card_spacing: f32,
        bar_height: f32,
        bar_spacing: f32,
        font_size: f32,
        header_font_size: f32,
        duration_font_size: f32,
        bar_radius: f32,
        font_color: Color,
        default_bar_color: Color,
        show_duration: bool,
        show_footer: bool,
        width: f32,
        _height: f32,
    ) {
        let num_challenges = challenges.len();
        if num_challenges == 0 {
            return;
        }

        // Calculate card width for horizontal layout
        let total_spacing = card_spacing * (num_challenges - 1) as f32;
        let available_width = width - padding * 2.0 - total_spacing;
        let card_width = available_width / num_challenges as f32;

        for (idx, challenge) in challenges.iter().enumerate() {
            let card_x = padding + (card_width + card_spacing) * idx as f32;
            let mut y = padding;

            let bar_color = challenge.color.unwrap_or(default_bar_color);

            // Render challenge card header
            y = self.render_challenge_header(
                challenge,
                card_x,
                y,
                card_width,
                header_font_size,
                duration_font_size,
                bar_spacing,
                font_color,
                show_duration,
            );

            // Render player bars (uses per-challenge columns setting)
            y = self.render_player_bars(
                challenge,
                card_x,
                y,
                card_width,
                bar_height,
                bar_spacing,
                font_size,
                bar_radius,
                font_color,
                bar_color,
            );

            // Render footer for this challenge
            if show_footer {
                self.render_challenge_footer(
                    challenge,
                    card_x,
                    y,
                    card_width,
                    font_size - 2.0,
                    bar_spacing,
                    font_color,
                );
            }
        }
    }

    /// Render the challenge card header with name and optional duration
    #[allow(clippy::too_many_arguments)]
    fn render_challenge_header(
        &mut self,
        challenge: &ChallengeEntry,
        x: f32,
        y: f32,
        width: f32,
        header_font_size: f32,
        duration_font_size: f32,
        spacing: f32,
        font_color: Color,
        show_duration: bool,
    ) -> f32 {
        // Draw challenge name
        let title_y = y + header_font_size;
        self.frame
            .draw_text_glowed(&challenge.name, x, title_y, header_font_size, font_color);

        // Draw duration in smaller font on the right if enabled
        if show_duration {
            let duration_str = format!("({})", format_duration_short(challenge.duration_secs));
            let (duration_width, _) = self.frame.measure_text(&duration_str, duration_font_size);
            let duration_x = x + width - duration_width;
            // Align baseline with header text (adjust for smaller font)
            let duration_y = title_y - (header_font_size - duration_font_size) * 0.3;
            self.frame.draw_text_glowed(
                &duration_str,
                duration_x,
                duration_y,
                duration_font_size,
                font_color,
            );
        }

        // Draw separator line
        let sep_y = title_y + spacing + 2.0;
        let line_height = 0.2 * self.frame.scale_factor();
        self.frame
            .fill_rect(x, sep_y, width, line_height, font_color);

        sep_y + spacing + 4.0 * self.frame.scale_factor()
    }

    /// Render player contribution bars for a challenge
    #[allow(clippy::too_many_arguments)]
    fn render_player_bars(
        &mut self,
        challenge: &ChallengeEntry,
        x: f32,
        mut y: f32,
        width: f32,
        bar_height: f32,
        bar_spacing: f32,
        font_size: f32,
        bar_radius: f32,
        font_color: Color,
        bar_color: Color,
    ) -> f32 {
        let players: Vec<_> = challenge.by_player.iter().take(MAX_PLAYERS).collect();
        let max_value = players.iter().map(|p| p.value).fold(1_i64, |a, b| a.max(b));

        for player in &players {
            let display_name = truncate_name(&player.name, MAX_NAME_CHARS);
            let progress = if max_value > 0 {
                player.value as f32 / max_value as f32
            } else {
                0.0
            };

            let mut bar = ProgressBar::new(display_name, progress)
                .with_fill_color(bar_color)
                .with_bg_color(colors::dps_bar_bg())
                .with_text_color(font_color);

            // Use per-challenge columns setting
            let eu = self.european_number_format;
            match challenge.columns {
                ChallengeColumns::TotalPercent => {
                    // 2-column: total | percent
                    bar = bar
                        .with_center_text(formatting::format_compact(player.value, eu))
                        .with_right_text(formatting::format_pct(player.percent as f64, eu));
                }
                ChallengeColumns::TotalPerSecond => {
                    // 2-column: total | per_second
                    let per_sec_val = player.per_second.map(|ps| ps as i64).unwrap_or(0);
                    bar = bar
                        .with_center_text(formatting::format_compact(player.value, eu))
                        .with_right_text(formatting::format_compact(per_sec_val, eu));
                }
                ChallengeColumns::PerSecondPercent => {
                    // 2-column: per_second | percent
                    let per_sec_val = player.per_second.map(|ps| ps as i64).unwrap_or(0);
                    bar = bar
                        .with_center_text(formatting::format_compact(per_sec_val, eu))
                        .with_right_text(formatting::format_pct(player.percent as f64, eu));
                }
                ChallengeColumns::TotalOnly => {
                    // Single column: just total
                    bar = bar.with_right_text(formatting::format_compact(player.value, eu));
                }
                ChallengeColumns::PerSecondOnly => {
                    // Single column: just per_second
                    let per_sec_val = player.per_second.map(|ps| ps as i64).unwrap_or(0);
                    bar = bar.with_right_text(formatting::format_compact(per_sec_val, eu));
                }
                ChallengeColumns::PercentOnly => {
                    // Single column: just percent
                    bar = bar.with_right_text(formatting::format_pct(player.percent as f64, eu));
                }
            }

            bar.render(
                &mut self.frame,
                x,
                y,
                width,
                bar_height,
                font_size - 2.0,
                bar_radius,
            );
            y += bar_height + bar_spacing;
        }

        y
    }

    /// Render footer with totals aligned to match bar columns
    #[allow(clippy::too_many_arguments)]
    fn render_challenge_footer(
        &mut self,
        challenge: &ChallengeEntry,
        x: f32,
        y: f32,
        width: f32,
        font_size: f32,
        spacing: f32,
        font_color: Color,
    ) -> f32 {
        let total_sum: i64 = challenge.by_player.iter().map(|p| p.value).sum();
        let total_per_sec: f32 = challenge
            .by_player
            .iter()
            .filter_map(|p| p.per_second)
            .sum();

        // Use Footer widget for consistent alignment with metric overlays
        let eu = self.european_number_format;
        let footer = match challenge.columns {
            ChallengeColumns::TotalPercent => {
                // 2-column: total | 100%
                Footer::new("100%".to_string())
                    .with_secondary(formatting::format_compact(total_sum, eu))
                    .with_color(font_color)
            }
            ChallengeColumns::TotalPerSecond => {
                // 2-column: total | per_second
                Footer::new(formatting::format_compact(total_per_sec as i64, eu))
                    .with_secondary(formatting::format_compact(total_sum, eu))
                    .with_color(font_color)
            }
            ChallengeColumns::PerSecondPercent => {
                // 2-column: per_second | 100%
                Footer::new("100%".to_string())
                    .with_secondary(formatting::format_compact(total_per_sec as i64, eu))
                    .with_color(font_color)
            }
            ChallengeColumns::TotalOnly => {
                // Single column: just total
                Footer::new(formatting::format_compact(total_sum, eu)).with_color(font_color)
            }
            ChallengeColumns::PerSecondOnly => {
                // Single column: just per_second
                Footer::new(formatting::format_compact(total_per_sec as i64, eu))
                    .with_color(font_color)
            }
            ChallengeColumns::PercentOnly => {
                // Single column: 100%
                Footer::new("100%".to_string()).with_color(font_color)
            }
        };

        footer.render(&mut self.frame, x, y, width, font_size);

        y + font_size + spacing
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Overlay Trait Implementation
// ═══════════════════════════════════════════════════════════════════════════════

impl Overlay for ChallengeOverlay {
    fn update_data(&mut self, data: OverlayData) -> bool {
        if let OverlayData::Challenges(challenge_data) = data {
            // Skip render if both old and new have no challenges
            let old_empty = self.data.entries.is_empty();
            let new_empty = challenge_data.entries.is_empty();
            self.set_data(challenge_data);
            !(old_empty && new_empty)
        } else {
            false
        }
    }

    fn update_config(&mut self, config: OverlayConfigUpdate) {
        if let OverlayConfigUpdate::Challenge(challenge_config, alpha, european) = config {
            self.set_config(challenge_config);
            self.set_background_alpha(alpha);
            self.european_number_format = european;
        }
    }

    fn render(&mut self) {
        self.render_overlay();
    }

    fn poll_events(&mut self) -> bool {
        self.frame.poll_events()
    }

    fn frame(&self) -> &OverlayFrame {
        &self.frame
    }

    fn frame_mut(&mut self) -> &mut OverlayFrame {
        &mut self.frame
    }
}
