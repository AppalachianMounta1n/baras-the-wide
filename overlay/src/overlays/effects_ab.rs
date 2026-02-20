//! Effects Overlay (A/B)
//!
//! Consolidated overlay for displaying effect icons with countdowns.
//! Supports both horizontal (row) and vertical (column) layouts.
//! Used for Effects A and Effects B overlays.

use std::collections::HashMap;
use std::sync::Arc;

use super::{Overlay, OverlayConfigUpdate, OverlayData};
use crate::frame::OverlayFrame;
use crate::platform::{OverlayConfig, PlatformError};
use crate::utils::color_from_rgba;
use crate::widgets::colors;
use crate::widgets::Header;

/// Cache for pre-scaled icons to avoid re-scaling every frame
type ScaledIconCache = HashMap<(u64, u32), Vec<u8>>;

/// Layout direction for effects display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffectsLayout {
    /// Horizontal row of icons
    #[default]
    Horizontal,
    /// Vertical column of icons with text
    Vertical,
}

/// A single effect entry for display
#[derive(Debug, Clone)]
pub struct EffectABEntry {
    /// Effect ID for identification
    pub effect_id: u64,
    /// Ability ID for icon lookup
    pub icon_ability_id: u64,
    /// Display name of the effect
    pub name: String,
    /// Remaining time in seconds
    pub remaining_secs: f32,
    /// Total duration in seconds (for progress calculation)
    pub total_secs: f32,
    /// Color (RGBA) - used as fallback if no icon
    pub color: [u8; 4],
    /// Stack count (0 = don't show)
    pub stacks: u8,
    /// Source entity name
    pub source_name: String,
    /// Target entity name
    pub target_name: String,
    /// Pre-loaded icon RGBA data (width, height, rgba_bytes) - Arc for cheap cloning
    pub icon: Option<Arc<(u32, u32, Vec<u8>)>>,
    /// Whether to show the icon (true) or use colored square (false)
    pub show_icon: bool,
    /// Whether to display the source entity name
    pub display_source: bool,
}

impl EffectABEntry {
    /// Progress as 0.0 (expired) to 1.0 (full)
    pub fn progress(&self) -> f32 {
        if self.total_secs <= 0.0 {
            return 1.0;
        }
        (self.remaining_secs / self.total_secs).clamp(0.0, 1.0)
    }

    /// Format remaining time
    pub fn format_time(&self, european: bool) -> String {
        baras_types::formatting::format_countdown_compact(self.remaining_secs, "0", european)
    }
}

/// Data sent from service to effects overlay
#[derive(Debug, Clone, Default)]
pub struct EffectsABData {
    pub effects: Vec<EffectABEntry>,
}

/// Configuration for effects overlay
#[derive(Debug, Clone)]
pub struct EffectsABConfig {
    pub icon_size: u8,
    pub max_display: u8,
    pub layout: EffectsLayout,
    pub show_effect_names: bool,
    pub show_countdown: bool,
    /// When true, stacks are shown large and centered; timer is secondary
    pub stack_priority: bool,
    /// Show header title above overlay
    pub show_header: bool,
    /// Title to display in header
    pub header_title: String,
    /// Font scale multiplier (1.0 - 2.0, default 1.0)
    pub font_scale: f32,
    /// When true, background shrinks to fit content instead of filling the window
    pub dynamic_background: bool,
}

impl Default for EffectsABConfig {
    fn default() -> Self {
        Self {
            icon_size: 32,
            max_display: 8,
            layout: EffectsLayout::Horizontal,
            show_effect_names: false,
            show_countdown: true,
            stack_priority: false,
            show_header: false,
            header_title: String::new(),
            font_scale: 1.0,
            dynamic_background: true,
        }
    }
}

/// Base dimensions
const BASE_WIDTH: f32 = 300.0;
const BASE_HEIGHT: f32 = 300.0;
const BASE_PADDING: f32 = 4.0;
const BASE_SPACING: f32 = 4.0;
const BASE_FONT_SIZE: f32 = 10.0;

/// Effects overlay - displays effect icons in horizontal or vertical layout
pub struct EffectsABOverlay {
    frame: OverlayFrame,
    config: EffectsABConfig,
    background_alpha: u8,
    data: EffectsABData,
    /// Cache of pre-scaled icons (ability_id, size) -> scaled RGBA
    icon_cache: ScaledIconCache,
    /// Last rendered state for dirty checking: (effect_id, time_string, stacks)
    last_rendered: Vec<(u64, String, u8)>,
    /// Label for this overlay instance
    _label: String,
    european_number_format: bool,
}

