//! Highlight attribute resolution, decoration styles and icon descriptors
//! (spec section 5.13), ported from todo-tree's `attributes.js`,
//! `highlights.js` and `icons.js`.

use crate::colours::{apply_opacity, complementary, is_hex, is_named, is_rgb, is_theme, is_valid};
use crate::settings::{Attributes, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A colour for the client: a theme colour id or a CSS colour string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Colour {
    Theme(String),
    Css(String),
}

/// What the client renders as an icon. SVG rendering and the octicon name
/// check happen on the client, which owns the octicon set; an unknown
/// octicon name falls back to `check` there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum IconDescriptor {
    /// A codicon; `colour` is a theme colour id when one was given.
    Codicon {
        name: String,
        colour: Option<String>,
    },
    /// An octicon rendered in `colour`.
    Octicon { name: String, colour: String },
    /// todo-tree's own icon, outline or filled.
    TodoTree { filled: bool, colour: String },
    /// The check-circle icon in `colour`.
    Check { colour: String },
    /// The bundled green default icon.
    Default,
    /// VS Code's folder icon from the file icon theme.
    Folder,
    /// VS Code's file icon from the file icon theme, for `resourceUri`.
    File,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationStyle {
    pub color: Option<Colour>,
    pub background_color: Option<Colour>,
    pub overview_ruler_color: Option<Colour>,
    pub overview_ruler_lane: Option<u32>,
    pub border_radius: String,
    pub font_style: String,
    pub font_weight: String,
    pub text_decoration: String,
    pub is_whole_line: bool,
    pub gutter_icon: Option<IconDescriptor>,
}

/// Attribute lookup for one key (a tag, group, sub-tag or untagged match).
pub struct Resolver<'a> {
    settings: &'a Settings,
}

impl<'a> Resolver<'a> {
    pub fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    /// todo-tree's `getAttribute`: an exact `customHighlight` entry, then
    /// `defaultHighlight` unless `ignore_default`.
    fn attr<T: Clone>(
        &self,
        key: &str,
        get: impl Fn(&Attributes) -> Option<T>,
        ignore_default: bool,
    ) -> Option<T> {
        if let Some(v) = self.settings.custom(key).and_then(&get) {
            return Some(v);
        }
        if ignore_default {
            None
        } else {
            get(&self.settings.highlights.default_highlight)
        }
    }

    fn scheme(&self, key: &str, colours: &[String]) -> Option<String> {
        let i = self.settings.tags().iter().position(|t| t == key)?;
        if colours.is_empty() {
            None
        } else {
            Some(colours[i % colours.len()].clone())
        }
    }

    pub fn foreground(&self, key: &str) -> Option<String> {
        let s = self.settings.highlights.use_colour_scheme;
        self.attr(key, |a| a.foreground.clone(), s).or_else(|| {
            s.then(|| self.scheme(key, &self.settings.highlights.foreground_colour_scheme))
                .flatten()
        })
    }

    pub fn background(&self, key: &str) -> Option<String> {
        let s = self.settings.highlights.use_colour_scheme;
        self.attr(key, |a| a.background.clone(), s).or_else(|| {
            s.then(|| self.scheme(key, &self.settings.highlights.background_colour_scheme))
                .flatten()
        })
    }

    pub fn icon_colour(&self, key: &str) -> String {
        let s = self.settings.highlights.use_colour_scheme;
        self.attr(key, |a| a.icon_color.clone(), false)
            .or_else(|| self.attr(key, |a| a.icon_colour.clone(), s))
            .or_else(|| {
                s.then(|| self.scheme(key, &self.settings.highlights.background_colour_scheme))
                    .flatten()
            })
            .or_else(|| self.attr(key, |a| a.foreground.clone(), s))
            .or_else(|| self.attr(key, |a| a.background.clone(), s))
            .unwrap_or_else(|| "green".to_string())
    }

    /// The `icon` attribute, with `defaultHighlight` applying.
    pub fn icon_name(&self, key: &str) -> Option<String> {
        self.attr(key, |a| a.icon.clone(), false)
    }

    pub fn kind(&self, key: &str) -> Option<String> {
        self.attr(key, |a| a.kind.clone(), false)
    }

