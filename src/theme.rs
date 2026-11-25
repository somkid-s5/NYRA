use ratatui::style::Color;
use std::{borrow::Cow, ops::Deref};
use toml_edit::{DocumentMut, Item};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ThemeId {
    #[default]
    CyberCyan,
    NeoTokyo,
    AcidGrid,
    CrimsonNoir,
    Custom,
}

impl ThemeId {
    pub(crate) const fn as_config_value(self) -> &'static str {
        match self {
            Self::CyberCyan => "cyber-cyan",
            Self::NeoTokyo => "neo-tokyo",
            Self::AcidGrid => "acid-grid",
            Self::CrimsonNoir => "crimson-noir",
            Self::Custom => "custom",
        }
    }

    pub(crate) fn from_config_value(value: &str) -> Option<Self> {
        match value {
            "cyber-cyan" => Some(Self::CyberCyan),
            "neo-tokyo" => Some(Self::NeoTokyo),
            "acid-grid" => Some(Self::AcidGrid),
            "crimson-noir" => Some(Self::CrimsonNoir),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ThemeColors {
    pub(crate) background: Color,
    pub(crate) surface: Color,
    pub(crate) foreground: Color,
    pub(crate) muted: Color,
    pub(crate) primary: Color,
    pub(crate) accent: Color,
    pub(crate) secondary: Color,
    pub(crate) border: Color,
    pub(crate) focus: Color,
    pub(crate) selection: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) disabled: Color,
    pub(crate) progress: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Theme {
    pub(crate) id: ThemeId,
    name: Cow<'static, str>,
    short_label: Cow<'static, str>,
    pub(crate) colors: ThemeColors,
}

impl Theme {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn short_label(&self) -> &str {
        &self.short_label
    }

    pub(crate) fn safe_fallback() -> Self {
        Self {
            id: ThemeId::CyberCyan,
            name: Cow::Borrowed("Safe Fallback"),
            short_label: Cow::Borrowed("Safe"),
            colors: ThemeColors {
                background: rgb(28, 30, 33),
                surface: rgb(43, 46, 51),
                foreground: rgb(245, 246, 248),
                muted: rgb(172, 178, 187),
                primary: rgb(112, 177, 255),
                accent: rgb(200, 160, 255),
                secondary: rgb(67, 71, 77),
                border: rgb(104, 110, 119),
                focus: rgb(132, 193, 255),
                selection: rgb(75, 81, 89),
                success: rgb(109, 216, 151),
                warning: rgb(255, 210, 112),
                error: rgb(255, 128, 128),
                disabled: rgb(137, 143, 152),
                progress: rgb(112, 177, 255),
            },
        }
    }

    pub(crate) fn parse_custom(source: &str) -> Result<Self, ThemeParseError> {
        let document = source
            .parse::<DocumentMut>()
            .map_err(|_| ThemeParseError::InvalidDocument)?;
        if document.get("version").and_then(Item::as_integer) != Some(1) {
            return Err(ThemeParseError::UnsupportedVersion);
        }
        let name = document
            .get("name")
            .and_then(Item::as_str)
            .ok_or(ThemeParseError::InvalidName)?
            .trim();
        if name.is_empty() || name.chars().count() > 48 || name.chars().any(char::is_control) {
            return Err(ThemeParseError::InvalidName);
        }
        let colors = document
            .get("colors")
            .and_then(Item::as_table)
            .ok_or(ThemeParseError::MissingColors)?;
        let color = |key| {
            colors
                .get(key)
                .and_then(Item::as_str)
                .ok_or(ThemeParseError::MissingColor)
                .and_then(parse_hex_color)
        };
        let colors = ThemeColors {
            background: color("background")?,
            surface: color("surface")?,
            foreground: color("foreground")?,
            muted: color("muted")?,
            primary: color("primary")?,
            accent: color("accent")?,
            secondary: color("secondary")?,
            border: color("border")?,
            focus: color("focus")?,
            selection: color("selection")?,
            success: color("success")?,
            warning: color("warning")?,
            error: color("error")?,
            disabled: color("disabled")?,
            progress: color("progress")?,
        };
        if !valid_contrast(colors) {
            return Err(ThemeParseError::LowContrast);
        }

        Ok(Self {
            id: ThemeId::Custom,
            name: Cow::Owned(name.to_owned()),
            short_label: Cow::Borrowed("Custom"),
            colors,
        })
    }
}

impl Deref for Theme {
    type Target = ThemeColors;

    fn deref(&self) -> &Self::Target {
        &self.colors
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeParseError {
    InvalidDocument,
    UnsupportedVersion,
    InvalidName,
    MissingColors,
    MissingColor,
    InvalidColor,
    LowContrast,
}

impl ThemeParseError {
    pub(crate) const fn user_message(self) -> &'static str {
        match self {
            Self::InvalidDocument => "custom theme TOML could not be parsed",
            Self::UnsupportedVersion => "custom theme version is not supported",
            Self::InvalidName => "custom theme name must be 1–48 characters",
            Self::MissingColors => "custom theme is missing its colors table",
            Self::MissingColor => "custom theme is missing a required color",
            Self::InvalidColor => "theme colors must use #RRGGBB values",
            Self::LowContrast => "custom theme text colors do not meet contrast requirements",
        }
    }
}

pub(crate) const BUILTIN_THEMES: &[Theme] = &[
    Theme {
        id: ThemeId::CyberCyan,
        name: Cow::Borrowed("Cyber Cyan"),
        short_label: Cow::Borrowed("Cyan"),
        colors: ThemeColors {
            background: rgb(7, 27, 34),
            surface: rgb(10, 51, 66),
            foreground: rgb(216, 247, 255),
            muted: rgb(115, 160, 173),
            primary: rgb(0, 229, 255),
            accent: rgb(255, 57, 198),
            secondary: rgb(10, 51, 66),
            border: rgb(23, 96, 112),
            focus: rgb(0, 229, 255),
            selection: rgb(16, 64, 75),
            success: rgb(68, 226, 143),
            warning: rgb(245, 230, 52),
            error: rgb(255, 93, 100),
            disabled: rgb(85, 112, 122),
            progress: rgb(0, 196, 255),
        },
    },
    Theme {
        id: ThemeId::NeoTokyo,
        name: Cow::Borrowed("Neo Tokyo"),
        short_label: Cow::Borrowed("Neo"),
        colors: ThemeColors {
            background: rgb(26, 8, 44),
            surface: rgb(37, 16, 58),
            foreground: rgb(242, 231, 255),
            muted: rgb(178, 153, 198),
            primary: rgb(168, 85, 247),
            accent: rgb(255, 53, 167),
            secondary: rgb(255, 148, 46),
            border: rgb(105, 61, 134),
            focus: rgb(255, 53, 167),
            selection: rgb(56, 27, 82),
            success: rgb(102, 224, 164),
            warning: rgb(255, 179, 80),
            error: rgb(255, 92, 130),
            disabled: rgb(125, 104, 142),
            progress: rgb(168, 85, 247),
        },
    },
    Theme {
        id: ThemeId::AcidGrid,
        name: Cow::Borrowed("Acid Grid"),
        short_label: Cow::Borrowed("Acid"),
        colors: ThemeColors {
            background: rgb(11, 23, 18),
            surface: rgb(19, 44, 31),
            foreground: rgb(232, 255, 226),
            muted: rgb(153, 184, 148),
            primary: rgb(181, 255, 0),
            accent: rgb(0, 229, 229),
            secondary: rgb(245, 230, 52),
            border: rgb(72, 117, 57),
            focus: rgb(0, 229, 229),
            selection: rgb(31, 68, 41),
            success: rgb(140, 255, 57),
            warning: rgb(245, 230, 52),
            error: rgb(255, 90, 75),
            disabled: rgb(104, 134, 94),
            progress: rgb(0, 229, 229),
        },
    },
    Theme {
        id: ThemeId::CrimsonNoir,
        name: Cow::Borrowed("Crimson Noir"),
        short_label: Cow::Borrowed("Crimson"),
        colors: ThemeColors {
            background: rgb(25, 8, 16),
            surface: rgb(51, 15, 30),
            foreground: rgb(255, 230, 238),
            muted: rgb(190, 143, 160),
            primary: rgb(255, 40, 93),
            accent: rgb(187, 107, 217),
            secondary: rgb(88, 32, 58),
            border: rgb(111, 45, 67),
            focus: rgb(255, 40, 93),
            selection: rgb(67, 22, 39),
            success: rgb(80, 220, 143),
            warning: rgb(255, 190, 90),
            error: rgb(255, 40, 93),
            disabled: rgb(137, 99, 112),
            progress: rgb(187, 107, 217),
        },
    },
];

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}

fn parse_hex_color(value: &str) -> Result<Color, ThemeParseError> {
    let Some(hex) = value.strip_prefix('#') else {
        return Err(ThemeParseError::InvalidColor);
    };
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ThemeParseError::InvalidColor);
    }
    let channel = |start| {
        u8::from_str_radix(&hex[start..start + 2], 16).map_err(|_| ThemeParseError::InvalidColor)
    };
    Ok(rgb(channel(0)?, channel(2)?, channel(4)?))
}

