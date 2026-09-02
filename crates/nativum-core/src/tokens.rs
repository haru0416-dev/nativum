//! Named design tokens. Values resolve against the live appearance.

use crate::color::Color;

/// Light or dark palette. Follows the host when the app does not pin one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    /// Warm paper background, dark ink.
    Light,
    /// Charcoal background, light ink.
    Dark,
}

/// Corner radius token names (`radius="md"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadiusToken {
    /// 4px.
    Sm,
    /// 8px.
    Md,
    /// 12px.
    Lg,
    /// 16px.
    Xl,
    /// 0.
    None,
}

impl RadiusToken {
    /// Parse a token name.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "sm" => Self::Sm,
            "md" => Self::Md,
            "lg" => Self::Lg,
            "xl" => Self::Xl,
            "none" | "0" => Self::None,
            _ => return None,
        })
    }

    /// Pixel radius.
    pub fn px(self) -> f32 {
        match self {
            Self::Sm => 4.0,
            Self::Md => 8.0,
            Self::Lg => 12.0,
            Self::Xl => 16.0,
            Self::None => 0.0,
        }
    }
}

/// Fully resolved color and radius tables for one appearance.
#[derive(Clone, Debug)]
pub struct TokenSet {
    /// Window chrome / page fill.
    pub background: Color,
    /// Raised panels, cards, fields.
    pub surface: Color,
    /// Subtle wells, secondary buttons.
    pub surface_subtle: Color,
    /// Pressed / selected wash.
    pub surface_pressed: Color,
    /// Primary copy.
    pub text: Color,
    /// Secondary copy.
    pub text_muted: Color,
    /// Hairlines, field borders.
    pub border: Color,
    /// Interactive accent (buttons, radios, focus companion).
    pub accent: Color,
    /// Text on accent fills.
    pub accent_text: Color,
    /// Destructive action.
    pub destructive: Color,
    /// Text on destructive fills.
    pub destructive_text: Color,
    /// Success.
    pub success: Color,
    /// Warning.
    pub warning: Color,
    /// Keyboard focus ring.
    pub focus_ring: Color,
    /// Disabled fill / copy.
    pub disabled: Color,
    /// Modal scrim.
    pub scrim: Color,
    /// Drop-shadow tint (software approximation).
    pub shadow: Color,
}

impl TokenSet {
    /// Look up a color token by the names Native SDK authors already know.
    pub fn color(&self, name: &str) -> Option<Color> {
        Some(match name {
            "background" => self.background,
            "surface" => self.surface,
            "surface_subtle" => self.surface_subtle,
            "surface_pressed" => self.surface_pressed,
            "text" => self.text,
            "text_muted" => self.text_muted,
            "border" => self.border,
            "accent" => self.accent,
            "accent_text" | "accent-foreground" => self.accent_text,
            "destructive" => self.destructive,
            "destructive_text" => self.destructive_text,
            "success" => self.success,
            "success_text" => self.text,
            "warning" => self.warning,
            "warning_text" => self.text,
            "info" => self.accent,
            "info_text" => self.accent_text,
            "focus_ring" | "focus-ring" => self.focus_ring,
            "shadow" => self.shadow,
            "scrim" => self.scrim,
            "disabled" => self.disabled,
            "transparent" => Color::TRANSPARENT,
            _ => return None,
        })
    }
}

/// Resolve the stock palette for an appearance.
pub fn tokens_for(appearance: Appearance) -> TokenSet {
    match appearance {
        Appearance::Light => TokenSet {
            background: Color::rgb(0xF4, 0xF3, 0xEE),
            surface: Color::rgb(0xFF, 0xFF, 0xFC),
            surface_subtle: Color::rgb(0xEB, 0xEA, 0xE3),
            surface_pressed: Color::rgb(0xE0, 0xDE, 0xD4),
            text: Color::rgb(0x1A, 0x19, 0x14),
            text_muted: Color::rgb(0x6B, 0x69, 0x5E),
            border: Color::rgb(0xD9, 0xD7, 0xCC),
            accent: Color::rgb(0x2A, 0x62, 0xD6),
            accent_text: Color::rgb(0xFF, 0xFF, 0xFF),
            destructive: Color::rgb(0xC4, 0x32, 0x2A),
            destructive_text: Color::rgb(0xFF, 0xFF, 0xFF),
            success: Color::rgb(0x2A, 0x8A, 0x4A),
            warning: Color::rgb(0xC4, 0x86, 0x12),
            focus_ring: Color::rgb(0x2A, 0x62, 0xD6),
            disabled: Color::rgb(0xB0, 0xAE, 0xA4),
            scrim: Color {
                r: 20,
                g: 18,
                b: 12,
                a: 90,
            },
            shadow: Color {
                r: 20,
                g: 18,
                b: 12,
                a: 40,
            },
        },
        Appearance::Dark => TokenSet {
            background: Color::rgb(0x14, 0x14, 0x12),
            surface: Color::rgb(0x1E, 0x1E, 0x1B),
            surface_subtle: Color::rgb(0x2A, 0x2A, 0x26),
            surface_pressed: Color::rgb(0x34, 0x34, 0x2E),
            text: Color::rgb(0xF2, 0xF1, 0xEA),
            text_muted: Color::rgb(0x9A, 0x98, 0x8C),
            border: Color::rgb(0x3A, 0x39, 0x33),
            accent: Color::rgb(0x6B, 0x9B, 0xFF),
            accent_text: Color::rgb(0x0C, 0x12, 0x24),
            destructive: Color::rgb(0xF0, 0x71, 0x67),
            destructive_text: Color::rgb(0x1A, 0x0A, 0x08),
            success: Color::rgb(0x5D, 0xC4, 0x78),
            warning: Color::rgb(0xE6, 0xB8, 0x4A),
            focus_ring: Color::rgb(0x6B, 0x9B, 0xFF),
            disabled: Color::rgb(0x6A, 0x68, 0x5E),
            scrim: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 140,
            },
            shadow: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 80,
            },
        },
    }
}