impl EffectsABOverlay {
    /// Create a new effects overlay
    pub fn new(
        window_config: OverlayConfig,
        config: EffectsABConfig,
        background_alpha: u8,
        label: &str,
    ) -> Result<Self, PlatformError> {
        let mut frame = OverlayFrame::new(window_config, BASE_WIDTH, BASE_HEIGHT)?;
        frame.set_background_alpha(background_alpha);
        frame.set_label(label);

        Ok(Self {
            frame,
            config,
            background_alpha,
            data: EffectsABData::default(),
            icon_cache: HashMap::new(),
            last_rendered: Vec::new(),
            _label: label.to_string(),
            european_number_format: false,
        })
    }

    /// Update the config
    pub fn set_config(&mut self, config: EffectsABConfig) {
        self.config = config;
    }

    /// Update background alpha
    pub fn set_background_alpha(&mut self, alpha: u8) {
        self.background_alpha = alpha;
        self.frame.set_background_alpha(alpha);
    }

    /// Update the data and pre-cache any new icons
    pub fn set_data(&mut self, data: EffectsABData) {
        // Pre-cache icons at current display size
        let icon_size = self.frame.scaled(self.config.icon_size as f32) as u32;

        for effect in &data.effects {
            if let Some(ref icon_arc) = effect.icon {
                let cache_key = (effect.icon_ability_id, icon_size);
                if !self.icon_cache.contains_key(&cache_key) {
                    let (src_w, src_h, ref src_data) = **icon_arc;
                    let scaled = scale_icon(src_data, src_w, src_h, icon_size);
                    self.icon_cache.insert(cache_key, scaled);
                }
            }
        }

        self.data = data;
    }

    /// Render the overlay
    pub fn render(&mut self) {
        // In move mode, always render preview (bypass dirty check)
        if self.frame.is_in_move_mode() {
            self.render_preview();
            return;
        }

        match self.config.layout {
            EffectsLayout::Horizontal => self.render_horizontal(),
            EffectsLayout::Vertical => self.render_vertical(),
        }
    }