fn valid_contrast(colors: ThemeColors) -> bool {
    contrast_ratio(colors.foreground, colors.background) >= 4.5
        && contrast_ratio(colors.muted, colors.background) >= 3.0
        && contrast_ratio(colors.primary, colors.background) >= 3.0
        && contrast_ratio(colors.accent, colors.background) >= 3.0
        && contrast_ratio(colors.success, colors.background) >= 3.0
        && contrast_ratio(colors.warning, colors.background) >= 3.0
        && contrast_ratio(colors.error, colors.background) >= 3.0
        && contrast_ratio(colors.focus, colors.background) >= 3.0
        && contrast_ratio(colors.foreground, colors.selection) >= 4.5
}

fn contrast_ratio(left: Color, right: Color) -> f64 {
    let high = luminance(left).max(luminance(right));
    let low = luminance(left).min(luminance(right));
    (high + 0.05) / (low + 0.05)
}

fn luminance(color: Color) -> f64 {
    let Color::Rgb(red, green, blue) = color else {
        return 0.0;
    };
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_THEME: &str = r##"
version = 1
name = "Sakura Night"

[colors]
background = "#071B22"
surface = "#0A3342"
foreground = "#D8F7FF"
muted = "#73A0AD"
primary = "#00E5FF"
accent = "#FF39C6"
secondary = "#0A3342"
border = "#176070"
focus = "#00E5FF"
selection = "#10404B"
success = "#44E28F"
warning = "#F5E634"
error = "#FF5D64"
disabled = "#55707A"
progress = "#00C4FF"
"##;

    #[test]
    fn custom_theme_parses_versioned_semantic_colors() {
        let theme = Theme::parse_custom(VALID_THEME).unwrap();

        assert_eq!(theme.id, ThemeId::Custom);
        assert_eq!(theme.name(), "Sakura Night");
        assert_eq!(theme.colors.primary, Color::Rgb(0, 229, 255));
        assert_eq!(theme.colors.foreground, Color::Rgb(216, 247, 255));
    }

    #[test]
    fn custom_theme_rejects_bad_hex_and_low_contrast_text() {
        let malformed = VALID_THEME.replace("#00E5FF", "#00E5FG");
        assert!(Theme::parse_custom(&malformed).is_err());

        let low_contrast =
            VALID_THEME.replace("foreground = \"#D8F7FF\"", "foreground = \"#071B22\"");
        assert!(Theme::parse_custom(&low_contrast).is_err());
    }

    #[test]
    fn built_in_palettes_meet_the_custom_theme_contrast_contract() {
        for theme in BUILTIN_THEMES {
            assert!(valid_contrast(theme.colors), "{}", theme.name());
        }
        assert!(valid_contrast(Theme::safe_fallback().colors));
    }
}
