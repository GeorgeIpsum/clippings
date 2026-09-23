//! Colour helpers ported from todo-tree's `utils.js` and `highlights.js`.

use crate::colour_names::{NAMED_COLOURS, THEME_COLOURS};
use regex::Regex;
use std::sync::OnceLock;

fn rgb_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*(\d+(?:\.\d+)?))?\)$").unwrap()
    })
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, with or without the `#`, as todo-tree accepts.
pub fn is_hex(colour: &str) -> bool {
    let without = colour.strip_prefix('#').unwrap_or(colour);
    let first = without.split(' ').next().unwrap_or("");
    let hex: String = first.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    hex.len() == without.len() && matches!(hex.len(), 3 | 4 | 6 | 8)
}

pub fn is_rgb(colour: &str) -> bool {
    rgb_regex().is_match(colour)
}

fn named_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| NAMED_COLOURS.into_iter().collect())
}

fn theme_set() -> &'static std::collections::HashSet<&'static str> {
    static SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| THEME_COLOURS.into_iter().collect())
}

pub fn is_named(colour: &str) -> bool {
    named_set().contains(colour.to_lowercase().as_str())
}

pub fn is_theme(colour: &str) -> bool {
    theme_set().contains(colour)
}

pub fn is_valid(colour: &str) -> bool {
    !colour.is_empty() && (is_named(colour) || is_theme(colour) || is_hex(colour) || is_rgb(colour))
}

/// A number formatted as JavaScript's `String(number)` does for these values.
pub fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn component(digits: &str) -> u32 {
    let d = if digits.len() == 1 {
        format!("{digits}{digits}")
    } else {
        digits.to_string()
    };
    u32::from_str_radix(&d, 16).unwrap_or(0)
}

/// todo-tree's `hexToRgba`: `opacity` is a percentage; hex alpha overrides it.
pub fn hex_to_rgba(hex: &str, opacity: f64) -> String {
    let hex = hex.replacen('#', "", 1);
    let rgb_len = if hex.len() == 3 || hex.len() == 4 {
        3
    } else {
        6
    };
    let rgb = &hex[..rgb_len.min(hex.len())];
    let n = rgb.len() / 3;
    let (r, g, b) = (
        component(&rgb[..n]),
        component(&rgb[n..2 * n]),
        component(&rgb[2 * n..3 * n]),
    );
    let mut opacity = opacity;
    if hex.len() == 4 || hex.len() == 8 {
        let a = &hex[3 * hex.len() / 4..];
        opacity = (component(a) as f64 * 100.0 / 255.0).trunc();
    }
    format!("rgba({r},{g},{b},{})", js_number(opacity / 100.0))
}

/// todo-tree's `setRgbAlpha`.
pub fn set_rgb_alpha(rgb: &str, alpha: f64) -> String {
    match rgb_regex().captures(rgb) {
        Some(c) => format!("rgba({},{},{},{})", &c[1], &c[2], &c[3], js_number(alpha)),
        None => rgb.to_string(),
    }
}

/// todo-tree's `applyOpacity`: hex becomes `rgba(...)`; rgb gets an alpha
/// unless the opacity is exactly 100; anything else is unchanged.
pub fn apply_opacity(colour: &str, opacity: f64) -> String {
    if is_hex(colour) {
        hex_to_rgba(
            colour,
            if opacity < 1.0 {
                opacity * 100.0
            } else {
                opacity
            },
        )
    } else if is_rgb(colour) {
        if opacity != 100.0 {
            set_rgb_alpha(
                colour,
                if opacity > 1.0 {
                    opacity / 100.0
                } else {
                    opacity
                },
            )
        } else {
            colour.to_string()
        }
    } else {
        colour.to_string()
    }
}

/// Red, green and blue of a hex or `rgb(a)` colour.
pub fn rgb_components(colour: &str) -> Option<(u32, u32, u32)> {
    if is_hex(colour) {
        let hex = colour.trim_start_matches('#');
        let rgb = &hex[..if hex.len() == 3 || hex.len() == 4 {
            3
        } else {
            6
        }];
        let n = rgb.len() / 3;
        return Some((
            component(&rgb[..n]),
            component(&rgb[n..2 * n]),
            component(&rgb[2 * n..]),
        ));
    }
    let c = rgb_regex().captures(colour)?;
    Some((c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?))
}

/// Black or white, whichever contrasts with `colour` (luminance threshold 0.179).
pub fn complementary(colour: &str) -> Option<&'static str> {
    let (r, g, b) = rgb_components(colour)?;
    let lin = |v: u32| {
        let c = v as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    Some(if l > 0.179 { "#000000" } else { "#ffffff" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert!(is_hex("#ff0000") && is_hex("f00") && is_hex("#ff000080") && is_hex("bad"));
        assert!(!is_hex("#ff000") && !is_hex("red") && !is_hex("#ggg"));
        assert!(is_rgb("rgb(1, 2, 3)") && is_rgb("RGBA(1,2,3,0.5)") && !is_rgb("rgb(1,2)"));
        assert!(is_named("Red") && !is_named("reddish"));
        assert!(is_theme("editor.foreground") && !is_theme("editor.nope"));
        assert!(is_valid("peachpuff") && !is_valid("") && !is_valid("nope"));
    }

    #[test]
    fn opacity_matches_todo_tree() {
        assert_eq!(apply_opacity("#ff0000", 100.0), "rgba(255,0,0,1)");
        assert_eq!(apply_opacity("#f00", 50.0), "rgba(255,0,0,0.5)");
        assert_eq!(apply_opacity("#f00", 0.25), "rgba(255,0,0,0.25)");
        assert_eq!(apply_opacity("#ff000080", 100.0), "rgba(255,0,0,0.5)");
        assert_eq!(apply_opacity("rgb(1,2,3)", 100.0), "rgb(1,2,3)");
        assert_eq!(apply_opacity("rgb(1,2,3)", 40.0), "rgba(1,2,3,0.4)");
        assert_eq!(apply_opacity("red", 40.0), "red");
    }

    #[test]
    fn contrast() {
        assert_eq!(complementary("#ffff00"), Some("#000000"));
        assert_eq!(complementary("rgba(0,0,128,0.5)"), Some("#ffffff"));
        assert_eq!(complementary("red"), None);
    }
}