    /// Render horizontal layout (row of icons)
    fn render_horizontal(&mut self) {
        let max_display = self.config.max_display as usize;

        // Build current visible state for dirty check
        let current_state: Vec<(u64, String, u8)> = self
            .data
            .effects
            .iter()
            .take(max_display)
            .map(|e| {
                (
                    e.effect_id,
                    e.format_time(self.european_number_format),
                    e.stacks,
                )
            })
            .collect();

        // Skip render if nothing changed
        if current_state == self.last_rendered && !self.last_rendered.is_empty() {
            return;
        }
        self.last_rendered = current_state;

        let padding = self.frame.scaled(BASE_PADDING);
        let spacing = self.frame.scaled(BASE_SPACING);
        let font_scale = self.config.font_scale.clamp(1.0, 2.0);
        let font_size = self.frame.scaled(BASE_FONT_SIZE * font_scale);
        let icon_size = self.frame.scaled(self.config.icon_size as f32);
        let scale = self.frame.scale_factor();
        let header_font_size = font_size * 1.4;

        // Calculate header space if enabled
        let header_space = if self.config.show_header {
            header_font_size + spacing + 2.0 + spacing + 4.0 * scale
        } else {
            0.0
        };

        // Count visible entries for dynamic background
        let num_visible = self.data.effects.iter().take(max_display).count();

        // Compute content height for dynamic background
        let content_height = if num_visible == 0 {
            if self.config.show_header {
                padding * 2.0 + header_space
            } else {
                0.0
            }
        } else {
            let mut h = padding * 2.0 + header_space + icon_size;
            if self.config.show_effect_names {
                let name_font_size = font_size * 0.85;
                h += name_font_size + 2.0;
            }
            h
        };

        if self.config.dynamic_background {
            self.frame.begin_frame_with_content_height(content_height);
        } else {
            self.frame.begin_frame();
        }

        // Render header if enabled
        if self.config.show_header && !self.config.header_title.is_empty() {
            let content_width = self.frame.width() as f32 - 2.0 * padding;
            Header::new(&self.config.header_title)
                .with_color(colors::white())
                .render(
                    &mut self.frame,
                    padding,
                    padding,
                    content_width,
                    header_font_size,
                    spacing,
                );
        }

        if self.data.effects.is_empty() {
            self.frame.end_frame();
            return;
        }

        let mut x = padding;
        let y = padding + header_space;
        let icon_size_u32 = icon_size as u32;

        // Clone effects to avoid borrow issues
        let effects: Vec<_> = self
            .data
            .effects
            .iter()
            .take(max_display)
            .cloned()
            .collect();

        for effect in &effects {
            // Draw icon
            self.draw_icon(effect, x, y, icon_size, icon_size_u32);

            // Border
            self.frame
                .stroke_rounded_rect(x, y, icon_size, icon_size, 3.0, 1.0, colors::white());

            // Clock wipe overlay
            let progress = effect.progress();
            let overlay_height = icon_size * (1.0 - progress);
            if overlay_height > 1.0 {
                self.frame.fill_rect(
                    x,
                    y,
                    icon_size,
                    overlay_height,
                    color_from_rgba([0, 0, 0, 140]),
                );
            }

            // Stack priority vs normal mode
            if self.config.stack_priority && effect.stacks >= 1 {
                self.draw_stack_priority(effect, x, y, icon_size, font_size);
            } else {
                self.draw_normal_mode(effect, x, y, icon_size, font_size);
            }

            // Effect name below icon
            let mut text_y_offset = 0.0;
            if self.config.show_effect_names {
                let name_font_size = font_size * 0.85;
                let name = truncate_name(&effect.name, 8);
                let name_width = self.frame.measure_text(&name, name_font_size).0;
                let name_x = x + (icon_size - name_width) / 2.0;
                let name_y = y + icon_size + name_font_size + 2.0;

                self.frame
                    .draw_text_glowed(&name, name_x, name_y, name_font_size, colors::white());
                text_y_offset = name_font_size + 2.0;
            }

            // Source name below effect name
            if effect.display_source && !effect.source_name.is_empty() {
                let source_font_size = font_size * 0.75;
                let source = truncate_name(&effect.source_name, 10);
                let source_width = self.frame.measure_text(&source, source_font_size).0;
                let source_x = x + (icon_size - source_width) / 2.0;
                let source_y = y + icon_size + source_font_size + 2.0 + text_y_offset;

                self.frame.draw_text_glowed(
                    &source,
                    source_x,
                    source_y,
                    source_font_size,
                    colors::white(),
                );
            }

            x += icon_size + spacing;
        }

        self.frame.end_frame();
    }

