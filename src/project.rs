use crate::widget::Widget;
use egui::{Vec2, vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Project {
    pub(crate) widgets: Vec<Widget>,
    pub(crate) canvas_size: Vec2,
    pub(crate) panel_top_enabled: bool,
    pub(crate) panel_bottom_enabled: bool,
    pub(crate) panel_left_enabled: bool,
    pub(crate) panel_right_enabled: bool,
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
        }
    }
}

