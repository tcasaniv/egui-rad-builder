use crate::widget::{Align, Justify, LayoutMode, ResponsiveLayout, Widget};
use egui::{Vec2, vec2};
use serde::{Deserialize, Serialize};

/// Screen format / device preview presets for responsive design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum ScreenPreset {
    #[default]
    Desktop,
    MobilePortrait,
    MobileLandscape,
    TabletPortrait,
    TabletLandscape,
    Custom,
}

#[allow(dead_code)]
impl ScreenPreset {
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::Desktop         => "Desktop (1280x800)",
            Self::MobilePortrait  => "Mobile Portrait (390x844)",
            Self::MobileLandscape => "Mobile Landscape (844x390)",
            Self::TabletPortrait  => "Tablet Portrait (768x1024)",
            Self::TabletLandscape => "Tablet Landscape (1024x768)",
            Self::Custom          => "Custom",
        }
    }

    pub(crate) const fn all() -> &'static [ScreenPreset] {
        &[
            Self::Desktop,
            Self::MobilePortrait,
            Self::MobileLandscape,
            Self::TabletPortrait,
            Self::TabletLandscape,
            Self::Custom,
        ]
    }

    /// Returns the target canvas size for this preset, if fixed.
    pub(crate) fn dimensions(self) -> Option<Vec2> {
        match self {
            Self::Desktop         => Some(vec2(1280.0, 800.0)),
            Self::MobilePortrait  => Some(vec2(390.0, 844.0)),
            Self::MobileLandscape => Some(vec2(844.0, 390.0)),
            Self::TabletPortrait  => Some(vec2(768.0, 1024.0)),
            Self::TabletLandscape => Some(vec2(1024.0, 768.0)),
            Self::Custom          => None,
        }
    }
}

fn default_root_layout_gap() -> f32 {
    16.0
}

fn default_root_layout_cols() -> usize {
    2
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Project {
    pub(crate) widgets: Vec<Widget>,
    pub(crate) canvas_size: Vec2,
    pub(crate) panel_top_enabled: bool,
    pub(crate) panel_bottom_enabled: bool,
    pub(crate) panel_left_enabled: bool,
    pub(crate) panel_right_enabled: bool,
    #[serde(default)]
    pub(crate) screen_preset: ScreenPreset,

    // ── Root Screen Container Layout (HTML/CSS Flex & Grid Flow) ───────────
    /// Layout mode for the root canvas viewport. CSS: `display: flex / grid / block`.
    #[serde(default)]
    pub(crate) root_layout_mode: LayoutMode,
    /// Spacing between root widgets in auto-layout modes (px). CSS: `gap`.
    #[serde(default = "default_root_layout_gap")]
    pub(crate) root_layout_gap: f32,
    /// Number of columns when `root_layout_mode == LayoutMode::Grid`.
    #[serde(default = "default_root_layout_cols")]
    pub(crate) root_layout_cols: usize,
    /// Cross-axis alignment for root widgets. CSS: `align-items`.
    #[serde(default)]
    pub(crate) root_layout_align: Align,
    /// Main-axis distribution for root widgets. CSS: `justify-content`.
    #[serde(default)]
    pub(crate) root_layout_justify: Justify,
    /// Screen inner padding [top, right, bottom, left] in px. CSS: `padding`.
    #[serde(default)]
    pub(crate) root_layout_padding: [f32; 4],
    /// Media-query style responsive layout adaptation rule for root canvas.
    #[serde(default)]
    pub(crate) root_responsive_layout: ResponsiveLayout,
}

impl Project {
    /// Returns the direct child widgets belonging to `parent_id`, sorted by `z` descending.
    pub(crate) fn children_of(&self, parent_id: crate::widget::WidgetId) -> Vec<&Widget> {
        let mut list: Vec<&Widget> = self
            .widgets
            .iter()
            .filter(|w| w.parent == Some(parent_id))
            .collect();
        list.sort_by_key(|w| std::cmp::Reverse(w.z));
        list
    }

    /// Returns `true` if `candidate_child` is a descendant of `ancestor_id` (prevent circular parenting).
    pub(crate) fn is_descendant_of(
        &self,
        candidate_child: crate::widget::WidgetId,
        ancestor_id: crate::widget::WidgetId,
    ) -> bool {
        let mut curr = candidate_child;
        while let Some(w) = self.widgets.iter().find(|w| w.id == curr) {
            if let Some(p) = w.parent {
                if p == ancestor_id {
                    return true;
                }
                curr = p;
            } else {
                break;
            }
        }
        false
    }

    /// Recursively collects all descendant IDs for a given widget.
    pub(crate) fn get_descendants(&self, root_id: crate::widget::WidgetId) -> Vec<crate::widget::WidgetId> {
        let mut descendants = Vec::new();
        let mut queue = vec![root_id];
        while let Some(parent) = queue.pop() {
            for child in self.widgets.iter().filter(|w| w.parent == Some(parent)) {
                descendants.push(child.id);
                queue.push(child.id);
            }
        }
        descendants
    }
}

impl Default for Project {
    fn default() -> Self {
        Self {
            widgets: Vec::new(),
            canvas_size: vec2(700.0, 600.0),
            panel_top_enabled: false,
            panel_bottom_enabled: false,
            panel_left_enabled: false,
            panel_right_enabled: false,
            screen_preset: ScreenPreset::Desktop,
            root_layout_mode: LayoutMode::Free,
            root_layout_gap: 16.0,
            root_layout_cols: 2,
            root_layout_align: Align::Start,
            root_layout_justify: Justify::Start,
            root_layout_padding: [16.0, 16.0, 16.0, 16.0],
            root_responsive_layout: ResponsiveLayout::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_root_layout_defaults_and_serde() {
        let proj = Project::default();
        assert_eq!(proj.root_layout_mode, LayoutMode::Free);
        assert_eq!(proj.root_layout_gap, 16.0);
        assert_eq!(proj.root_layout_cols, 2);
        assert_eq!(proj.root_layout_padding, [16.0, 16.0, 16.0, 16.0]);

        // Verify JSON backward compat without root layout fields
        let legacy_json = r#"{
            "widgets": [],
            "canvas_size": [800.0, 600.0],
            "panel_top_enabled": false,
            "panel_bottom_enabled": false,
            "panel_left_enabled": false,
            "panel_right_enabled": false
        }"#;

        let parsed: Project = serde_json::from_str(legacy_json).expect("should parse legacy project JSON");
        assert_eq!(parsed.root_layout_mode, LayoutMode::Free);
        assert_eq!(parsed.root_layout_gap, 16.0);
        assert_eq!(parsed.root_layout_cols, 2);
    }
}