    /// Render vertical layout (column with text beside icons)
    fn render_vertical(&mut self) {
        let max_display = self.config.max_display as usize;

        // Build current visible state for dirty check
        let current_state: Vec<(u64, String, u8)> = self
            .data
            .effects
            .iter()
            .take(max_display)
            .map(|e| {
                (
                    e.effect_id,
                    e.format_time(self.european_number_format),
                    e.stacks,
                )
            })
            .collect();

        // Skip render if nothing changed
        if current_state == self.last_rendered && !self.last_rendered.is_empty() {
            return;
        }
        self.last_rendered = current_state;

        let padding = self.frame.scaled(BASE_PADDING);
        let row_spacing = self.frame.scaled(BASE_SPACING);
        let font_scale = self.config.font_scale.clamp(1.0, 2.0);
        let font_size = self.frame.scaled(BASE_FONT_SIZE * font_scale);
        let icon_size = self.frame.scaled(self.config.icon_size as f32);
        let row_height = icon_size + row_spacing;
        let scale = self.frame.scale_factor();
        let header_font_size = font_size * 1.4;

        // Calculate header space if enabled
        let header_space = if self.config.show_header {
            header_font_size + row_spacing + 2.0 + row_spacing + 4.0 * scale
        } else {
            0.0
        };

        // Count visible entries for dynamic background
        let num_visible = self.data.effects.iter().take(max_display).count();

        // Compute content height for dynamic background
        let content_height = if num_visible == 0 {
            if self.config.show_header {
                padding * 2.0 + header_space
            } else {
                0.0
            }
        } else {
            padding * 2.0 + header_space + num_visible as f32 * row_height
        };

        if self.config.dynamic_background {
            self.frame.begin_frame_with_content_height(content_height);
        } else {
            self.frame.begin_frame();
        }

        // Render header if enabled
        if self.config.show_header && !self.config.header_title.is_empty() {
            let content_width = self.frame.width() as f32 - 2.0 * padding;
            Header::new(&self.config.header_title)
                .with_color(colors::white())
                .render(
                    &mut self.frame,
                    padding,
                    padding,
                    content_width,
                    header_font_size,
                    row_spacing,
                );
        }

        if self.data.effects.is_empty() {
            self.frame.end_frame();
            return;
        }

        let mut y = padding + header_space;
        let icon_size_u32 = icon_size as u32;

        // Clone effects to avoid borrow issues
        let effects: Vec<_> = self
            .data
            .effects
            .iter()
            .take(max_display)
            .cloned()
            .collect();

        for effect in &effects {
            let x = padding;

            // Draw icon
            self.draw_icon(effect, x, y, icon_size, icon_size_u32);

            // Clock wipe overlay
            let progress = effect.progress();
            let overlay_height = icon_size * (1.0 - progress);
            if overlay_height > 1.0 {
                self.frame.fill_rect(
                    x,
                    y,
                    icon_size,
                    overlay_height,
                    color_from_rgba([0, 0, 0, 140]),
                );
            }

            // Border
            self.frame
                .stroke_rounded_rect(x, y, icon_size, icon_size, 3.0, 1.0, colors::white());

            // Stack priority vs normal mode
            if self.config.stack_priority && effect.stacks >= 1 {
                self.draw_stack_priority(effect, x, y, icon_size, font_size);
            } else {
                self.draw_normal_mode(effect, x, y, icon_size, font_size);
            }

            // Text to the right of icon
            let text_x = x + icon_size + padding;
            let text_y = y + icon_size / 2.0;

            if self.config.show_effect_names {
                // Effect name on top
                let name_y = text_y - font_size * 0.3;
                self.frame.draw_text_glowed(
                    &effect.name,
                    text_x,
                    name_y,
                    font_size,
                    colors::white(),
                );

                // Countdown below
                if self.config.show_countdown && effect.total_secs > 0.0 {
                    let time_text = effect.format_time(self.european_number_format);
                    let time_y = name_y + font_size + 2.0;
                    self.frame.draw_text_glowed(
                        &time_text,
                        text_x,
                        time_y,
                        font_size * 0.9,
                        colors::white(),
                    );

                    // Source name below countdown
                    if effect.display_source && !effect.source_name.is_empty() {
                        let source_font_size = font_size * 0.8;
                        self.frame.draw_text_glowed(
                            &effect.source_name,
                            text_x,
                            time_y + font_size * 0.9 + 2.0,
                            source_font_size,
                            colors::white(),
                        );
                    }
                }
            } else {
                // Just countdown centered
                if self.config.show_countdown && effect.total_secs > 0.0 {
                    let time_text = effect.format_time(self.european_number_format);
                    self.frame.draw_text_glowed(
                        &time_text,
                        text_x,
                        text_y + font_size / 3.0,
                        font_size,
                        colors::white(),
                    );

                    // Source name below countdown
                    if effect.display_source && !effect.source_name.is_empty() {
                        let source_font_size = font_size * 0.8;
                        self.frame.draw_text_glowed(
                            &effect.source_name,
                            text_x,
                            text_y + font_size / 3.0 + font_size + 2.0,
                            source_font_size,
                            colors::white(),
                        );
                    }
                }
            }

            y += row_height;
        }

        self.frame.end_frame();
    }

    /// Draw icon or colored square fallback
    fn draw_icon(
        &mut self,
        effect: &EffectABEntry,
        x: f32,
        y: f32,
        icon_size: f32,
        icon_size_u32: u32,
    ) {
        let cache_key = (effect.icon_ability_id, icon_size_u32);
        let has_icon = if effect.show_icon {
            if let Some(scaled_icon) = self.icon_cache.get(&cache_key) {
                self.frame.draw_image(
                    scaled_icon,
                    icon_size_u32,
                    icon_size_u32,
                    x,
                    y,
                    icon_size,
                    icon_size,
                );
                true
            } else if let Some(ref icon_arc) = effect.icon {
                let (img_w, img_h, ref rgba) = **icon_arc;
                self.frame
                    .draw_image(rgba, img_w, img_h, x, y, icon_size, icon_size);
                true
            } else {
                false
            }
        } else {
            false
        };

        if !has_icon {
            // Fallback: colored square
            let bg_color = color_from_rgba(effect.color);
            self.frame
                .fill_rounded_rect(x, y, icon_size, icon_size, 3.0, bg_color);
        }
    }