    pub fn flag(&self, key: &str, get: fn(&Attributes) -> Option<bool>) -> bool {
        self.attr(key, get, false).unwrap_or(false)
    }

    pub fn icon(&self, key: &str) -> IconDescriptor {
        let colour = self.icon_colour(key);
        let not_theme = |c: &str| {
            if is_theme(c) {
                "green".to_string()
            } else {
                c.to_string()
            }
        };
        match self.attr(key, |a| a.icon.clone(), false) {
            Some(name) if name == "todo-tree" || name == "todo-tree-filled" => {
                IconDescriptor::TodoTree {
                    filled: name.ends_with("filled"),
                    colour: not_theme(&colour),
                }
            }
            Some(name) if name.trim().starts_with("$(") => {
                let rest = name.trim().strip_prefix("$(").unwrap_or_default();
                // todo-tree's `substr(2, length - 3)`: the last character goes
                // whether or not it is the closing `)`.
                let name = rest.strip_suffix(')').unwrap_or_else(|| {
                    let mut chars = rest.chars();
                    chars.next_back();
                    chars.as_str()
                });
                IconDescriptor::Codicon {
                    name: name.to_string(),
                    colour: is_theme(&colour).then_some(colour),
                }
            }
            Some(name) if !name.is_empty() => IconDescriptor::Octicon {
                name,
                colour: not_theme(&colour),
            },
            _ if is_hex(&colour) || is_rgb(&colour) || is_named(&colour) => {
                IconDescriptor::Check { colour }
            }
            _ => IconDescriptor::Default,
        }
    }

    fn lane(&self, key: &str) -> Option<u32> {
        match self.attr(key, |a| a.ruler_lane.clone(), false) {
            None => Some(4),
            Some(Value::Number(n)) => n.as_u64().map(|n| n as u32),
            Some(Value::String(s)) => match s.trim().parse::<u32>() {
                Ok(n) => Some(n),
                Err(_) => match s.to_lowercase().as_str() {
                    "left" => Some(1),
                    "center" => Some(2),
                    "right" => Some(4),
                    "full" => Some(7),
                    _ => None,
                },
            },
            Some(_) => None,
        }
    }

    fn colour(value: &str, fallback: &str) -> Colour {
        let lower = value.to_lowercase();
        if lower.contains("foreground") || lower.contains("background") {
            Colour::Theme(value.to_string())
        } else if !is_valid(value) {
            Colour::Theme(fallback.to_string())
        } else {
            Colour::Css(value.to_string())
        }
    }

    /// todo-tree's `getDecoration`, with the auto-contrast fix.
    pub fn style(&self, key: &str) -> DecorationStyle {
        let fg = self
            .foreground(key)
            .map(|c| Self::colour(&c, "editor.foreground"));
        let opacity = self.attr(key, |a| a.opacity, false).unwrap_or(100.0);
        let bg = self
            .background(key)
            .map(|c| match Self::colour(&c, "editor.background") {
                Colour::Css(css) => Colour::Css(apply_opacity(&css, opacity)),
                theme => theme,
            });
        let fg = fg.or_else(|| match &bg {
            Some(Colour::Css(css)) => complementary(css).map(|c| Colour::Css(c.to_string())),
            _ => None,
        });
        let (fg, bg) = match (fg, bg) {
            (None, None) => (
                Some(Colour::Theme("editor.background".into())),
                Some(Colour::Theme("editor.foreground".into())),
            ),
            other => other,
        };
        let lane = self.lane(key);
        let ruler = lane.map(|_| {
            let ruler_opacity = self.attr(key, |a| a.ruler_opacity, false).unwrap_or(100.0);
            match self.attr(key, |a| a.ruler_colour.clone(), false) {
                Some(c) if is_theme(&c) => Colour::Theme(c),
                Some(c) => Colour::Css(apply_opacity(&c, ruler_opacity)),
                None => match &bg {
                    Some(Colour::Css(c)) => Colour::Css(apply_opacity(c, ruler_opacity)),
                    Some(theme) => theme.clone(),
                    None => Colour::Theme("editor.foreground".into()),
                },
            }
        });
        let gutter = self.flag(key, |a| a.gutter_icon).then(|| self.icon(key));
        DecorationStyle {
            color: fg,
            background_color: bg,
            overview_ruler_color: ruler,
            overview_ruler_lane: lane,
            border_radius: self
                .attr(key, |a| a.border_radius.clone(), false)
                .unwrap_or_else(|| "0.2em".into()),
            font_style: self
                .attr(key, |a| a.font_style.clone(), false)
                .unwrap_or_else(|| "normal".into()),
            font_weight: self
                .attr(key, |a| a.font_weight.clone(), false)
                .unwrap_or_else(|| "normal".into()),
            text_decoration: self
                .attr(key, |a| a.text_decoration.clone(), false)
                .unwrap_or_default(),
            is_whole_line: self.kind(key).as_deref() == Some("whole-line"),
            gutter_icon: gutter,
        }
    }
}