    /// Draw stack-priority mode (big stacks centered, timer in corner)
    fn draw_stack_priority(
        &mut self,
        effect: &EffectABEntry,
        x: f32,
        y: f32,
        icon_size: f32,
        font_size: f32,
    ) {
        let stack_text = format!("{}", effect.stacks);
        let stack_font_size = font_size * 1.9;
        let text_width = self.frame.measure_text(&stack_text, stack_font_size).0;
        let text_x = x + (icon_size - text_width) / 2.0;
        let text_y = y + icon_size / 2.0 + stack_font_size / 3.0;

        self.frame.draw_text_glowed(
            &stack_text,
            text_x,
            text_y,
            stack_font_size,
            colors::white(),
        );

        // Timer small in top-right corner
        if self.config.show_countdown && effect.total_secs > 0.0 {
            let time_text = effect.format_time(self.european_number_format);
            let time_font_size = font_size * 0.9;
            let time_x =
                x + icon_size - self.frame.measure_text(&time_text, time_font_size).0 - 2.0;
            let time_y = y + time_font_size + 2.0;

            self.frame.draw_text_glowed(
                &time_text,
                time_x,
                time_y,
                time_font_size,
                colors::icon_countdown(),
            );
        }
    }

    /// Draw normal mode (timer centered, stacks in corner)
    fn draw_normal_mode(
        &mut self,
        effect: &EffectABEntry,
        x: f32,
        y: f32,
        icon_size: f32,
        font_size: f32,
    ) {
        if self.config.show_countdown && effect.total_secs > 0.0 {
            let time_text = effect.format_time(self.european_number_format);
            let text_width = self.frame.measure_text(&time_text, font_size).0;
            let text_x = x + (icon_size - text_width) / 2.0;
            let text_y = y + icon_size / 2.0 + font_size * 0.4;

            self.frame.draw_text_glowed(
                &time_text,
                text_x,
                text_y,
                font_size,
                colors::icon_countdown(),
            );
        }

        // Stack count in bottom-right corner
        if effect.stacks >= 1 {
            let stack_text = format!("{}", effect.stacks);
            let stack_font_size = font_size * 1.4;
            let stack_x =
                x + icon_size - self.frame.measure_text(&stack_text, stack_font_size).0 - 2.0;
            let stack_y = y + icon_size - 3.0;

            self.frame.draw_text_glowed(
                &stack_text,
                stack_x,
                stack_y,
                stack_font_size,
                colors::icon_countdown(),
            );
        }
    }

    /// Render preview placeholders in move mode
    fn render_preview(&mut self) {
        match self.config.layout {
            EffectsLayout::Horizontal => self.render_preview_horizontal(),
            EffectsLayout::Vertical => self.render_preview_vertical(),
        }
    }

    /// Render horizontal preview (row of icons)
    fn render_preview_horizontal(&mut self) {
        let padding = self.frame.scaled(BASE_PADDING);
        let spacing = self.frame.scaled(BASE_SPACING);
        let font_size = self.frame.scaled(BASE_FONT_SIZE);
        let icon_size = self.frame.scaled(self.config.icon_size as f32);
        let scale = self.frame.scale_factor();
        let header_font_size = font_size * 1.4;

        // Calculate header space if enabled
        let header_space = if self.config.show_header {
            header_font_size + spacing + 2.0 + spacing + 4.0 * scale
        } else {
            0.0
        };

        self.frame.begin_frame();

        // Render header if enabled
        if self.config.show_header && !self.config.header_title.is_empty() {
            let content_width = self.frame.width() as f32 - 2.0 * padding;
            Header::new(&self.config.header_title)
                .with_color(colors::white())
                .render(
                    &mut self.frame,
                    padding,
                    padding,
                    content_width,
                    header_font_size,
                    spacing,
                );
        }

        let mut x = padding;
        let y = padding + header_space;

        // Sample preview data: (time, stacks)
        let previews = [("12.3", 3u8), ("45", 1u8), ("8.5", 2u8)];

        for (time_text, stacks) in &previews {
            // Placeholder icon background
            self.frame
                .fill_rounded_rect(x, y, icon_size, icon_size, 3.0, colors::effect_icon_bg());

            // Dashed border to indicate preview
            self.frame.stroke_rounded_rect_dashed(
                x,
                y,
                icon_size,
                icon_size,
                3.0,
                1.0,
                colors::preview_border(),
                3.0,
                2.0,
            );

            // Stack priority vs normal mode
            if self.config.stack_priority && *stacks >= 1 {
                self.draw_preview_stack_priority(x, y, icon_size, font_size, time_text, *stacks);
            } else {
                self.draw_preview_normal_mode(x, y, icon_size, font_size, time_text, *stacks);
            }

            // Effect name below icon
            if self.config.show_effect_names {
                let name_font_size = font_size * 0.85;
                let name = "Effect";
                let name_width = self.frame.measure_text(name, name_font_size).0;
                let name_x = x + (icon_size - name_width) / 2.0;
                let name_y = y + icon_size + name_font_size + 2.0;

                self.frame
                    .draw_text_glowed(name, name_x, name_y, name_font_size, colors::white());
            }

            x += icon_size + spacing;
        }

        self.frame.end_frame();
    }

    /// Render vertical preview (column with text beside icons)
    fn render_preview_vertical(&mut self) {
        let padding = self.frame.scaled(BASE_PADDING);
        let row_spacing = self.frame.scaled(BASE_SPACING);
        let font_size = self.frame.scaled(BASE_FONT_SIZE);
        let icon_size = self.frame.scaled(self.config.icon_size as f32);
        let row_height = icon_size + row_spacing;
        let scale = self.frame.scale_factor();
        let header_font_size = font_size * 1.4;

        // Calculate header space if enabled
        let header_space = if self.config.show_header {
            header_font_size + row_spacing + 2.0 + row_spacing + 4.0 * scale
        } else {
            0.0
        };

        self.frame.begin_frame();

        // Render header if enabled
        if self.config.show_header && !self.config.header_title.is_empty() {
            let content_width = self.frame.width() as f32 - 2.0 * padding;
            Header::new(&self.config.header_title)
                .with_color(colors::white())
                .render(
                    &mut self.frame,
                    padding,
                    padding,
                    content_width,
                    header_font_size,
                    row_spacing,
                );
        }

        let mut y = padding + header_space;

        // Sample preview data: (time, stacks)
        let previews = [("12.3", 3u8), ("45", 1u8), ("8.5", 2u8)];

        for (time_text, stacks) in &previews {
            let x = padding;

            // Placeholder icon background
            self.frame
                .fill_rounded_rect(x, y, icon_size, icon_size, 3.0, colors::effect_icon_bg());

            // Dashed border to indicate preview
            self.frame.stroke_rounded_rect_dashed(
                x,
                y,
                icon_size,
                icon_size,
                3.0,
                1.0,
                colors::preview_border(),
                3.0,
                2.0,
            );

            // Stack count in corner
            if *stacks >= 1 {
                let stack_text = format!("{}", stacks);
                let stack_font_size = font_size * 1.0;
                let stack_x =
                    x + icon_size - self.frame.measure_text(&stack_text, stack_font_size).0 - 2.0;
                let stack_y = y + stack_font_size + 2.0;

                self.frame.draw_text_glowed(
                    &stack_text,
                    stack_x,
                    stack_y,
                    stack_font_size,
                    colors::icon_countdown(),
                );
            }

            // Text to the right of icon
            let text_x = x + icon_size + padding;
            let text_y = y + icon_size / 2.0;

            if self.config.show_effect_names {
                // Effect name on top
                let name_y = text_y - font_size * 0.3;
                self.frame
                    .draw_text_glowed("Effect", text_x, name_y, font_size, colors::white());

                // Countdown below
                if self.config.show_countdown {
                    let time_y = name_y + font_size + 2.0;
                    self.frame.draw_text_glowed(
                        time_text,
                        text_x,
                        time_y,
                        font_size * 0.9,
                        colors::white(),
                    );
                }
            } else if self.config.show_countdown {
                // Just countdown centered
                self.frame.draw_text_glowed(
                    time_text,
                    text_x,
                    text_y + font_size / 3.0,
                    font_size,
                    colors::white(),
                );
            }

            y += row_height;
        }

        self.frame.end_frame();
    }