/// Configuration warnings for invalid colours (todo-tree's `validateColours`).
pub fn colour_warnings(settings: &Settings) -> Vec<String> {
    let mut bad = Vec::new();
    let mut check = |prefix: &str, a: &Attributes| {
        for (name, v) in [
            ("foreground", &a.foreground),
            ("background", &a.background),
            ("iconColour", &a.icon_colour),
            ("rulerColour", &a.ruler_colour),
        ] {
            if let Some(v) = v {
                if !is_valid(v) {
                    bad.push(format!("{prefix}.{name} ({v})"));
                }
            }
        }
    };
    check("defaultHighlight", &settings.highlights.default_highlight);
    for (k, a) in &settings.highlights.custom_highlight {
        check(&format!("customHighlight.{k}"), a);
    }
    if bad.is_empty() {
        Vec::new()
    } else {
        vec![format!("Invalid colour settings: {}", bad.join(", "))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Attributes;

    fn settings(f: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = Settings::default();
        f(&mut s);
        s
    }

    #[test]
    fn default_style_inverts_editor_colours() {
        let s = Settings::default();
        let st = Resolver::new(&s).style("TODO");
        assert_eq!(st.color, Some(Colour::Theme("editor.background".into())));
        assert_eq!(
            st.background_color,
            Some(Colour::Theme("editor.foreground".into()))
        );
        assert_eq!(st.overview_ruler_lane, Some(4));
        assert_eq!(
            st.overview_ruler_color,
            Some(Colour::Theme("editor.foreground".into()))
        );
        assert_eq!(st.border_radius, "0.2em");
        assert!(!st.is_whole_line && st.gutter_icon.is_none());
    }

    #[test]
    fn custom_then_default_then_builtin() {
        let s = settings(|s| {
            s.highlights.default_highlight = Attributes {
                foreground: Some("red".into()),
                font_weight: Some("bold".into()),
                ..Default::default()
            };
            s.highlights.custom_highlight.insert(
                "TODO".into(),
                Attributes {
                    foreground: Some("blue".into()),
                    ..Default::default()
                },
            );
        });
        let r = Resolver::new(&s);
        assert_eq!(r.foreground("TODO").as_deref(), Some("blue"));
        assert_eq!(r.foreground("FIXME").as_deref(), Some("red"));
        assert_eq!(r.style("TODO").font_weight, "bold");
        assert_eq!(
            r.foreground("TODOS").as_deref(),
            Some("red"),
            "keys match exactly"
        );
    }

    #[test]
    fn colour_scheme_by_tag_position_and_not_for_groups() {
        let s = settings(|s| {
            s.highlights.use_colour_scheme = true;
            s.highlights.default_highlight.background = Some("pink".into());
        });
        let r = Resolver::new(&s);
        assert_eq!(r.background("BUG").as_deref(), Some("red"));
        assert_eq!(r.background("TODO").as_deref(), Some("green"));
        assert_eq!(r.foreground("TODO").as_deref(), Some("white"));
        assert_eq!(
            r.background("GROUP"),
            None,
            "defaultHighlight ignored for colours with a scheme"
        );
        assert_eq!(r.icon_colour("BUG"), "red");
    }

    #[test]
    fn background_opacity_and_auto_contrast() {
        let s = settings(|s| {
            s.highlights.custom_highlight.insert(
                "TODO".into(),
                Attributes {
                    background: Some("#ffff00".into()),
                    opacity: Some(50.0),
                    ..Default::default()
                },
            );
        });
        let st = Resolver::new(&s).style("TODO");
        assert_eq!(
            st.background_color,
            Some(Colour::Css("rgba(255,255,0,0.5)".into()))
        );
        assert_eq!(st.color, Some(Colour::Css("#000000".into())));
        assert_eq!(
            st.overview_ruler_color,
            Some(Colour::Css("rgba(255,255,0,0.5)".into()))
        );
    }

    #[test]
    fn theme_and_invalid_colours() {
        let s = settings(|s| {
            s.highlights.custom_highlight.insert(
                "A".into(),
                Attributes {
                    foreground: Some("errorForeground".into()),
                    background: Some("nonsense".into()),
                    ..Default::default()
                },
            );
        });
        let st = Resolver::new(&s).style("A");
        assert_eq!(st.color, Some(Colour::Theme("errorForeground".into())));
        assert_eq!(
            st.background_color,
            Some(Colour::Theme("editor.background".into()))
        );
        assert_eq!(
            colour_warnings(&s),
            vec!["Invalid colour settings: customHighlight.A.background (nonsense)"]
        );
    }

    #[test]
    fn ruler_lanes() {
        let lane = |v: Value| {
            let s = settings(|s| {
                s.highlights.custom_highlight.insert(
                    "T".into(),
                    Attributes {
                        ruler_lane: Some(v),
                        ..Default::default()
                    },
                );
            });
            Resolver::new(&s).style("T").overview_ruler_lane
        };
        assert_eq!(lane(Value::String("left".into())), Some(1));
        assert_eq!(lane(Value::String("FULL".into())), Some(7));
        assert_eq!(lane(Value::String("none".into())), None);
        assert_eq!(lane(Value::String("2".into())), Some(2));
        assert_eq!(lane(serde_json::json!(3)), Some(3));
    }

    #[test]
    fn icons() {
        let s = settings(|s| {
            let h = &mut s.highlights.custom_highlight;
            h.insert(
                "A".into(),
                Attributes {
                    icon: Some("$(bug)".into()),
                    icon_colour: Some("errorForeground".into()),
                    ..Default::default()
                },
            );
            h.insert(
                "B".into(),
                Attributes {
                    icon: Some("todo-tree-filled".into()),
                    icon_colour: Some("badge.background".into()),
                    ..Default::default()
                },
            );
            h.insert(
                "C".into(),
                Attributes {
                    background: Some("#123456".into()),
                    ..Default::default()
                },
            );
        });
        let r = Resolver::new(&s);
        assert_eq!(
            r.icon("A"),
            IconDescriptor::Codicon {
                name: "bug".into(),
                colour: Some("errorForeground".into())
            }
        );
        assert_eq!(
            r.icon("B"),
            IconDescriptor::TodoTree {
                filled: true,
                colour: "green".into()
            }
        );
        assert_eq!(
            r.icon("BUG"),
            IconDescriptor::Octicon {
                name: "bug".into(),
                colour: "green".into()
            }
        );
        assert_eq!(
            r.icon("C"),
            IconDescriptor::Check {
                colour: "#123456".into()
            }
        );
        let plain = Settings {
            highlights: crate::settings::Highlights {
                custom_highlight: Default::default(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            Resolver::new(&plain).icon("TODO"),
            IconDescriptor::Check {
                colour: "green".into()
            }
        );
    }

    #[test]
    fn malformed_codicons_do_not_panic() {
        let codicon = |icon: &str| {
            let s = settings(|s| {
                s.highlights.custom_highlight.insert(
                    "A".into(),
                    Attributes {
                        icon: Some(icon.into()),
                        ..Default::default()
                    },
                );
            });
            match Resolver::new(&s).icon("A") {
                IconDescriptor::Codicon { name, .. } => name,
                other => panic!("{other:?}"),
            }
        };
        // todo-tree drops the last character whether or not it is `)`.
        assert_eq!(codicon("$(é"), "");
        assert_eq!(codicon("$(ab"), "a");
        assert_eq!(codicon("$(éé)"), "éé");
        assert_eq!(codicon(" $("), "");
    }
}