    /// Draw preview stack-priority mode (big stacks centered, timer in corner)
    fn draw_preview_stack_priority(
        &mut self,
        x: f32,
        y: f32,
        icon_size: f32,
        font_size: f32,
        time_text: &str,
        stacks: u8,
    ) {
        let stack_text = format!("{}", stacks);
        let stack_font_size = font_size * 1.9;
        let text_width = self.frame.measure_text(&stack_text, stack_font_size).0;
        let text_x = x + (icon_size - text_width) / 2.0;
        let text_y = y + icon_size / 2.0 + stack_font_size / 3.0;

        self.frame.draw_text_glowed(
            &stack_text,
            text_x,
            text_y,
            stack_font_size,
            colors::white(),
        );

        // Timer small in top-right corner
        if self.config.show_countdown {
            let time_font_size = font_size * 0.9;
            let time_x = x + icon_size - self.frame.measure_text(time_text, time_font_size).0 - 2.0;
            let time_y = y + time_font_size + 2.0;

            self.frame.draw_text_glowed(
                time_text,
                time_x,
                time_y,
                time_font_size,
                colors::icon_countdown(),
            );
        }
    }

    /// Draw preview normal mode (timer centered, stacks in corner)
    fn draw_preview_normal_mode(
        &mut self,
        x: f32,
        y: f32,
        icon_size: f32,
        font_size: f32,
        time_text: &str,
        stacks: u8,
    ) {
        if self.config.show_countdown {
            let text_width = self.frame.measure_text(time_text, font_size).0;
            let text_x = x + (icon_size - text_width) / 2.0;
            let text_y = y + icon_size / 2.0 + font_size * 0.4;

            self.frame.draw_text_glowed(
                time_text,
                text_x,
                text_y,
                font_size,
                colors::icon_countdown(),
            );
        }

        // Stack count in bottom-right corner
        if stacks >= 1 {
            let stack_text = format!("{}", stacks);
            let stack_font_size = font_size * 1.4;
            let stack_x =
                x + icon_size - self.frame.measure_text(&stack_text, stack_font_size).0 - 2.0;
            let stack_y = y + icon_size - 3.0;

            self.frame.draw_text_glowed(
                &stack_text,
                stack_x,
                stack_y,
                stack_font_size,
                colors::icon_countdown(),
            );
        }
    }
}

/// Truncate a name to fit within a character limit
fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        name.to_string()
    } else {
        let truncated: String = name.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    }
}

/// Scale icon to target size (nearest neighbor for speed)
fn scale_icon(src: &[u8], src_w: u32, src_h: u32, target_size: u32) -> Vec<u8> {
    let mut dest = vec![0u8; (target_size * target_size * 4) as usize];
    let scale_x = src_w as f32 / target_size as f32;
    let scale_y = src_h as f32 / target_size as f32;

    for dy in 0..target_size {
        for dx in 0..target_size {
            let sx = ((dx as f32 * scale_x) as u32).min(src_w - 1);
            let sy = ((dy as f32 * scale_y) as u32).min(src_h - 1);
            let src_idx = ((sy * src_w + sx) * 4) as usize;
            let dest_idx = ((dy * target_size + dx) * 4) as usize;

            dest[dest_idx] = src[src_idx];
            dest[dest_idx + 1] = src[src_idx + 1];
            dest[dest_idx + 2] = src[src_idx + 2];
            dest[dest_idx + 3] = src[src_idx + 3];
        }
    }
    dest
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay Trait Implementation
// ─────────────────────────────────────────────────────────────────────────────

impl Overlay for EffectsABOverlay {
    fn update_data(&mut self, data: OverlayData) -> bool {
        match data {
            OverlayData::EffectsA(effects_data) | OverlayData::EffectsB(effects_data) => {
                let was_empty = self.data.effects.is_empty();
                let is_empty = effects_data.effects.is_empty();
                self.set_data(effects_data);
                !(was_empty && is_empty)
            }
            _ => false,
        }
    }

    fn update_config(&mut self, config: OverlayConfigUpdate) {
        match config {
            OverlayConfigUpdate::EffectsA(cfg, alpha, european)
            | OverlayConfigUpdate::EffectsB(cfg, alpha, european) => {
                self.set_config(cfg);
                self.set_background_alpha(alpha);
                self.european_number_format = european;
            }
            _ => {}
        }
    }

    fn render(&mut self) {
        EffectsABOverlay::render(self);
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
