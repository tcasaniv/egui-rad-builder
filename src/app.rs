//! Application shell and code-generation configuration.
//!
//! Most builder state is intentionally kept behind the desktop application UI.
//! The public pieces in this module describe generated-code output modes and
//! the [`eframe::App`] implementation used by the binary.

use crate::{
    highlight::Highlighter,
    history::{History, HistorySnapshot},
    project::{Project, ScreenPreset},
    widget::{
        self, ActionEffect, ActionTrigger, Align, DockArea, Justify, LayoutMode,
        ResponsiveLayout, ResponsiveVisibility, SizePolicy, Widget, WidgetAction, WidgetId,
        WidgetKind, escape, snap_pos_with_grid,
    },
};
use chrono::{Datelike, NaiveDate};
use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, UiBuilder, pos2, vec2};
use egui_extras::DatePickerButton;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Output shape used when generating Rust source from the current project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CodeGenFormat {
    /// Generate one complete `main.rs`-style file.
    #[default]
    SingleFile,
    /// Generate conceptual `main.rs`, `state.rs`, and `ui.rs` sections.
    SeparateFiles,
    /// Generate only the state and UI function for embedding in an app.
    UiOnly,
}

impl CodeGenFormat {
    /// Returns the label shown in the builder settings UI.
    pub const fn display_name(&self) -> &'static str {
        match self {
            CodeGenFormat::SingleFile => "Single File",
            CodeGenFormat::SeparateFiles => "Separate Files",
            CodeGenFormat::UiOnly => "UI Function Only",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CodeGenFormat;

    #[test]
    fn test_codegen_format_display_names() {
        assert_eq!(CodeGenFormat::SingleFile.display_name(), "Single File");
        assert_eq!(
            CodeGenFormat::SeparateFiles.display_name(),
            "Separate Files"
        );
        assert_eq!(CodeGenFormat::UiOnly.display_name(), "UI Function Only");
    }

    #[test]
    fn test_codegen_format_default_is_single_file() {
        assert_eq!(CodeGenFormat::default(), CodeGenFormat::SingleFile);
    }

    #[test]
    fn test_codegen_skips_inactive_widget() {
        let mut app = super::RadBuilderApp::default();
        let mut w = crate::widget::Widget {
            id: crate::widget::WidgetId::new(1),
            kind: crate::widget::WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps::default(),
            parent: None,
        };
        w.props.text = "InactiveBtn".into();
        w.props.active = false;
        app.project.widgets.push(w);

        let code = app.generate_single_file();
        assert!(
            !code.contains("InactiveBtn"),
            "inactive widget text should not appear in codegen"
        );
    }

    #[test]
    fn test_codegen_initially_hidden_generates_show_flag() {
        let mut app = super::RadBuilderApp::default();
        let mut w = crate::widget::Widget {
            id: crate::widget::WidgetId::new(42),
            kind: crate::widget::WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps::default(),
            parent: None,
        };
        w.props.text = "SecretBtn".into();
        w.props.initially_visible = false;
        app.project.widgets.push(w);

        let code = app.generate_single_file();
        assert!(
            code.contains("show_42: bool"),
            "should have show_42 flag in state"
        );
        assert!(
            code.contains("show_42: false"),
            "should default show_42 to false"
        );
        assert!(
            code.contains("if state.show_42 {"),
            "should guard widget with if state.show_42"
        );
    }

    #[test]
    fn test_codegen_emits_tooltip() {
        let mut app = super::RadBuilderApp::default();
        let mut w = crate::widget::Widget {
            id: crate::widget::WidgetId::new(99),
            kind: crate::widget::WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps::default(),
            parent: None,
        };
        w.props.text = "HoverBtn".into();
        w.props.tooltip = "Click to save".into();
        app.project.widgets.push(w);

        let code = app.generate_single_file();
        assert!(
            code.contains(".on_hover_text(\"Click to save\")"),
            "should emit on_hover_text with tooltip text"
        );
    }

    #[test]
    fn test_reparent_widget_coordinate_conversion() {
        let mut app = super::RadBuilderApp::default();
        app.grid_size = 1.0;

        let group = crate::widget::Widget {
            id: crate::widget::WidgetId::new(1),
            kind: crate::widget::WidgetKind::Group,
            pos: egui::pos2(50.0, 50.0),
            size: egui::vec2(200.0, 150.0),
            z: 1,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps {
                text: "MyGroup".into(),
                ..Default::default()
            },
            parent: None,
        };

        let btn = crate::widget::Widget {
            id: crate::widget::WidgetId::new(2),
            kind: crate::widget::WidgetKind::Button,
            pos: egui::pos2(80.0, 100.0),
            size: egui::vec2(60.0, 24.0),
            z: 2,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps {
                text: "InsideBtn".into(),
                ..Default::default()
            },
            parent: None,
        };

        app.project.widgets.push(group);
        app.project.widgets.push(btn);

        // Reparent button into group
        app.reparent_widget(crate::widget::WidgetId::new(2), Some(crate::widget::WidgetId::new(1)));

        let reparented_btn = app.project.widgets.iter().find(|w| w.id == crate::widget::WidgetId::new(2)).unwrap();
        assert_eq!(reparented_btn.parent, Some(crate::widget::WidgetId::new(1)));
        // Group offset for non-empty title is (8.0, 26.0), so content origin is (58.0, 76.0)
        // Button was at (80.0, 100.0), so relative pos is (80 - 58, 100 - 76) = (22.0, 24.0)
        assert_eq!(reparented_btn.pos, egui::pos2(22.0, 24.0));

        // Computing abs pos should return back (80.0, 100.0)
        let abs_pos = app.compute_abs_pos(crate::widget::WidgetId::new(2));
        assert_eq!(abs_pos, egui::pos2(80.0, 100.0));

        // Cycle prevention: attempting to reparent group into button should do nothing
        app.reparent_widget(crate::widget::WidgetId::new(1), Some(crate::widget::WidgetId::new(2)));
        let group_check = app.project.widgets.iter().find(|w| w.id == crate::widget::WidgetId::new(1)).unwrap();
        assert_eq!(group_check.parent, None);

        // Unparent button back to root: pos should return to absolute (80.0, 100.0)
        app.reparent_widget(crate::widget::WidgetId::new(2), None);
        let unparented_btn = app.project.widgets.iter().find(|w| w.id == crate::widget::WidgetId::new(2)).unwrap();
        assert_eq!(unparented_btn.parent, None);
        assert_eq!(unparented_btn.pos, egui::pos2(80.0, 100.0));
    }

    #[test]
    fn test_codegen_recursive_container_emits_children() {
        let mut app = super::RadBuilderApp::default();
        let group = crate::widget::Widget {
            id: crate::widget::WidgetId::new(10),
            kind: crate::widget::WidgetKind::Group,
            pos: egui::pos2(20.0, 30.0),
            size: egui::vec2(250.0, 180.0),
            z: 1,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps {
                text: "SettingsGroup".into(),
                ..Default::default()
            },
            parent: None,
        };

        let btn = crate::widget::Widget {
            id: crate::widget::WidgetId::new(11),
            kind: crate::widget::WidgetKind::Button,
            pos: egui::pos2(15.0, 25.0),
            size: egui::vec2(90.0, 24.0),
            z: 2,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps {
                text: "SaveSettings".into(),
                ..Default::default()
            },
            parent: Some(crate::widget::WidgetId::new(10)),
        };

        app.project.widgets.push(group);
        app.project.widgets.push(btn);

        let code = app.generate_single_file();
        // The root CentralPanel iterates only root widgets, so it should not emit SaveSettings directly on canvas.min
        // SaveSettings must be emitted inside the Frame::group closure using ui.min_rect().min
        assert!(code.contains("egui::Frame::group"), "should emit Frame::group");
        assert!(code.contains("SaveSettings"), "should emit child button");
        assert!(
            code.contains("ui.min_rect().min + egui::vec2(15.0,25.0)"),
            "child button should use relative origin ui.min_rect().min"
        );
    }

    // ─── Action System ────────────────────────────────────────────────────────

    /// Helper: build a minimal single-widget project, run generate_single_file, return code.
    #[allow(dead_code)]
    fn codegen_with_action(
        kind: crate::widget::WidgetKind,
        label: &str,
        initially_visible: bool,
        action: crate::widget::WidgetAction,
    ) -> String {
        let mut app = super::RadBuilderApp::default();
        let w = crate::widget::Widget {
            id: crate::widget::WidgetId::new(1),
            kind,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps {
                text: label.into(),
                initially_visible,
                actions: vec![action],
                ..Default::default()
            },
            parent: None,
        };
        app.project.widgets.push(w);
        app.generate_single_file()
    }

    #[test]
    fn test_live_apply_action_effects() {
        use crate::widget::{ActionEffect, WidgetId, WidgetProps, WidgetKind, DockArea};
        let mut widgets: Vec<crate::widget::Widget> = vec![
            crate::widget::Widget {
                id: WidgetId::new(10),
                kind: WidgetKind::Label,
                pos: egui::pos2(0.0, 0.0),
                size: egui::vec2(100.0, 24.0),
                z: 1,
                area: DockArea::Center,
                props: WidgetProps { initially_visible: true, ..Default::default() },
                parent: None,
            },
        ];

        // ShowWidget / HideWidget / ToggleWidget
        super::RadBuilderApp::apply_action_effect(&ActionEffect::HideWidget(WidgetId::new(10)), &mut widgets);
        assert!(!widgets[0].props.initially_visible, "HideWidget should set initially_visible=false");

        super::RadBuilderApp::apply_action_effect(&ActionEffect::ShowWidget(WidgetId::new(10)), &mut widgets);
        assert!(widgets[0].props.initially_visible, "ShowWidget should set initially_visible=true");

        super::RadBuilderApp::apply_action_effect(&ActionEffect::ToggleWidget(WidgetId::new(10)), &mut widgets);
        assert!(!widgets[0].props.initially_visible, "ToggleWidget should flip to false");

        super::RadBuilderApp::apply_action_effect(&ActionEffect::ToggleWidget(WidgetId::new(10)), &mut widgets);
        assert!(widgets[0].props.initially_visible, "ToggleWidget should flip back to true");

        // SetText
        super::RadBuilderApp::apply_action_effect(
            &ActionEffect::SetText { target: WidgetId::new(10), text: "Hello".into() },
            &mut widgets,
        );
        assert_eq!(widgets[0].props.text, "Hello", "SetText should update props.text");

        // SwitchTab / OpenModal / CloseModal do not panic on unknown IDs
        super::RadBuilderApp::apply_action_effect(&ActionEffect::SwitchTab { target: WidgetId::new(99), tab_index: 2 }, &mut widgets);
        super::RadBuilderApp::apply_action_effect(&ActionEffect::OpenModal(WidgetId::new(99)), &mut widgets);
        super::RadBuilderApp::apply_action_effect(&ActionEffect::CloseModal(WidgetId::new(99)), &mut widgets);
    }

    #[test]
    fn test_codegen_button_click_toggle_widget() {
        use crate::widget::{ActionEffect, ActionTrigger, WidgetAction, WidgetId, WidgetKind, WidgetProps, DockArea};

        // Target widget that will be toggled (starts hidden)
        let mut app = super::RadBuilderApp::default();
        let target_id = WidgetId::new(2);
        let btn_id = WidgetId::new(1);

        let btn = crate::widget::Widget {
            id: btn_id,
            kind: WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "ToggleBtn".into(),
                actions: vec![WidgetAction {
                    trigger: ActionTrigger::OnClick,
                    effect: ActionEffect::ToggleWidget(target_id),
                }],
                ..Default::default()
            },
            parent: None,
        };
        let target = crate::widget::Widget {
            id: target_id,
            kind: WidgetKind::Label,
            pos: egui::pos2(10.0, 50.0),
            size: egui::vec2(80.0, 24.0),
            z: 2,
            area: DockArea::Center,
            props: WidgetProps {
                text: "Target".into(),
                initially_visible: false,
                ..Default::default()
            },
            parent: None,
        };
        app.project.widgets.push(btn);
        app.project.widgets.push(target);

        let code = app.generate_single_file();
        assert!(code.contains("resp.clicked()"), "should emit resp.clicked() check");
        assert!(
            code.contains(&format!("state.show_{} = !state.show_{}", target_id, target_id)),
            "should emit toggle expression"
        );
        assert!(
            code.contains(&format!("show_{}: bool", target_id)),
            "GeneratedState should have show_ field"
        );
    }

    #[test]
    fn test_codegen_button_click_switch_tab() {
        use crate::widget::{ActionEffect, ActionTrigger, WidgetAction, WidgetId, WidgetKind, WidgetProps, DockArea};

        let mut app = super::RadBuilderApp::default();
        let tab_id = WidgetId::new(5);

        let btn = crate::widget::Widget {
            id: WidgetId::new(1),
            kind: WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "GoTab2".into(),
                actions: vec![WidgetAction {
                    trigger: ActionTrigger::OnClick,
                    effect: ActionEffect::SwitchTab { target: tab_id, tab_index: 1 },
                }],
                ..Default::default()
            },
            parent: None,
        };
        app.project.widgets.push(btn);
        let code = app.generate_single_file();
        assert!(code.contains("resp.clicked()"), "should emit resp.clicked()");
        assert!(
            code.contains(&format!("state.tab_{} = 1", tab_id)),
            "should emit tab switch"
        );
    }

    #[test]
    fn test_codegen_button_click_set_text() {
        use crate::widget::{ActionEffect, ActionTrigger, WidgetAction, WidgetId, WidgetKind, WidgetProps, DockArea};

        let mut app = super::RadBuilderApp::default();
        let label_id = WidgetId::new(3);

        let btn = crate::widget::Widget {
            id: WidgetId::new(1),
            kind: WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "SetBtn".into(),
                actions: vec![WidgetAction {
                    trigger: ActionTrigger::OnClick,
                    effect: ActionEffect::SetText { target: label_id, text: "Updated!".into() },
                }],
                ..Default::default()
            },
            parent: None,
        };
        let lbl = crate::widget::Widget {
            id: label_id,
            kind: WidgetKind::Label,
            pos: egui::pos2(10.0, 50.0),
            size: egui::vec2(100.0, 24.0),
            z: 2,
            area: DockArea::Center,
            props: WidgetProps { text: "Original".into(), ..Default::default() },
            parent: None,
        };
        app.project.widgets.push(btn);
        app.project.widgets.push(lbl);

        let code = app.generate_single_file();
        assert!(code.contains("resp.clicked()"), "should emit resp.clicked()");
        assert!(
            code.contains(&format!("state.text_{} = \"Updated!\".to_owned()", label_id)),
            "should emit SetText assignment"
        );
        // Label should use dynamic text expression
        assert!(
            code.contains(&format!("state.text_{}", label_id)),
            "label should reference state.text_"
        );
    }

    #[test]
    fn test_codegen_modal_open_close() {
        use crate::widget::{ActionEffect, ActionTrigger, WidgetAction, WidgetId, WidgetKind, WidgetProps, DockArea};

        let mut app = super::RadBuilderApp::default();
        let win_id = WidgetId::new(7);

        let open_btn = crate::widget::Widget {
            id: WidgetId::new(1),
            kind: WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "OpenWin".into(),
                actions: vec![WidgetAction {
                    trigger: ActionTrigger::OnClick,
                    effect: ActionEffect::OpenModal(win_id),
                }],
                ..Default::default()
            },
            parent: None,
        };
        app.project.widgets.push(open_btn);

        let code = app.generate_single_file();
        assert!(code.contains("resp.clicked()"), "should emit resp.clicked()");
        assert!(
            code.contains(&format!("state.window_{}_open = true", win_id)),
            "should emit window open"
        );

        // Now test CloseModal
        let mut app2 = super::RadBuilderApp::default();
        let close_btn = crate::widget::Widget {
            id: WidgetId::new(2),
            kind: WidgetKind::Button,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "CloseWin".into(),
                actions: vec![WidgetAction {
                    trigger: ActionTrigger::OnClick,
                    effect: ActionEffect::CloseModal(win_id),
                }],
                ..Default::default()
            },
            parent: None,
        };
        app2.project.widgets.push(close_btn);
        let code2 = app2.generate_single_file();
        assert!(
            code2.contains(&format!("state.window_{}_open = false", win_id)),
            "should emit window close"
        );
    }

    // ─── Z-order / context menu helpers ────────────────────────────────────

    fn make_widget(id: u64, z: i32, parent: Option<u64>) -> crate::widget::Widget {
        crate::widget::Widget {
            id: crate::widget::WidgetId::new(id),
            kind: crate::widget::WidgetKind::Button,
            pos: egui::pos2(0.0, 0.0),
            size: egui::vec2(80.0, 24.0),
            z,
            area: crate::widget::DockArea::Center,
            props: crate::widget::WidgetProps::default(),
            parent: parent.map(crate::widget::WidgetId::new),
        }
    }

    #[test]
    fn test_move_up_down_siblings() {
        let mut app = super::RadBuilderApp::default();
        // Three siblings: z=1, z=2, z=3
        app.project.widgets.push(make_widget(1, 1, None));
        app.project.widgets.push(make_widget(2, 2, None));
        app.project.widgets.push(make_widget(3, 3, None));

        let id1 = crate::widget::WidgetId::new(1);
        let id2 = crate::widget::WidgetId::new(2);

        // move_widget_up(id1): should swap z with id2 (next above)
        app.move_widget_up(id1);
        let z1 = app.project.widgets.iter().find(|w| w.id == id1).unwrap().z;
        let z2 = app.project.widgets.iter().find(|w| w.id == id2).unwrap().z;
        assert_eq!(z1, 2, "id1 should have z=2 after moving up");
        assert_eq!(z2, 1, "id2 should have z=1 after id1 moves up");

        // move_widget_down(id1): should swap back with id2
        app.move_widget_down(id1);
        let z1 = app.project.widgets.iter().find(|w| w.id == id1).unwrap().z;
        let z2 = app.project.widgets.iter().find(|w| w.id == id2).unwrap().z;
        assert_eq!(z1, 1, "id1 should be back to z=1");
        assert_eq!(z2, 2, "id2 should be back to z=2");
    }

    #[test]
    fn test_bring_to_front_send_to_back() {
        let mut app = super::RadBuilderApp::default();
        app.project.widgets.push(make_widget(1, 10, None));
        app.project.widgets.push(make_widget(2, 20, None));
        app.project.widgets.push(make_widget(3, 30, None));

        let id1 = crate::widget::WidgetId::new(1);
        let id3 = crate::widget::WidgetId::new(3);

        app.bring_to_front(id1);
        let z1 = app.project.widgets.iter().find(|w| w.id == id1).unwrap().z;
        assert_eq!(z1, 31, "bring_to_front should set z = max_sibling + 1");

        app.send_to_back(id3);
        let z3 = app.project.widgets.iter().find(|w| w.id == id3).unwrap().z;
        // Siblings are id1=31, id2=20; min is 20, so z3 should be 19
        assert_eq!(z3, 19, "send_to_back should set z = min_sibling - 1");
    }

    #[test]
    fn test_context_action_toggle_visible() {
        let mut app = super::RadBuilderApp::default();
        app.project.widgets.push(make_widget(1, 1, None));
        let id = crate::widget::WidgetId::new(1);
        assert!(app.project.widgets[0].props.initially_visible);

        app.apply_context_menu_action(super::ContextMenuAction::ToggleVisible(id));
        assert!(!app.project.widgets[0].props.initially_visible, "should be hidden");

        app.apply_context_menu_action(super::ContextMenuAction::ToggleVisible(id));
        assert!(app.project.widgets[0].props.initially_visible, "should be visible again");
    }

    #[test]
    fn test_context_action_copy_paste() {
        let mut app = super::RadBuilderApp::default();
        app.project.widgets.push(make_widget(1, 1, None));
        let id = crate::widget::WidgetId::new(1);

        assert!(app.clipboard.is_none());
        app.apply_context_menu_action(super::ContextMenuAction::Copy(id));
        assert!(app.clipboard.is_some(), "clipboard should have widget after Copy");

        let count_before = app.project.widgets.len();
        app.apply_context_menu_action(super::ContextMenuAction::Paste);
        assert_eq!(
            app.project.widgets.len(),
            count_before + 1,
            "Paste should add a new widget"
        );
    }

    // ─── Auto-layout tests ────────────────────────────────────────────────────

    #[test]
    fn test_compute_auto_layout_row_fixed_and_percent_and_fill() {
        use crate::widget::{Align, Justify, LayoutMode, SizePolicy, WidgetId};

        let container_size = egui::vec2(300.0, 100.0);
        let gap = 10.0;
        let padding = [0.0; 4];

        let children = vec![
            // Child 1: Fixed 50px
            (WidgetId::new(1), egui::vec2(50.0, 30.0), SizePolicy::Fixed, SizePolicy::Fixed, None),
            // Child 2: 50% of available (300px * 0.5 = 150px)
            (WidgetId::new(2), egui::vec2(10.0, 40.0), SizePolicy::Percent(50.0), SizePolicy::Fixed, None),
            // Child 3: Fill remainder (300 - 50 - 150 - 20gap = 80px)
            (WidgetId::new(3), egui::vec2(10.0, 50.0), SizePolicy::Fill, SizePolicy::Fixed, None),
        ];

        let rects = super::RadBuilderApp::compute_auto_layout(
            container_size,
            LayoutMode::Row,
            gap,
            2,
            Align::Start,
            Justify::Start,
            padding,
            &children,
            false,
        );

        assert_eq!(rects.len(), 3);
        // Child 1
        assert_eq!(rects[0].0, WidgetId::new(1));
        assert_eq!(rects[0].1.min.x, 0.0);
        assert_eq!(rects[0].1.width(), 50.0);

        // Child 2
        assert_eq!(rects[1].0, WidgetId::new(2));
        assert_eq!(rects[1].1.min.x, 60.0); // 50 + 10gap
        assert_eq!(rects[1].1.width(), 150.0);

        // Child 3
        assert_eq!(rects[2].0, WidgetId::new(3));
        assert_eq!(rects[2].1.min.x, 220.0); // 60 + 150 + 10gap
        assert_eq!(rects[2].1.width(), 80.0);
    }

    #[test]
    fn test_compute_auto_layout_column_justify_center() {
        use crate::widget::{Align, Justify, LayoutMode, SizePolicy, WidgetId};

        let container_size = egui::vec2(100.0, 200.0);
        let gap = 10.0;
        let padding = [0.0; 4];

        let children = vec![
            (WidgetId::new(1), egui::vec2(80.0, 40.0), SizePolicy::Fixed, SizePolicy::Fixed, None),
            (WidgetId::new(2), egui::vec2(80.0, 50.0), SizePolicy::Fixed, SizePolicy::Fixed, None),
        ];
        // Total height = 40 + 50 + 10 = 100. Spare = 100.
        // Justify::Center offset = 50.

        let rects = super::RadBuilderApp::compute_auto_layout(
            container_size,
            LayoutMode::Column,
            gap,
            1,
            Align::Center,
            Justify::Center,
            padding,
            &children,
            false,
        );

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].1.min.y, 50.0);
        assert_eq!(rects[1].1.min.y, 100.0); // 50 + 40 + 10
        // Cross axis centered: width is 80, container is 100 -> x = (100 - 80) / 2 = 10
        assert_eq!(rects[0].1.min.x, 10.0);
        assert_eq!(rects[1].1.min.x, 10.0);
    }

    #[test]
    fn test_codegen_group_with_row_and_grid_layout() {
        use crate::widget::{Justify, LayoutMode, WidgetKind};

        let mut app = super::RadBuilderApp::default();
        let mut group = make_widget(10, 1, None);
        group.kind = WidgetKind::Group;
        group.props.layout_mode = LayoutMode::Row;
        group.props.layout_gap = 16.0;
        group.props.layout_justify = Justify::Center;

        let mut btn1 = make_widget(11, 1, Some(10));
        btn1.props.text = "Btn1".into();

        let mut btn2 = make_widget(12, 2, Some(10));
        btn2.props.text = "Btn2".into();

        app.project.widgets.push(group);
        app.project.widgets.push(btn1);
        app.project.widgets.push(btn2);

        let code = app.generate_single_file();
        assert!(code.contains("ui.with_layout(egui::Layout::left_to_right(egui::Align::Center)"), "should emit row with justify");
        assert!(code.contains("ui.add_space(16.0);"), "should emit gap space");

        // Now test Grid
        let mut app_grid = super::RadBuilderApp::default();
        let mut grid_group = make_widget(20, 1, None);
        grid_group.kind = WidgetKind::Group;
        grid_group.props.layout_mode = LayoutMode::Grid;
        grid_group.props.layout_cols = 2;
        grid_group.props.layout_gap = 8.0;

        let child1 = make_widget(21, 1, Some(20));
        let child2 = make_widget(22, 2, Some(20));

        app_grid.project.widgets.push(grid_group);
        app_grid.project.widgets.push(child1);
        app_grid.project.widgets.push(child2);

        let grid_code = app_grid.generate_single_file();
        assert!(grid_code.contains("egui::Grid::new(\"grid_20\").num_columns(2)"), "should emit grid with 2 columns");
        assert!(grid_code.contains("ui.end_row();"), "should emit end_row");
    }

    #[test]
    fn test_codegen_tab_bar_conditional_pages() {
        use crate::widget::WidgetKind;
        let mut app = super::RadBuilderApp::default();

        // TabBar with two tabs
        let mut tabbar = make_widget(30, 1, None);
        tabbar.kind = WidgetKind::TabBar;
        tabbar.props.items = vec!["Tab A".into(), "Tab B".into()];
        tabbar.props.selected = 0;

        // Child assigned to Tab 0
        let mut child_a = make_widget(31, 1, Some(30));
        child_a.kind = WidgetKind::Label;
        child_a.props.text = "LabelForTabA".into();
        child_a.props.tab_page = Some(0);

        // Child assigned to Tab 1
        let mut child_b = make_widget(32, 2, Some(30));
        child_b.kind = WidgetKind::Label;
        child_b.props.text = "LabelForTabB".into();
        child_b.props.tab_page = Some(1);

        // Unassigned child (should appear unconditionally)
        let mut child_free = make_widget(33, 3, Some(30));
        child_free.kind = WidgetKind::Label;
        child_free.props.text = "LabelAlways".into();
        child_free.props.tab_page = None;

        app.project.widgets.push(tabbar);
        app.project.widgets.push(child_a);
        app.project.widgets.push(child_b);
        app.project.widgets.push(child_free);

        let code = app.generate_single_file();
        // Tab bar header
        assert!(code.contains("ui.selectable_value"), "should emit selectable_value for tabs");
        // Tab 0 conditional
        assert!(code.contains("if state.tab_30 == 0"), "should emit tab page 0 conditional");
        assert!(code.contains("LabelForTabA"), "should emit tab A child");
        // Tab 1 conditional
        assert!(code.contains("if state.tab_30 == 1"), "should emit tab page 1 conditional");
        assert!(code.contains("LabelForTabB"), "should emit tab B child");
        // Unconditional child appears without tab guard
        assert!(code.contains("LabelAlways"), "should emit unconditional child");
    }

    #[test]
    fn test_codegen_collapsing_header_emits_children_inside() {
        use crate::widget::WidgetKind;
        let mut app = super::RadBuilderApp::default();

        let mut header = make_widget(40, 1, None);
        header.kind = WidgetKind::CollapsingHeader;
        header.props.text = "MySection".into();
        header.props.checked = true; // default open

        let mut child = make_widget(41, 1, Some(40));
        child.kind = WidgetKind::Label;
        child.props.text = "SectionContent".into();

        app.project.widgets.push(header);
        app.project.widgets.push(child);

        let code = app.generate_single_file();
        assert!(code.contains("egui::CollapsingHeader::new(\"MySection\")"), "should emit CollapsingHeader");
        assert!(code.contains("SectionContent"), "child should be emitted");
        // Children must appear inside the header's .show closure, not after });
        let header_pos = code.find("egui::CollapsingHeader::new").unwrap_or(0);
        let child_pos = code.find("SectionContent").unwrap_or(0);
        assert!(child_pos > header_pos, "child should appear after CollapsingHeader::new in code");
    }

    #[test]
    fn test_codegen_responsive_portrait_only() {
        use crate::widget::{WidgetKind, ResponsiveVisibility};
        let mut app = super::RadBuilderApp::default();

        let mut btn = make_widget(50, 1, None);
        btn.kind = WidgetKind::Button;
        btn.props.text = "PortraitBtn".into();
        btn.props.responsive_vis = ResponsiveVisibility::PortraitOnly;
        app.project.widgets.push(btn);

        let code = app.generate_single_file();
        assert!(
            code.contains("ui.available_height() > ui.available_width()"),
            "should emit portrait condition"
        );
        assert!(code.contains("PortraitBtn"), "should still emit button text");
    }

    #[test]
    fn test_codegen_responsive_landscape_mobile_desktop() {
        use crate::widget::{WidgetKind, ResponsiveVisibility};

        for (vis, expected_cond, label) in [
            (ResponsiveVisibility::LandscapeOnly, "ui.available_width() >= ui.available_height()", "LandscapeBtn"),
            (ResponsiveVisibility::MobileOnly,    "ui.available_width() <= 600.0",                 "MobileBtn"),
            (ResponsiveVisibility::DesktopOnly,   "ui.available_width() > 600.0",                  "DesktopBtn"),
        ] {
            let mut app = super::RadBuilderApp::default();
            let mut btn = make_widget(60, 1, None);
            btn.kind = WidgetKind::Button;
            btn.props.text = label.into();
            btn.props.responsive_vis = vis;
            app.project.widgets.push(btn);
            let code = app.generate_single_file();
            assert!(code.contains(expected_cond), "should emit condition for {:?}", label);
            assert!(code.contains(label), "should emit widget text for {:?}", label);
        }
    }

    #[test]
    fn test_tiny_group_and_window_do_not_panic() {
        // Regression test for: "Negative width makes no sense" panic when auto-layout
        // assigns a size smaller than the widget border offsets (12px Group, 16px Window).
        // This can happen when a Fill-policy Group is inside a WrapRow container that
        // becomes very narrow, causing cw.size to be overwritten with a tiny value.
        use crate::widget::{DockArea, WidgetId, WidgetKind, WidgetProps};

        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::Vec2::new(30.0, 30.0);

        // Group with 8px size: 8 - 12 = -4 would have panicked before the fix
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(9001),
            kind: WidgetKind::Group,
            pos: egui::pos2(0.0, 0.0),
            size: egui::vec2(8.0, 8.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps { text: String::new(), active: true, ..WidgetProps::default() },
            parent: None,
        });

        // Window with 10px size: 10 - 16 = -6 would have panicked before the fix
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(9002),
            kind: WidgetKind::Window,
            pos: egui::pos2(0.0, 0.0),
            size: egui::vec2(10.0, 10.0),
            z: 2,
            area: DockArea::Center,
            props: WidgetProps { text: "T".to_string(), active: true, ..WidgetProps::default() },
            parent: None,
        });

        let ctx = egui::Context::default();
        // Must not panic in any of 10 simulated frames
        for _ in 0..10 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                app.preview_panels_ui(ctx);
            });
        }
        let code = app.generate_single_file();
        assert!(!code.is_empty());
    }

    #[test]
    fn test_compute_auto_layout_column_hug_height() {
        use crate::widget::{Align, Justify, LayoutMode, SizePolicy, WidgetId};

        let container_size = egui::vec2(100.0, 500.0);
        let gap = 10.0;
        let padding = [0.0; 4];

        // Two children with Fill height policy in a 500px container
        let children = vec![
            (WidgetId::new(1), egui::vec2(80.0, 40.0), SizePolicy::Fixed, SizePolicy::Fill, None),
            (WidgetId::new(2), egui::vec2(80.0, 60.0), SizePolicy::Fixed, SizePolicy::Fill, None),
        ];

        // When hug_height is true, children must NOT expand to fill 500px; they must use natural sizes (40 and 60)
        let rects = super::RadBuilderApp::compute_auto_layout(
            container_size,
            LayoutMode::Column,
            gap,
            1,
            Align::Start,
            Justify::Start,
            padding,
            &children,
            true,
        );

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].1.height(), 40.0);
        assert_eq!(rects[1].1.height(), 60.0);
        assert_eq!(rects[1].1.min.y, 50.0); // 40 + 10 gap
    }

    #[test]
    fn test_collapsing_header_auto_size_y_stable() {
        use crate::widget::{DockArea, LayoutMode, SizePolicy, WidgetId, WidgetKind, WidgetProps};

        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::vec2(600.0, 800.0);

        // Collapsing header with Column layout and auto_size_y = true
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(100),
            kind: WidgetKind::CollapsingHeader,
            pos: egui::pos2(10.0, 10.0),
            size: egui::vec2(300.0, 100.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "Section".to_string(),
                checked: true,
                active: true,
                auto_size_y: true,
                layout_mode: LayoutMode::Column,
                layout_gap: 10.0,
                layout_padding: [8.0, 8.0, 8.0, 8.0],
                ..WidgetProps::default()
            },
            parent: None,
        });

        // Two children with Fill height policy (identical to the user's project setup)
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(101),
            kind: WidgetKind::Group,
            pos: egui::pos2(0.0, 0.0),
            size: egui::vec2(280.0, 50.0),
            z: 2,
            area: DockArea::Center,
            props: WidgetProps {
                text: "Child 1".to_string(),
                active: true,
                height_policy: SizePolicy::Fill,
                ..WidgetProps::default()
            },
            parent: Some(WidgetId::new(100)),
        });

        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(102),
            kind: WidgetKind::Group,
            pos: egui::pos2(0.0, 0.0),
            size: egui::vec2(280.0, 60.0),
            z: 3,
            area: DockArea::Center,
            props: WidgetProps {
                text: "Child 2".to_string(),
                active: true,
                height_policy: SizePolicy::Fill,
                ..WidgetProps::default()
            },
            parent: Some(WidgetId::new(100)),
        });

        let ctx = egui::Context::default();
        let mut heights = Vec::new();
        // Run 30 consecutive simulated frames
        for _ in 0..30 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                app.preview_panels_ui(ctx);
            });
            if let Some(h) = app.project.widgets.iter().find(|w| w.id == WidgetId::new(100)).map(|w| w.size.y) {
                heights.push(h);
            }
        }

        // Height must be stable across frames, NOT growing to infinity
        let first = heights[2];
        let last = *heights.last().unwrap();
        assert!(
            (first - last).abs() <= 1.0,
            "auto_size_y must not grow to infinity! Frame 2: {}, Frame 29: {}",
            first,
            last
        );
    }

    #[test]
    fn test_collapsing_header_collapse_and_expand_height() {
        use crate::widget::{DockArea, WidgetId, WidgetKind, WidgetProps};

        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::vec2(400.0, 400.0);

        // Header initially open with height 200px
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(200),
            kind: WidgetKind::CollapsingHeader,
            pos: egui::pos2(20.0, 20.0),
            size: egui::vec2(250.0, 200.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "My Section".to_string(),
                checked: true,
                active: true,
                auto_size_y: false,
                ..WidgetProps::default()
            },
            parent: None,
        });

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.preview_panels_ui(ctx);
        });

        // Close header
        if let Some(w) = app.project.widgets.iter_mut().find(|w| w.id == WidgetId::new(200)) {
            w.props.checked = false;
        }

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.preview_panels_ui(ctx);
        });

        let w = app.project.widgets.iter().find(|w| w.id == WidgetId::new(200)).unwrap();
        assert_eq!(w.size.y, 26.0, "collapsed header must shrink to 26px");
        assert_eq!(w.props.expanded_height, Some(200.0), "expanded height must be preserved");

        // Re-open header
        if let Some(w) = app.project.widgets.iter_mut().find(|w| w.id == WidgetId::new(200)) {
            w.props.checked = true;
        }

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.preview_panels_ui(ctx);
        });

        let w = app.project.widgets.iter().find(|w| w.id == WidgetId::new(200)).unwrap();
        assert_eq!(w.size.y, 200.0, "re-opened header must restore its previous expanded height");
    }

    #[test]
    fn test_collapsing_header_shifts_sibling_widgets_below() {
        use crate::widget::{DockArea, WidgetId, WidgetKind, WidgetProps};

        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::vec2(400.0, 600.0);

        // Header at y = 50.0 with height 100.0 (bottom is at 150.0)
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(300),
            kind: WidgetKind::CollapsingHeader,
            pos: egui::pos2(20.0, 50.0),
            size: egui::vec2(250.0, 100.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps {
                text: "Header".to_string(),
                checked: true,
                active: true,
                auto_size_y: false,
                ..WidgetProps::default()
            },
            parent: None,
        });

        // Sibling widget at y = 160.0 (10px below the header)
        app.project.widgets.push(crate::widget::Widget {
            id: WidgetId::new(301),
            kind: WidgetKind::Button,
            pos: egui::pos2(20.0, 160.0),
            size: egui::vec2(100.0, 30.0),
            z: 2,
            area: DockArea::Center,
            props: WidgetProps {
                text: "Below".to_string(),
                active: true,
                ..WidgetProps::default()
            },
            parent: None,
        });

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.preview_panels_ui(ctx);
        });

        // Collapse header: height changes from 100.0 to 26.0 (delta = -74.0)
        if let Some(w) = app.project.widgets.iter_mut().find(|w| w.id == WidgetId::new(300)) {
            w.props.checked = false;
        }

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.preview_panels_ui(ctx);
        });

        let btn = app.project.widgets.iter().find(|w| w.id == WidgetId::new(301)).unwrap();
        // Button should have shifted up by 74px: 160.0 - 74.0 = 86.0
        assert_eq!(btn.pos.y, 86.0, "widget below collapsed header must shift up by delta_h");

        // Re-expand header: height changes from 26.0 back to 100.0 (delta = +74.0)
        if let Some(w) = app.project.widgets.iter_mut().find(|w| w.id == WidgetId::new(300)) {
            w.props.checked = true;
        }

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.preview_panels_ui(ctx);
        });

        let btn = app.project.widgets.iter().find(|w| w.id == WidgetId::new(301)).unwrap();
        assert_eq!(btn.pos.y, 160.0, "widget below re-expanded header must shift back down to original position");
    }

    #[test]
    fn test_platform_info() {
        let info = super::RadBuilderApp::platform_info();
        assert!(!info.is_empty(), "platform info must not be empty");
        assert!(info.contains(std::env::consts::ARCH), "should contain current architecture");
        assert!(
            info.chars().all(|c| c.is_ascii() || c.is_alphanumeric() || c.is_whitespace() || c == '(' || c == ')' || c == '-'),
            "platform info must be clean text without emojis"
        );
    }

    #[test]
    fn test_status_bar_messages() {
        let mut app = super::RadBuilderApp::default();
        // Initial state is "Ready"
        assert_eq!(app.status_message, "Ready");

        // Custom set_status
        app.set_status("Project loaded".into());
        assert_eq!(app.status_message, "Project loaded");

        // Add widget
        let w = make_widget(10, 1, None);
        app.project.widgets.push(w);
        app.selected = vec![crate::widget::WidgetId::new(10)];

        // Duplicate
        app.duplicate_selected();
        assert_eq!(app.status_message, "Duplicated 1 widget(s)");

        // Undo
        app.undo();
        assert_eq!(app.status_message, "Undo");

        // Redo
        app.redo();
        assert_eq!(app.status_message, "Redo");

        // Delete
        app.delete_selected();
        assert_eq!(app.status_message, "Deleted 1 widget(s)");

        // Paste
        app.clipboard = Some(make_widget(99, 1, None));
        app.paste();
        assert!(app.status_message.starts_with("Pasted widget #"));
    }

    #[test]
    fn test_root_auto_layout_column_fills_screen_width() {
        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::vec2(390.0, 844.0);
        app.project.root_layout_mode = crate::widget::LayoutMode::Column;
        app.project.root_layout_padding = [10.0, 10.0, 10.0, 10.0];
        app.project.root_layout_gap = 8.0;

        let mut w1 = make_widget(1, 1, None);
        w1.size = egui::vec2(100.0, 50.0);
        w1.props.width_policy = crate::widget::SizePolicy::Fill;

        let mut w2 = make_widget(2, 2, None);
        w2.size = egui::vec2(100.0, 30.0);
        w2.props.width_policy = crate::widget::SizePolicy::Percent(50.0);

        let children_info = vec![
            (w1.id, w1.size, w1.props.width_policy, w1.props.height_policy, w1.props.align_self),
            (w2.id, w2.size, w2.props.width_policy, w2.props.height_policy, w2.props.align_self),
        ];

        let rects = super::RadBuilderApp::compute_auto_layout(
            app.project.canvas_size,
            app.project.root_layout_mode,
            app.project.root_layout_gap,
            app.project.root_layout_cols,
            app.project.root_layout_align,
            app.project.root_layout_justify,
            app.project.root_layout_padding,
            &children_info,
            true,
        );

        assert_eq!(rects.len(), 2);
        // Available width = 390 - 10 - 10 = 370
        let r1 = rects[0].1;
        assert!((r1.width() - 370.0).abs() < 0.1, "Widget 1 should fill full width minus padding");
        assert_eq!(r1.min.x, 10.0);
        assert_eq!(r1.min.y, 10.0);

        let r2 = rects[1].1;
        assert!((r2.width() - 185.0).abs() < 0.1, "Widget 2 should take 50% of available width");
        assert_eq!(r2.min.x, 10.0);
        // r2.min.y should be 10 (pad_top) + 50 (w1 height) + 8 (gap) = 68.0
        assert_eq!(r2.min.y, 68.0);
    }

    #[test]
    fn test_codegen_root_layout_and_scroll_area() {
        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::vec2(400.0, 700.0);
        app.project.root_layout_mode = crate::widget::LayoutMode::Column;
        app.project.root_layout_gap = 12.0;

        let w = make_widget(10, 1, None);
        app.project.widgets.push(w);

        let code = app.generate_single_file();
        assert!(
            code.contains("egui::ScrollArea::vertical().auto_shrink([false, false])"),
            "Generated CentralPanel must include vertical ScrollArea"
        );
        assert!(
            code.contains("ui.vertical(|ui| {"),
            "Column root layout must emit ui.vertical"
        );
    }

    #[test]
    fn test_canvas_zoom_spawn_coordinate_scaling() {
        let mut app = super::RadBuilderApp::default();
        app.grid_size = 1.0; // 1px snap for exact math

        // Test at zoom = 0.5 (scaled down by half)
        app.canvas_zoom = 0.5;
        let origin = egui::pos2(100.0, 100.0);
        let mouse_pos = egui::pos2(200.0, 300.0);
        // (mouse - origin) = (100, 200). Divided by 0.5 zoom => logical (200, 400).
        // default_size for Label is vec2(140, 24). Half size is vec2(70, 12).
        // expected pos = (200 - 70, 400 - 12) = (130, 388).
        app.spawn_widget(
            crate::widget::WidgetKind::Label,
            mouse_pos,
            crate::widget::DockArea::Center,
            origin,
        );
        let w = app.project.widgets.last().unwrap();
        assert_eq!(w.pos.x, 130.0);
        assert_eq!(w.pos.y, 388.0);

        // Test at zoom = 2.0 (scaled up 2x)
        app.canvas_zoom = 2.0;
        let mouse_pos2 = egui::pos2(300.0, 500.0);
        // (mouse - origin) = (200, 400). Divided by 2.0 zoom => logical (100, 200).
        // expected pos = (100 - 70, 200 - 12) = (30, 188).
        app.spawn_widget(
            crate::widget::WidgetKind::Label,
            mouse_pos2,
            crate::widget::DockArea::Center,
            origin,
        );
        let w2 = app.project.widgets.last().unwrap();
        assert_eq!(w2.pos.x, 30.0);
        assert_eq!(w2.pos.y, 188.0);
    }

    #[test]
    fn test_canvas_auto_fit_calculation() {
        let mut app = super::RadBuilderApp::default();
        app.project.canvas_size = egui::vec2(1920.0, 1080.0);
        app.canvas_auto_fit = true;

        // In a laptop screen with 960 width available and 540 height available:
        let avail_w = 984.0 - 24.0; // 960.0
        let avail_h = 578.0 - 38.0; // 540.0
        let fit_w = avail_w / app.project.canvas_size.x; // 960 / 1920 = 0.5
        let fit_h = avail_h / app.project.canvas_size.y; // 540 / 1080 = 0.5
        let calculated_zoom = fit_w.min(fit_h).clamp(0.1, 3.0);

        assert!((calculated_zoom - 0.5).abs() < 0.001);
    }
}


/// Draft state for the inspector's "+ Add Action" form.
#[derive(Clone, Debug)]
pub(crate) struct ActionDraft {
    pub(crate) trigger: ActionTrigger,
    pub(crate) effect_kind: usize,
    pub(crate) target: Option<WidgetId>,
    pub(crate) text: String,
    pub(crate) tab: usize,
    pub(crate) code: String,
}

impl Default for ActionDraft {
    fn default() -> Self {
        Self {
            trigger: ActionTrigger::OnClick,
            effect_kind: 0,
            target: None,
            text: String::new(),
            tab: 0,
            code: String::new(),
        }
    }
}

/// Action selected from the Layers panel right-click context menu.
/// Deferred to avoid borrow conflicts during tree rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ContextMenuAction {
    Duplicate(WidgetId),
    Copy(WidgetId),
    Paste,
    Delete(WidgetId),
    ToggleVisible(WidgetId),
    ToggleActive(WidgetId),
    MoveUp(WidgetId),
    MoveDown(WidgetId),
    BringToFront(WidgetId),
    SendToBack(WidgetId),
    Reparent(WidgetId, Option<WidgetId>),
}

/// Zone within a Layers row where a dragged widget will be dropped.
#[derive(Clone, Copy, PartialEq, Debug)]
enum DragZone {
    /// Insert before the target (above) — same parent, higher z.
    Before,
    /// Nest inside the target (only for containers).
    Inside,
    /// Insert after the target (below) — same parent, lower z.
    After,
}

/// Interactive `eframe` application that owns the builder workspace.
///
/// The app stores the active project, selection state, preview settings, and
/// generated-code options. Construct it with [`Default`] when launching the
/// desktop builder.
pub struct RadBuilderApp {
    palette_open: bool,
    project: Project,
    /// Currently selected widgets (supports multi-select)
    selected: Vec<WidgetId>,
    next_id: u64,
    // Drag state for spawning from palette
    spawning: Option<WidgetKind>,
    // Cached generated code
    generated: String,
    // Settings
    grid_size: f32,
    show_grid: bool,
    live_top: Option<Rect>,
    live_bottom: Option<Rect>,
    live_left: Option<Rect>,
    live_right: Option<Rect>,
    live_center: Option<Rect>,
    live_center_origin: Option<Pos2>,
    /// Canvas viewport zoom factor (1.0 = 100%, 0.5 = 50%, etc.)
    canvas_zoom: f32,
    /// Automatically scale zoom to fit available CentralPanel space
    canvas_auto_fit: bool,
    // Clipboard for copy/paste
    clipboard: Option<Widget>,
    /// Current project file path (for Save)
    current_file: Option<PathBuf>,
    /// Error/status message to display in the bottom status bar
    status_message: String,
    /// Drag selection box (start position when dragging to select)
    #[allow(dead_code)]
    drag_select_start: Option<Pos2>,
    /// Syntax highlighter for code preview
    highlighter: Highlighter,
    /// Whether to show syntax highlighting (can be toggled for performance)
    syntax_highlighting: bool,
    /// Auto-generate code on widget changes
    auto_generate: bool,
    /// Code generation output format
    codegen_format: CodeGenFormat,
    /// Add comments to generated code
    codegen_comments: bool,
    /// Preview mode: interact with widgets without selection handles
    preview_mode: bool,
    /// Active tab in the right panel (0 = Inspector, 1 = Code Output)
    right_panel_tab: usize,
    /// Active tab in the left panel (0 = Palette, 1 = Layers)
    left_panel_tab: usize,
    /// Widget being dragged within the Layers panel for reorder
    layer_drag: Option<WidgetId>,
    /// Drop zone detected during a layer D&D drag
    layer_drag_zone: Option<DragZone>,
    /// Pending context-menu action from Layers panel (deferred to avoid borrow conflicts)
    #[allow(dead_code)]
    context_menu_action: Option<ContextMenuAction>,
    /// Undo and redo history manager
    history: History,
    /// Draft state for configuring actions in the inspector
    action_draft: ActionDraft,
}

impl Default for RadBuilderApp {
    fn default() -> Self {
        Self {
            palette_open: true,
            project: Project::default(),
            selected: Vec::new(),
            next_id: 1,
            spawning: None,
            generated: String::new(),
            grid_size: 1.0,
            show_grid: false,
            live_top: None,
            live_bottom: None,
            live_left: None,
            live_right: None,
            live_center: None,
            live_center_origin: None,
            canvas_zoom: 1.0,
            canvas_auto_fit: false,
            clipboard: None,
            current_file: None,
            status_message: "Ready".to_string(),
            drag_select_start: None,
            highlighter: Highlighter::new(),
            syntax_highlighting: true,
            auto_generate: false,
            codegen_format: CodeGenFormat::default(),
            codegen_comments: true,
            preview_mode: false,
            right_panel_tab: 0,
            left_panel_tab: 0,
            layer_drag: None,
            layer_drag_zone: None,
            context_menu_action: None,
            history: History::default(),
            action_draft: ActionDraft::default(),
        }
    }
}

impl RadBuilderApp {
    fn current_snapshot(&self) -> HistorySnapshot {
        HistorySnapshot::new(self.project.clone(), self.selected.clone(), self.next_id)
    }

    fn apply_snapshot(&mut self, snapshot: HistorySnapshot) {
        self.project = snapshot.project;
        self.selected = snapshot.selected;
        self.next_id = snapshot.next_id;
        self.selected
            .retain(|id| self.project.widgets.iter().any(|w| w.id == *id));
    }

    fn push_undo(&mut self) {
        let snapshot = self.current_snapshot();
        self.history.push(snapshot);
    }

    fn undo(&mut self) {
        let current = self.current_snapshot();
        if let Some(prev) = self.history.undo(current) {
            self.apply_snapshot(prev);
            self.set_status("Undo".into());
        }
    }

    fn redo(&mut self) {
        let current = self.current_snapshot();
        if let Some(next) = self.history.redo(current) {
            self.apply_snapshot(next);
            self.set_status("Redo".into());
        }
    }

    fn delete_selected(&mut self) {
        if !self.selected.is_empty() {
            self.push_undo();
            let mut to_delete = Vec::new();
            for &sel_id in &self.selected {
                to_delete.push(sel_id);
                to_delete.extend(self.project.get_descendants(sel_id));
            }
            let count = to_delete.len();
            self.project.widgets.retain(|w| !to_delete.contains(&w.id));
            self.selected.clear();
            self.set_status(format!("Deleted {} widget(s)", count));
        }
    }

    // ── Z-order / Layer helpers ──────────────────────────────────────────────

    /// Sorted (ascending z) list of siblings of `id` (same parent, excluding self).
    fn siblings_of(&self, id: WidgetId) -> Vec<(WidgetId, i32)> {
        let parent = self
            .project
            .widgets
            .iter()
            .find(|w| w.id == id)
            .and_then(|w| w.parent);
        let mut sibs: Vec<(WidgetId, i32)> = self
            .project
            .widgets
            .iter()
            .filter(|w| w.parent == parent && w.id != id)
            .map(|w| (w.id, w.z))
            .collect();
        sibs.sort_by_key(|&(_, z)| z);
        sibs
    }

    /// Swap z with the next sibling above (higher z = higher in Layers list).
    fn move_widget_up(&mut self, id: WidgetId) {
        let my_z = match self.project.widgets.iter().find(|w| w.id == id) {
            Some(w) => w.z,
            None => return,
        };
        let sibs = self.siblings_of(id);
        if let Some(&(sib_id, sib_z)) = sibs.iter().find(|&&(_, z)| z > my_z) {
            self.push_undo();
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == id) {
                w.z = sib_z;
            }
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == sib_id) {
                w.z = my_z;
            }
        }
    }

    /// Swap z with the next sibling below (lower z = lower in Layers list).
    fn move_widget_down(&mut self, id: WidgetId) {
        let my_z = match self.project.widgets.iter().find(|w| w.id == id) {
            Some(w) => w.z,
            None => return,
        };
        let sibs = self.siblings_of(id);
        if let Some(&(sib_id, sib_z)) = sibs.iter().rev().find(|&&(_, z)| z < my_z) {
            self.push_undo();
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == id) {
                w.z = sib_z;
            }
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == sib_id) {
                w.z = my_z;
            }
        }
    }

    /// Set z above the highest sibling.
    fn bring_to_front(&mut self, id: WidgetId) {
        let sibs = self.siblings_of(id);
        if let Some(&(_, max_z)) = sibs.iter().max_by_key(|&&(_, z)| z) {
            self.push_undo();
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == id) {
                w.z = max_z + 1;
            }
        }
    }

    /// Set z below the lowest sibling.
    fn send_to_back(&mut self, id: WidgetId) {
        let sibs = self.siblings_of(id);
        if let Some(&(_, min_z)) = sibs.iter().min_by_key(|&&(_, z)| z) {
            self.push_undo();
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == id) {
                w.z = min_z - 1;
            }
        }
    }

    /// Apply a deferred context-menu action from the Layers panel.
    fn apply_context_menu_action(&mut self, action: ContextMenuAction) {
        match action {
            ContextMenuAction::Duplicate(id) => {
                self.selected = vec![id];
                self.duplicate_selected();
            }
            ContextMenuAction::Copy(id) => {
                self.clipboard = self.project.widgets.iter().find(|w| w.id == id).cloned();
                self.set_status(format!("Copied widget #{id}"));
            }
            ContextMenuAction::Paste => self.paste(),
            ContextMenuAction::Delete(id) => {
                self.selected = vec![id];
                self.delete_selected();
            }
            ContextMenuAction::ToggleVisible(id) => {
                self.push_undo();
                if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == id) {
                    w.props.initially_visible = !w.props.initially_visible;
                }
            }
            ContextMenuAction::ToggleActive(id) => {
                self.push_undo();
                if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == id) {
                    w.props.active = !w.props.active;
                }
            }
            ContextMenuAction::MoveUp(id)       => self.move_widget_up(id),
            ContextMenuAction::MoveDown(id)     => self.move_widget_down(id),
            ContextMenuAction::BringToFront(id) => self.bring_to_front(id),
            ContextMenuAction::SendToBack(id)   => self.send_to_back(id),
            ContextMenuAction::Reparent(id, p)  => self.reparent_widget(id, p),
        }
    }

    fn duplicate_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let selected_ids: Vec<_> = self.selected.clone();
        let mut new_ids = Vec::new();
        for sel_id in selected_ids {
            if let Some(w) = self
                .project
                .widgets
                .iter()
                .find(|w| w.id == sel_id)
                .cloned()
            {
                let new_id = WidgetId::new(self.next_id);
                self.next_id += 1;
                let mut dup = w;
                dup.id = new_id;
                dup.z = new_id.as_z();
                dup.pos.x += 20.0;
                dup.pos.y += 20.0;
                self.project.widgets.push(dup);
                new_ids.push(new_id);
            }
        }
        let count = new_ids.len();
        self.selected = new_ids;
        self.set_status(format!("Duplicated {} widget(s)", count));
    }

    fn paste(&mut self) {
        if let Some(w) = self.clipboard.clone() {
            self.push_undo();
            let new_id = WidgetId::new(self.next_id);
            self.next_id += 1;
            let mut pasted = w;
            pasted.id = new_id;
            pasted.z = new_id.as_z();
            pasted.pos.x += 20.0;
            pasted.pos.y += 20.0;
            self.project.widgets.push(pasted);
            self.selected = vec![new_id];
            self.set_status(format!("Pasted widget #{}", new_id));
        }
    }

    fn normalize_project_widget_ids(&mut self) {
        let mut seen = HashSet::new();
        let mut next_id = self
            .project
            .widgets
            .iter()
            .map(|w| w.id.as_z() as u64 + 1)
            .max()
            .unwrap_or(1);
        let mut had_duplicates = false;

        for w in &mut self.project.widgets {
            if seen.insert(w.id) {
                continue;
            }

            had_duplicates = true;
            while seen.contains(&WidgetId::new(next_id)) {
                next_id += 1;
            }

            w.id = WidgetId::new(next_id);
            w.z = w.id.as_z();
            seen.insert(w.id);
            next_id += 1;
        }

        self.next_id = next_id;
        self.selected
            .retain(|id| self.project.widgets.iter().any(|w| w.id == *id));
        self.selected.sort();
        self.selected.dedup();

        if had_duplicates {
            self.set_status("Duplicate widget IDs were normalized".into());
        }
    }

    fn area_at(&self, pos: Pos2) -> DockArea {
        if let Some(r) = self.live_top
            && r.contains(pos)
        {
            return DockArea::Top;
        }
        if let Some(r) = self.live_bottom
            && r.contains(pos)
        {
            return DockArea::Bottom;
        }
        if let Some(r) = self.live_left
            && r.contains(pos)
        {
            return DockArea::Left;
        }
        if let Some(r) = self.live_right
            && r.contains(pos)
        {
            return DockArea::Right;
        }
        if let Some(r) = self.live_center
            && r.contains(pos)
        {
            return DockArea::Center;
        }
        DockArea::Free
    }

    fn origin_for_area(&self, area: DockArea) -> Option<Pos2> {
        match area {
            DockArea::Top => self.live_top.map(|r| r.min),
            DockArea::Bottom => self.live_bottom.map(|r| r.min),
            DockArea::Left => self.live_left.map(|r| r.min),
            DockArea::Right => self.live_right.map(|r| r.min),
            DockArea::Center | DockArea::Free => {
                self.live_center_origin.or_else(|| self.live_center.map(|r| r.min))
            }
        }
    }

    fn spawn_widget(
        &mut self,
        kind: WidgetKind,
        at_global: Pos2,
        area: DockArea,
        area_origin: Pos2,
    ) {
        self.push_undo();
        let id = WidgetId::new(self.next_id);
        self.next_id += 1;

        // Use centralized default_size and default_props from WidgetKind
        let size = kind.default_size();
        let props = kind.default_props();

        let zoom = self.canvas_zoom.clamp(0.1, 4.0);
        let vecpos = (at_global - area_origin) / zoom - size * 0.5; // local to area
        let pos = self.snap_pos(pos2(vecpos.x, vecpos.y));
        let w = Widget {
            id,
            kind,
            pos,
            size,
            z: id.as_z(),
            area,
            props,
            parent: None,
        };
        self.project.widgets.push(w);
        self.selected = vec![id];
    }

    /// Returns the inner content offset for a container widget.
    pub(crate) fn container_content_offset(kind: WidgetKind, props_text: &str, show_tabs: bool) -> egui::Vec2 {
        match kind {
            WidgetKind::Group => {
                if !props_text.is_empty() {
                    vec2(8.0, 26.0)
                } else {
                    vec2(8.0, 8.0)
                }
            }
            WidgetKind::Window => vec2(8.0, 28.0),
            WidgetKind::ScrollBox => vec2(6.0, 6.0),
            // When show_tabs=false (ViewContainer), no tab header → 6px padding only
            WidgetKind::TabBar => if show_tabs { vec2(6.0, 28.0) } else { vec2(6.0, 6.0) },
            WidgetKind::CollapsingHeader => vec2(8.0, 24.0),
            WidgetKind::Columns => vec2(6.0, 6.0),
            WidgetKind::Container => vec2(0.0, 0.0),
            _ => vec2(6.0, 6.0),
        }
    }

    /// Computes the absolute canvas position of a widget by traversing up its parent chain.
    pub(crate) fn compute_abs_pos(&self, wid: WidgetId) -> Pos2 {
        let Some(w) = self.project.widgets.iter().find(|x| x.id == wid) else {
            return Pos2::ZERO;
        };
        match w.parent {
            None => w.pos,
            Some(pid) => {
                let parent_pos = self.compute_abs_pos(pid);
                let offset = self
                    .project
                    .widgets
                    .iter()
                    .find(|x| x.id == pid)
                    .map(|pw| Self::container_content_offset(pw.kind, &pw.props.text, pw.props.show_tabs))
                    .unwrap_or(egui::Vec2::ZERO);
                parent_pos + offset + w.pos.to_vec2()
            }
        }
    }

    /// Reparents a widget to a new parent (or makes it root if None),
    /// keeping its visual position on the canvas intact and clamping within new parent bounds.
    pub(crate) fn reparent_widget(&mut self, wid: WidgetId, new_parent: Option<WidgetId>) {
        let current_parent = self
            .project
            .widgets
            .iter()
            .find(|x| x.id == wid)
            .and_then(|x| x.parent);

        if current_parent == new_parent {
            return;
        }

        // Prevent cyclic parenting
        if let Some(pid) = new_parent {
            if pid == wid || self.project.is_descendant_of(pid, wid) {
                return;
            }
        }

        let abs_pos = self.compute_abs_pos(wid);

        let mut new_pos = match new_parent {
            None => abs_pos,
            Some(pid) => {
                let parent_abs = self.compute_abs_pos(pid);
                let offset = self
                    .project
                    .widgets
                    .iter()
                    .find(|x| x.id == pid)
                    .map(|pw| Self::container_content_offset(pw.kind, &pw.props.text, pw.props.show_tabs))
                    .unwrap_or(egui::Vec2::ZERO);
                let content_origin = parent_abs + offset;
                pos2(
                    (abs_pos.x - content_origin.x).max(0.0),
                    (abs_pos.y - content_origin.y).max(0.0),
                )
            }
        };

        // Clamp to parent bounds if nested
        if let Some(pid) = new_parent {
            if let Some(pw) = self.project.widgets.iter().find(|x| x.id == pid) {
                let child_size = self
                    .project
                    .widgets
                    .iter()
                    .find(|x| x.id == wid)
                    .map(|x| x.size)
                    .unwrap_or(vec2(50.0, 30.0));
                let max_x = (pw.size.x - child_size.x).max(0.0);
                let max_y = (pw.size.y - child_size.y).max(0.0);
                new_pos.x = new_pos.x.clamp(0.0, max_x);
                new_pos.y = new_pos.y.clamp(0.0, max_y);
            }
        }

        let snapped_pos = snap_pos_with_grid(new_pos, self.grid_size);

        if let Some(w) = self.project.widgets.iter_mut().find(|x| x.id == wid) {
            w.parent = new_parent;
            w.pos = snapped_pos;
        }
    }

    /// Returns the first selected widget for editing (inspector uses this)
    fn selected_mut(&mut self) -> Option<&mut Widget> {
        let id = *self.selected.first()?;
        self.project.widgets.iter_mut().find(|w| w.id == id)
    }

    /// Check if a widget is selected
    #[allow(dead_code)]
    fn is_selected(&self, id: WidgetId) -> bool {
        self.selected.contains(&id)
    }

    /// Select a single widget (clears other selections)
    #[allow(dead_code)]
    fn select_single(&mut self, id: WidgetId) {
        self.selected = vec![id];
    }

    /// Toggle selection of a widget (for Shift+click multi-select)
    #[allow(dead_code)]
    fn toggle_selection(&mut self, id: WidgetId) {
        if let Some(pos) = self.selected.iter().position(|&x| x == id) {
            self.selected.remove(pos);
        } else {
            self.selected.push(id);
        }
    }

    /// Add widget to selection (for drag box select)
    #[allow(dead_code)]
    fn add_to_selection(&mut self, id: WidgetId) {
        if !self.selected.contains(&id) {
            self.selected.push(id);
        }
    }

    /// Clear all selections
    #[allow(dead_code)]
    fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Save project to file
    fn save_project(&mut self, path: PathBuf) {
        match serde_json::to_string_pretty(&self.project) {
            Ok(json) => match std::fs::write(&path, &json) {
                Ok(_) => {
                    self.current_file = Some(path.clone());
                    self.set_status(format!("Saved to {}", path.display()));
                }
                Err(e) => self.set_status(format!("Save failed: {}", e)),
            },
            Err(e) => self.set_status(format!("Serialization failed: {}", e)),
        }
    }

    /// Load project from file
    fn load_project(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<Project>(&json) {
                Ok(project) => {
                    self.project = project;
                    self.normalize_project_widget_ids();
                    self.selected.clear();
                    self.history.clear();
                    self.current_file = Some(path.clone());
                    self.set_status(format!("Loaded {}", path.display()));
                }
                Err(e) => self.set_status(format!("Parse failed: {}", e)),
            },
            Err(e) => self.set_status(format!("Load failed: {}", e)),
        }
    }

    /// Loads a built-in example project, resets IDs, clears history, and shows a status message.
    fn load_example(&mut self, project: Project, name: &str) {
        self.project = project;
        self.normalize_project_widget_ids();
        self.selected.clear();
        self.history.clear();
        self.current_file = None;
        self.push_undo();
        self.set_status(format!("Loaded example: {}", name));
    }

    /// Returns the host platform and architecture without emojis (e.g. "Windows x86_64")
    pub(crate) fn platform_info() -> String {
        let os = if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else if cfg!(target_os = "linux") {
            "Linux"
        } else if cfg!(target_os = "android") {
            "Android"
        } else if cfg!(target_os = "ios") {
            "iOS"
        } else if cfg!(target_arch = "wasm32") {
            "WASM"
        } else {
            std::env::consts::OS
        };
        format!("{} {}", os, std::env::consts::ARCH)
    }

    /// Set a status message that will be permanently displayed in the status bar
    fn set_status(&mut self, msg: String) {
        self.status_message = msg;
    }

    /// Get widgets in selection rect (for drag-box selection)
    #[allow(dead_code)]
    fn widgets_in_rect(&self, rect: Rect, area_origin: Pos2) -> Vec<WidgetId> {
        self.project
            .widgets
            .iter()
            .filter(|w| {
                let widget_rect = Rect::from_min_size(area_origin + w.pos.to_vec2(), w.size);
                rect.intersects(widget_rect)
            })
            .map(|w| w.id)
            .collect()
    }

    fn preview_panels_ui(&mut self, ctx: &egui::Context) {
        use DockArea::*;

        // Optional: stable visual order
        self.project.widgets.sort_by_key(|w| w.z);

        // Reset live rects each frame
        self.live_top = None;
        self.live_bottom = None;
        self.live_left = None;
        self.live_right = None;
        self.live_center = None;
        self.live_center_origin = None;

        // -------- 1) Bucket root widget IDs by area --------
        let mut top_ids = Vec::new();
        let mut bottom_ids = Vec::new();
        let mut left_ids = Vec::new();
        let mut right_ids = Vec::new();
        let mut center_ids = Vec::new();
        let mut free_ids = Vec::new();

        for w in &self.project.widgets {
            if w.parent.is_none() {
                match w.area {
                    Top => top_ids.push(w.id),
                    Bottom => bottom_ids.push(w.id),
                    Left => left_ids.push(w.id),
                    Right => right_ids.push(w.id),
                    Center => center_ids.push(w.id),
                    Free => free_ids.push(w.id),
                }
            }
        }

        let mut triggered_actions: Vec<ActionEffect> = Vec::new();

        // Top
        if self.project.panel_top_enabled {
            egui::TopBottomPanel::top("rb_top")
                .resizable(true)
                .show(ctx, |ui| {
                    let panel_rect = ui.clip_rect();
                    self.live_top = Some(panel_rect);
                    if self.show_grid {
                        self.draw_grid(ui, panel_rect, self.grid_size);
                    }
                    for wid in &top_ids {
                        Self::draw_widget_tree(
                            ui,
                            panel_rect,
                            self.grid_size,
                            &mut self.selected,
                            *wid,
                            &mut self.project.widgets,
                            &mut triggered_actions,
                            false,
                            1.0,
                        );
                    }
                });
        }

        // Bottom
        if self.project.panel_bottom_enabled {
            egui::TopBottomPanel::bottom("rb_bottom")
                .resizable(true)
                .show(ctx, |ui| {
                    let panel_rect = ui.clip_rect();
                    self.live_bottom = Some(panel_rect);
                    if self.show_grid {
                        self.draw_grid(ui, panel_rect, self.grid_size);
                    }
                    for wid in &bottom_ids {
                        Self::draw_widget_tree(
                            ui,
                            panel_rect,
                            self.grid_size,
                            &mut self.selected,
                            *wid,
                            &mut self.project.widgets,
                            &mut triggered_actions,
                            false,
                            1.0,
                        );
                    }
                });
        }

        // Left
        if self.project.panel_left_enabled {
            egui::SidePanel::left("rb_left")
                .resizable(true)
                .show(ctx, |ui| {
                    let panel_rect = ui.clip_rect();
                    self.live_left = Some(panel_rect);
                    if self.show_grid {
                        self.draw_grid(ui, panel_rect, self.grid_size);
                    }
                    for wid in &left_ids {
                        Self::draw_widget_tree(
                            ui,
                            panel_rect,
                            self.grid_size,
                            &mut self.selected,
                            *wid,
                            &mut self.project.widgets,
                            &mut triggered_actions,
                            false,
                            1.0,
                        );
                    }
                });
        }

        // Right
        if self.project.panel_right_enabled {
            egui::SidePanel::right("rb_right")
                .resizable(true)
                .show(ctx, |ui| {
                    let panel_rect = ui.clip_rect();
                    self.live_right = Some(panel_rect);
                    if self.show_grid {
                        self.draw_grid(ui, panel_rect, self.grid_size);
                    }
                    for wid in &right_ids {
                        Self::draw_widget_tree(
                            ui,
                            panel_rect,
                            self.grid_size,
                            &mut self.selected,
                            *wid,
                            &mut self.project.widgets,
                            &mut triggered_actions,
                            false,
                            1.0,
                        );
                    }
                });
        }

        // Center (design canvas / device screen viewport)
        let viewport_size = self.project.canvas_size;
        let effective_root_mode = self
            .project
            .root_responsive_layout
            .resolve_mode(self.project.root_layout_mode, viewport_size);

        let mut root_ids = center_ids;
        root_ids.extend(free_ids);

        let is_portrait = viewport_size.y > viewport_size.x;

        // ── Compute panel dimensions ──────────────────────────────────────────
        let top_h = if self.project.panel_top_enabled { self.project.panel_top_height } else { 0.0 };
        let bot_h = if self.project.panel_bottom_enabled { self.project.panel_bottom_height } else { 0.0 };
        let left_w = if self.project.panel_left_enabled { self.project.panel_left_width } else { 0.0 };
        let right_w = if self.project.panel_right_enabled { self.project.panel_right_width } else { 0.0 };

        // The center area within the viewport (after subtracting dock panels)
        let center_viewport = egui::Vec2::new(
            (viewport_size.x - left_w - right_w).max(0.0),
            (viewport_size.y - top_h - bot_h).max(0.0),
        );

        if effective_root_mode != LayoutMode::Free {
            let children_info: Vec<(WidgetId, egui::Vec2, SizePolicy, SizePolicy, Option<Align>)> = root_ids
                .iter()
                .filter_map(|&cid| {
                    self.project.widgets.iter().find(|x| x.id == cid && x.props.active).and_then(|cw| {
                        let vis = match cw.props.responsive_vis {
                            ResponsiveVisibility::Always => true,
                            ResponsiveVisibility::PortraitOnly => is_portrait,
                            ResponsiveVisibility::LandscapeOnly => !is_portrait,
                            ResponsiveVisibility::MobileOnly => viewport_size.x < 768.0,
                            ResponsiveVisibility::DesktopOnly => viewport_size.x >= 768.0,
                        };
                        if vis {
                            Some((cw.id, cw.size, cw.props.width_policy, cw.props.height_policy, cw.props.align_self))
                        } else {
                            None
                        }
                    })
                })
                .collect();

            let has_fill_height = children_info.iter().any(|(_, _, _, h_pol, _)| *h_pol == SizePolicy::Fill);
            let hug_height = !has_fill_height;

            let layout_rects = Self::compute_auto_layout(
                center_viewport,
                effective_root_mode,
                self.project.root_layout_gap,
                self.project.root_layout_cols,
                self.project.root_layout_align,
                self.project.root_layout_justify,
                self.project.root_layout_padding,
                &children_info,
                hug_height,
            );

            for (cid, rel_rect) in layout_rects {
                if let Some(cw) = self.project.widgets.iter_mut().find(|x| x.id == cid) {
                    cw.pos = rel_rect.min;
                    cw.size = rel_rect.size().max(egui::Vec2::splat(1.0));
                }
            }
        } else {
            // Free mode: support screen docking for root widgets (center/free area)
            for wid in &root_ids {
                if let Some(cw) = self.project.widgets.iter_mut().find(|x| x.id == *wid) {
                    match cw.area {
                        DockArea::Center | DockArea::Free => {}
                        _ => {} // dock panel widgets are positioned independently below
                    }
                }
            }
        }

        // ── Position dock-panel widgets (always Free within their panel rect) ──
        // These are positioned relative to their panel origin (0,0).
        // Enforce width/height fill based on the panel size.
        let vp = viewport_size;
        for wid in &top_ids {
            if let Some(cw) = self.project.widgets.iter_mut().find(|x| x.id == *wid) {
                cw.pos.y = cw.pos.y.clamp(0.0, (top_h - cw.size.y).max(0.0));
            }
        }
        for wid in &bottom_ids {
            if let Some(cw) = self.project.widgets.iter_mut().find(|x| x.id == *wid) {
                cw.pos.y = cw.pos.y.clamp(0.0, (bot_h - cw.size.y).max(0.0));
            }
        }
        for wid in &left_ids {
            if let Some(cw) = self.project.widgets.iter_mut().find(|x| x.id == *wid) {
                cw.pos.x = cw.pos.x.clamp(0.0, (left_w - cw.size.x).max(0.0));
            }
        }
        for wid in &right_ids {
            if let Some(cw) = self.project.widgets.iter_mut().find(|x| x.id == *wid) {
                cw.pos.x = cw.pos.x.clamp(0.0, (right_w - cw.size.x).max(0.0));
            }
        }
        let _ = vp; // suppress unused warning if no panels enabled

        // Inner scrollable content height (center area only, allows scrolling to any content taller than center)
        let total_content_h = root_ids
            .iter()
            .filter_map(|&wid| self.project.widgets.iter().find(|x| x.id == wid && x.props.active))
            .map(|w| w.pos.y + w.size.y + if effective_root_mode != LayoutMode::Free { self.project.root_layout_padding[2] } else { 0.0 })
            .fold(0.0_f32, f32::max);
        let scroll_h = total_content_h.max(center_viewport.y);
        let inner_canvas_size = egui::vec2(center_viewport.x, scroll_h);

        egui::CentralPanel::default().show(ctx, |ui| {
            // Ctrl + MouseWheel zoom and Ctrl+0 reset
            let center_clip = ui.clip_rect();
            if ui.rect_contains_pointer(center_clip) {
                let (ctrl, scroll_delta) = ui.input(|i| {
                    (
                        i.modifiers.ctrl || i.modifiers.command,
                        i.raw_scroll_delta.y,
                    )
                });
                if ctrl && scroll_delta.abs() > 0.0 {
                    self.canvas_auto_fit = false;
                    let factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
                    self.canvas_zoom = (self.canvas_zoom * factor).clamp(0.1, 4.0);
                }
            }
            if ui.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Num0)) {
                self.canvas_auto_fit = false;
                self.canvas_zoom = 1.0;
            }

            // Viewport info banner & zoom controls
            let screen_name = self.project.screen_preset.display_name();

            // Auto-fit calculation
            if self.canvas_auto_fit {
                let avail = ui.available_size();
                let avail_w = (avail.x - 24.0).max(50.0);
                let avail_h = (avail.y - 38.0).max(50.0);
                let fit_w = avail_w / viewport_size.x;
                let fit_h = avail_h / viewport_size.y;
                self.canvas_zoom = fit_w.min(fit_h).clamp(0.1, 3.0);
            }
            let zoom = self.canvas_zoom.clamp(0.1, 4.0);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Viewport: {} ({:.0} x {:.0})",
                        screen_name, viewport_size.x, viewport_size.y
                    ))
                    .strong()
                    .size(11.0),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Root Layout: {}", effective_root_mode.display_name()))
                        .weak()
                        .size(11.0),
                );
                ui.separator();

                ui.label(egui::RichText::new("Zoom:").weak().size(11.0));
                if ui.button("-").on_hover_text("Zoom out (Ctrl + Wheel down)").clicked() {
                    self.canvas_auto_fit = false;
                    self.canvas_zoom = (self.canvas_zoom - 0.1).max(0.1);
                }

                let pct = (zoom * 100.0).round() as i32;
                let mut selected_preset = pct;
                let zoom_label = format!("{}%", pct);
                egui::ComboBox::from_id_salt("canvas_zoom_combo")
                    .selected_text(zoom_label)
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for &val in &[25, 50, 75, 100, 125, 150, 200, 300] {
                            if ui.selectable_value(&mut selected_preset, val, format!("{}%", val)).clicked() {
                                self.canvas_auto_fit = false;
                                self.canvas_zoom = val as f32 / 100.0;
                            }
                        }
                    });

                if ui.button("+").on_hover_text("Zoom in (Ctrl + Wheel up)").clicked() {
                    self.canvas_auto_fit = false;
                    self.canvas_zoom = (self.canvas_zoom + 0.1).min(4.0);
                }

                if ui.button("100%").on_hover_text("Reset zoom to 100% (Ctrl+0)").clicked() {
                    self.canvas_auto_fit = false;
                    self.canvas_zoom = 1.0;
                }

                let fit_btn = ui.selectable_label(self.canvas_auto_fit, "Fit")
                    .on_hover_text("Fit screen to available window space");
                if fit_btn.clicked() {
                    self.canvas_auto_fit = !self.canvas_auto_fit;
                }
            });
            ui.add_space(2.0);

            // Container frame for device screen (full viewport size)
            let scaled_viewport_size = viewport_size * zoom;

            egui::Frame::NONE
                .stroke(Stroke::new(1.0_f32, Color32::from_gray(65)))
                .corner_radius(4.0)
                .fill(ui.visuals().panel_fill)
                .show(ui, |ui| {
                    // The outer scroll area is the full device viewport size
                    egui::ScrollArea::both()
                        .id_salt("device_screen_outer_scroll")
                        .max_width(scaled_viewport_size.x)
                        .max_height(scaled_viewport_size.y)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Allocate the full viewport for interaction
                            let (full_resp, _full_painter) = ui.allocate_painter(
                                scaled_viewport_size,
                                egui::Sense::click(),
                            );
                            let vp_origin = full_resp.rect.min;

                            // Background fill for entire device viewport
                            _full_painter.rect_filled(full_resp.rect, 0.0, ui.visuals().window_fill());

                            if self.show_grid {
                                self.draw_grid(ui, full_resp.rect, self.grid_size * zoom);
                            }

                            // ── Panel separator colors ──────────────────────────
                            let panel_bg = Color32::from_rgba_premultiplied(30, 30, 45, 230);
                            let panel_sep = Color32::from_gray(70);
                            let panel_label_color = Color32::from_gray(120);

                            // ── TOP PANEL ─────────────────────────────────────────
                            if self.project.panel_top_enabled {
                                let scaled_top_h = top_h * zoom;
                                let top_rect = Rect::from_min_size(
                                    vp_origin,
                                    egui::vec2(scaled_viewport_size.x, scaled_top_h),
                                );
                                self.live_top = Some(top_rect);
                                _full_painter.rect_filled(top_rect, 0.0, panel_bg);
                                _full_painter.line_segment(
                                    [top_rect.left_bottom(), top_rect.right_bottom()],
                                    Stroke::new(1.0_f32, panel_sep),
                                );
                                if top_ids.is_empty() {
                                    _full_painter.text(
                                        top_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "[ Top Panel ]",
                                        egui::FontId::proportional(11.0 * zoom),
                                        panel_label_color,
                                    );
                                }
                                for wid in &top_ids {
                                    if let Some(w) = self.project.widgets.iter().find(|x| x.id == *wid) {
                                        let vis = match w.props.responsive_vis {
                                            ResponsiveVisibility::Always => true,
                                            ResponsiveVisibility::PortraitOnly => is_portrait,
                                            ResponsiveVisibility::LandscapeOnly => !is_portrait,
                                            ResponsiveVisibility::MobileOnly => viewport_size.x < 768.0,
                                            ResponsiveVisibility::DesktopOnly => viewport_size.x >= 768.0,
                                        };
                                        if !vis { continue; }
                                    }
                                    Self::draw_widget_tree(
                                        ui,
                                        top_rect,
                                        self.grid_size,
                                        &mut self.selected,
                                        *wid,
                                        &mut self.project.widgets,
                                        &mut triggered_actions,
                                        false,
                                        zoom,
                                    );
                                }
                            }

                            // ── BOTTOM PANEL ──────────────────────────────────────
                            if self.project.panel_bottom_enabled {
                                let scaled_bot_h = bot_h * zoom;
                                let bot_rect = Rect::from_min_size(
                                    vp_origin + egui::vec2(0.0, scaled_viewport_size.y - scaled_bot_h),
                                    egui::vec2(scaled_viewport_size.x, scaled_bot_h),
                                );
                                self.live_bottom = Some(bot_rect);
                                _full_painter.rect_filled(bot_rect, 0.0, panel_bg);
                                _full_painter.line_segment(
                                    [bot_rect.left_top(), bot_rect.right_top()],
                                    Stroke::new(1.0_f32, panel_sep),
                                );
                                if bottom_ids.is_empty() {
                                    _full_painter.text(
                                        bot_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "[ Bottom Panel ]",
                                        egui::FontId::proportional(11.0 * zoom),
                                        panel_label_color,
                                    );
                                }
                                for wid in &bottom_ids {
                                    if let Some(w) = self.project.widgets.iter().find(|x| x.id == *wid) {
                                        let vis = match w.props.responsive_vis {
                                            ResponsiveVisibility::Always => true,
                                            ResponsiveVisibility::PortraitOnly => is_portrait,
                                            ResponsiveVisibility::LandscapeOnly => !is_portrait,
                                            ResponsiveVisibility::MobileOnly => viewport_size.x < 768.0,
                                            ResponsiveVisibility::DesktopOnly => viewport_size.x >= 768.0,
                                        };
                                        if !vis { continue; }
                                    }
                                    Self::draw_widget_tree(
                                        ui,
                                        bot_rect,
                                        self.grid_size,
                                        &mut self.selected,
                                        *wid,
                                        &mut self.project.widgets,
                                        &mut triggered_actions,
                                        false,
                                        zoom,
                                    );
                                }
                            }

                            // ── LEFT PANEL ────────────────────────────────────────
                            if self.project.panel_left_enabled {
                                let scaled_top_h = top_h * zoom;
                                let scaled_bot_h = bot_h * zoom;
                                let scaled_left_w = left_w * zoom;
                                let mid_h = scaled_viewport_size.y - scaled_top_h - scaled_bot_h;
                                let left_rect = Rect::from_min_size(
                                    vp_origin + egui::vec2(0.0, scaled_top_h),
                                    egui::vec2(scaled_left_w, mid_h.max(0.0)),
                                );
                                self.live_left = Some(left_rect);
                                _full_painter.rect_filled(left_rect, 0.0, panel_bg);
                                _full_painter.line_segment(
                                    [left_rect.right_top(), left_rect.right_bottom()],
                                    Stroke::new(1.0_f32, panel_sep),
                                );
                                if left_ids.is_empty() {
                                    _full_painter.text(
                                        left_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "[ Left ]",
                                        egui::FontId::proportional(10.0 * zoom),
                                        panel_label_color,
                                    );
                                }
                                for wid in &left_ids {
                                    if let Some(w) = self.project.widgets.iter().find(|x| x.id == *wid) {
                                        let vis = match w.props.responsive_vis {
                                            ResponsiveVisibility::Always => true,
                                            ResponsiveVisibility::PortraitOnly => is_portrait,
                                            ResponsiveVisibility::LandscapeOnly => !is_portrait,
                                            ResponsiveVisibility::MobileOnly => viewport_size.x < 768.0,
                                            ResponsiveVisibility::DesktopOnly => viewport_size.x >= 768.0,
                                        };
                                        if !vis { continue; }
                                    }
                                    Self::draw_widget_tree(
                                        ui,
                                        left_rect,
                                        self.grid_size,
                                        &mut self.selected,
                                        *wid,
                                        &mut self.project.widgets,
                                        &mut triggered_actions,
                                        false,
                                        zoom,
                                    );
                                }
                            }

                            // ── RIGHT PANEL ───────────────────────────────────────
                            if self.project.panel_right_enabled {
                                let scaled_top_h = top_h * zoom;
                                let scaled_bot_h = bot_h * zoom;
                                let scaled_right_w = right_w * zoom;
                                let mid_h = scaled_viewport_size.y - scaled_top_h - scaled_bot_h;
                                let right_rect = Rect::from_min_size(
                                    vp_origin + egui::vec2(scaled_viewport_size.x - scaled_right_w, scaled_top_h),
                                    egui::vec2(scaled_right_w, mid_h.max(0.0)),
                                );
                                self.live_right = Some(right_rect);
                                _full_painter.rect_filled(right_rect, 0.0, panel_bg);
                                _full_painter.line_segment(
                                    [right_rect.left_top(), right_rect.left_bottom()],
                                    Stroke::new(1.0_f32, panel_sep),
                                );
                                if right_ids.is_empty() {
                                    _full_painter.text(
                                        right_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "[ Right ]",
                                        egui::FontId::proportional(10.0 * zoom),
                                        panel_label_color,
                                    );
                                }
                                for wid in &right_ids {
                                    if let Some(w) = self.project.widgets.iter().find(|x| x.id == *wid) {
                                        let vis = match w.props.responsive_vis {
                                            ResponsiveVisibility::Always => true,
                                            ResponsiveVisibility::PortraitOnly => is_portrait,
                                            ResponsiveVisibility::LandscapeOnly => !is_portrait,
                                            ResponsiveVisibility::MobileOnly => viewport_size.x < 768.0,
                                            ResponsiveVisibility::DesktopOnly => viewport_size.x >= 768.0,
                                        };
                                        if !vis { continue; }
                                    }
                                    Self::draw_widget_tree(
                                        ui,
                                        right_rect,
                                        self.grid_size,
                                        &mut self.selected,
                                        *wid,
                                        &mut self.project.widgets,
                                        &mut triggered_actions,
                                        false,
                                        zoom,
                                    );
                                }
                            }

                            // ── CENTER / CONTENT AREA ─────────────────────────────
                            {
                                let scaled_top_h = top_h * zoom;
                                let scaled_bot_h = bot_h * zoom;
                                let scaled_left_w = left_w * zoom;
                                let scaled_right_w = right_w * zoom;
                                let scaled_inner_size = inner_canvas_size * zoom;
                                let center_origin = vp_origin + egui::vec2(scaled_left_w, scaled_top_h);
                                let center_visible_w = (scaled_viewport_size.x - scaled_left_w - scaled_right_w).max(0.0);
                                let center_visible_h = (scaled_viewport_size.y - scaled_top_h - scaled_bot_h).max(0.0);
                                let center_clip_rect = Rect::from_min_size(center_origin, egui::vec2(center_visible_w, center_visible_h));
                                let center_alloc_size = egui::vec2(scaled_inner_size.x, scaled_inner_size.y.max(center_visible_h));

                                self.live_center = Some(center_clip_rect);
                                self.live_center_origin = Some(center_origin);

                                // Draw center area background (slightly different to distinguish from panels)
                                _full_painter.rect_filled(center_clip_rect, 0.0, ui.visuals().window_fill());

                                // Use a child ui scoped to the center region for center widgets
                                let center_ui_builder = UiBuilder::new()
                                    .max_rect(center_clip_rect)
                                    .sense(Sense::click());
                                let mut center_ui = ui.new_child(center_ui_builder);

                                // Inner scroll for center content
                                egui::ScrollArea::vertical()
                                    .id_salt("device_center_scroll")
                                    .max_width(center_visible_w)
                                    .max_height(center_visible_h)
                                    .auto_shrink([false, false])
                                    .show(&mut center_ui, |center_ui| {
                                        let (center_resp, _center_painter) = center_ui.allocate_painter(
                                            center_alloc_size,
                                            egui::Sense::click(),
                                        );
                                        let center_rect = Rect::from_min_size(center_resp.rect.min, center_alloc_size);

                                        for wid in &root_ids {
                                            if let Some(w) = self.project.widgets.iter().find(|x| x.id == *wid) {
                                                let vis = match w.props.responsive_vis {
                                                    ResponsiveVisibility::Always => true,
                                                    ResponsiveVisibility::PortraitOnly => is_portrait,
                                                    ResponsiveVisibility::LandscapeOnly => !is_portrait,
                                                    ResponsiveVisibility::MobileOnly => viewport_size.x < 768.0,
                                                    ResponsiveVisibility::DesktopOnly => viewport_size.x >= 768.0,
                                                };
                                                if !vis { continue; }
                                            }
                                            Self::draw_widget_tree(
                                                center_ui,
                                                center_rect,
                                                self.grid_size,
                                                &mut self.selected,
                                                *wid,
                                                &mut self.project.widgets,
                                                &mut triggered_actions,
                                                effective_root_mode != LayoutMode::Free,
                                                zoom,
                                            );
                                        }

                                        if center_resp.clicked() {
                                            self.selected.clear();
                                        }
                                    });
                            }

                            if full_resp.clicked() {
                                self.selected.clear();
                            }
                        });
                });

            // --- Drag ghost + drop ---
            if let Some(kind) = self.spawning {
                if let Some(mouse) = ui.ctx().pointer_interact_pos() {
                    // Use centralized default_size from WidgetKind scaled by zoom
                    let ghost_size = kind.default_size() * zoom;
                    let ghost = egui::Rect::from_center_size(mouse, ghost_size);
                    let layer = egui::LayerId::new(egui::Order::Tooltip, Id::new("ghost"));
                    let painter = ui.ctx().layer_painter(layer);
                    painter.rect_filled(ghost, 4.0, Color32::from_gray(40));
                    painter.rect_stroke(
                        ghost,
                        CornerRadius::same(4),
                        Stroke::new(1.0_f32, Color32::LIGHT_BLUE),
                        egui::StrokeKind::Outside,
                    );

                    // highlight target panel
                    let area = self.area_at(mouse);
                    if let Some(hilite) = match area {
                        DockArea::Top => self.live_top,
                        DockArea::Bottom => self.live_bottom,
                        DockArea::Left => self.live_left,
                        DockArea::Right => self.live_right,
                        DockArea::Center | DockArea::Free => self.live_center,
                    } {
                        painter.rect_stroke(
                            hilite,
                            CornerRadius::same(6),
                            Stroke::new(2.0_f32, Color32::LIGHT_BLUE),
                            egui::StrokeKind::Outside,
                        );
                    }
                }

                if ui.input(|i| i.pointer.any_released()) {
                    if let Some(pos) = ui.ctx().pointer_interact_pos() {
                        let area = self.area_at(pos);
                        if let Some(origin) = self.origin_for_area(area) {
                            self.spawn_widget(kind, pos, area, origin);
                        }
                    }
                    self.spawning = None;
                }
            }
        });

        // Apply any actions that were triggered during this frame
        for effect in triggered_actions {
            Self::apply_action_effect(&effect, &mut self.project.widgets);
        }
    }

    fn draw_grid(&self, ui: &mut egui::Ui, rect: Rect, grid_step: f32) {
        let painter = ui.painter_at(rect);
        let g = grid_step.max(4.0);
        let cols = (rect.width() / g) as i32;
        let rows = (rect.height() / g) as i32;
        for c in 0..=cols {
            let x = rect.left() + c as f32 * g;
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.bottom())],
                Stroke::new(1.0_f32, Color32::from_gray(40)),
            );
        }
        for r in 0..=rows {
            let y = rect.top() + r as f32 * g;
            painter.line_segment(
                [pos2(rect.left(), y), pos2(rect.right(), y)],
                Stroke::new(1.0_f32, Color32::from_gray(40)),
            );
        }
    }

    pub(crate) fn apply_action_effect(effect: &ActionEffect, widgets: &mut [Widget]) {
        match effect {
            ActionEffect::ShowWidget(target) => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.initially_visible = true;
                }
            }
            ActionEffect::HideWidget(target) => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.initially_visible = false;
                }
            }
            ActionEffect::ToggleWidget(target) => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.initially_visible = !tw.props.initially_visible;
                }
            }
            ActionEffect::SetText { target, text } => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.text = text.clone();
                }
            }
            ActionEffect::SwitchTab { target, tab_index } => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.selected = *tab_index;
                }
            }
            ActionEffect::OpenModal(target) => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.checked = true;
                }
            }
            ActionEffect::CloseModal(target) => {
                if let Some(tw) = widgets.iter_mut().find(|w| w.id == *target) {
                    tw.props.checked = false;
                }
            }
            ActionEffect::CustomRustCode(_) => {}
        }
    }

    fn check_widget_actions(
        resp: &egui::Response,
        actions: &[WidgetAction],
        triggered: &mut Vec<ActionEffect>,
    ) {
        for action in actions {
            let fired = match action.trigger {
                ActionTrigger::OnClick => resp.clicked(),
                ActionTrigger::OnHover => resp.hovered(),
                ActionTrigger::OnChanged => resp.changed(),
                ActionTrigger::OnDoubleClick => resp.double_clicked(),
            };
            if fired {
                triggered.push(action.effect.clone());
            }
        }
    }

    /// Computes relative bounding rects for children in an auto-layout container.
    /// Returns a list of (WidgetId, Rect) with positions relative to the container inner origin (0, 0).
    pub(crate) fn compute_auto_layout(
        container_size: egui::Vec2,
        mode: LayoutMode,
        gap: f32,
        cols: usize,
        align: Align,
        justify: Justify,
        padding: [f32; 4],
        children_props: &[(WidgetId, egui::Vec2, SizePolicy, SizePolicy, Option<Align>)],
        hug_height: bool,
    ) -> Vec<(WidgetId, Rect)> {
        let mut results = Vec::new();
        if children_props.is_empty() {
            return results;
        }

        let pad_top = padding[0];
        let pad_right = padding[1];
        let pad_bottom = padding[2];
        let pad_left = padding[3];

        let avail_w = (container_size.x - pad_left - pad_right).max(0.0);
        let avail_h = (container_size.y - pad_top - pad_bottom).max(0.0);

        match mode {
            LayoutMode::Free => {
                // Not used here
            }
            LayoutMode::Row => {
                let n = children_props.len();
                let total_gaps = gap * (n.saturating_sub(1) as f32);
                let mut fixed_w_total = 0.0;
                let mut fill_count = 0;

                for &(_, sz, w_pol, _, _) in children_props {
                    match w_pol {
                        SizePolicy::Fixed => fixed_w_total += sz.x,
                        SizePolicy::Percent(p) => fixed_w_total += (avail_w * (p / 100.0)).max(0.0),
                        SizePolicy::Fill => fill_count += 1,
                    }
                }

                let remaining_w = (avail_w - fixed_w_total - total_gaps).max(0.0);
                let fill_unit_w = if fill_count > 0 { remaining_w / fill_count as f32 } else { 0.0 };

                let total_content_w = fixed_w_total + (fill_unit_w * fill_count as f32) + total_gaps;
                let spare_main = (avail_w - total_content_w).max(0.0);

                let (start_offset_x, gap_stride) = match justify {
                    Justify::Start => (0.0, gap),
                    Justify::Center => (spare_main / 2.0, gap),
                    Justify::End => (spare_main, gap),
                    Justify::SpaceBetween => {
                        let dynamic_gap = if n > 1 { (avail_w - (total_content_w - total_gaps)) / (n - 1) as f32 } else { 0.0 };
                        (0.0, dynamic_gap)
                    }
                    Justify::SpaceAround => {
                        let unit = if n > 0 { spare_main / (n as f32 * 2.0) } else { 0.0 };
                        (unit, gap + unit * 2.0)
                    }
                    Justify::SpaceEvenly => {
                        let unit = if n > 0 { spare_main / (n as f32 + 1.0) } else { 0.0 };
                        (unit, gap + unit)
                    }
                };

                let mut current_x = pad_left + start_offset_x;
                for &(cid, sz, w_pol, h_pol, self_align) in children_props {
                    let w = match w_pol {
                        SizePolicy::Fixed => sz.x,
                        SizePolicy::Percent(p) => (avail_w * (p / 100.0)).max(0.0),
                        SizePolicy::Fill => fill_unit_w,
                    };

                    let effective_align = self_align.unwrap_or(align);
                    let (y, h) = match effective_align {
                        Align::Start => {
                            let h = match h_pol {
                                SizePolicy::Fixed => sz.y,
                                SizePolicy::Percent(p) => if hug_height { sz.y } else { (avail_h * (p / 100.0)).max(0.0) },
                                SizePolicy::Fill => if hug_height { sz.y } else { avail_h },
                            };
                            (pad_top, h)
                        }
                        Align::Center => {
                            let h = match h_pol {
                                SizePolicy::Fixed => sz.y,
                                SizePolicy::Percent(p) => if hug_height { sz.y } else { (avail_h * (p / 100.0)).max(0.0) },
                                SizePolicy::Fill => if hug_height { sz.y } else { avail_h },
                            };
                            let y = if hug_height { pad_top } else { pad_top + (avail_h - h).max(0.0) / 2.0 };
                            (y, h)
                        }
                        Align::End => {
                            let h = match h_pol {
                                SizePolicy::Fixed => sz.y,
                                SizePolicy::Percent(p) => if hug_height { sz.y } else { (avail_h * (p / 100.0)).max(0.0) },
                                SizePolicy::Fill => if hug_height { sz.y } else { avail_h },
                            };
                            let y = if hug_height { pad_top } else { pad_top + (avail_h - h).max(0.0) };
                            (y, h)
                        }
                        Align::Stretch => {
                            let h = if hug_height { sz.y } else { avail_h };
                            (pad_top, h)
                        }
                    };

                    results.push((cid, Rect::from_min_size(pos2(current_x, y), vec2(w.max(1.0), h.max(1.0)))));
                    current_x += w + gap_stride;
                }
            }
            LayoutMode::Column => {
                let n = children_props.len();
                let total_gaps = gap * (n.saturating_sub(1) as f32);
                let mut fixed_h_total = 0.0;
                let mut fill_count = 0;

                for &(_, sz, _, h_pol, _) in children_props {
                    match h_pol {
                        SizePolicy::Fixed => fixed_h_total += sz.y,
                        SizePolicy::Percent(p) => {
                            if hug_height {
                                fixed_h_total += sz.y;
                            } else {
                                fixed_h_total += (avail_h * (p / 100.0)).max(0.0);
                            }
                        }
                        SizePolicy::Fill => {
                            if hug_height {
                                fixed_h_total += sz.y;
                            } else {
                                fill_count += 1;
                            }
                        }
                    }
                }

                let remaining_h = if hug_height {
                    0.0
                } else {
                    (avail_h - fixed_h_total - total_gaps).max(0.0)
                };
                let fill_unit_h = if fill_count > 0 { remaining_h / fill_count as f32 } else { 0.0 };

                let total_content_h = fixed_h_total + (fill_unit_h * fill_count as f32) + total_gaps;
                let spare_main = if hug_height { 0.0 } else { (avail_h - total_content_h).max(0.0) };

                let (start_offset_y, gap_stride) = match justify {
                    Justify::Start => (0.0, gap),
                    Justify::Center => (spare_main / 2.0, gap),
                    Justify::End => (spare_main, gap),
                    Justify::SpaceBetween => {
                        let dynamic_gap = if n > 1 { (avail_h - (total_content_h - total_gaps)) / (n - 1) as f32 } else { 0.0 };
                        (0.0, dynamic_gap)
                    }
                    Justify::SpaceAround => {
                        let unit = if n > 0 { spare_main / (n as f32 * 2.0) } else { 0.0 };
                        (unit, gap + unit * 2.0)
                    }
                    Justify::SpaceEvenly => {
                        let unit = if n > 0 { spare_main / (n as f32 + 1.0) } else { 0.0 };
                        (unit, gap + unit)
                    }
                };

                let mut current_y = pad_top + start_offset_y;
                for &(cid, sz, w_pol, h_pol, self_align) in children_props {
                    let h = match h_pol {
                        SizePolicy::Fixed => sz.y,
                        SizePolicy::Percent(p) => {
                            if hug_height {
                                sz.y
                            } else {
                                (avail_h * (p / 100.0)).max(0.0)
                            }
                        }
                        SizePolicy::Fill => {
                            if hug_height {
                                sz.y
                            } else {
                                fill_unit_h
                            }
                        }
                    };

                    let effective_align = self_align.unwrap_or(align);
                    let (x, w) = match effective_align {
                        Align::Start => {
                            let w = match w_pol {
                                SizePolicy::Fixed => sz.x,
                                SizePolicy::Percent(p) => (avail_w * (p / 100.0)).max(0.0),
                                SizePolicy::Fill => avail_w,
                            };
                            (pad_left, w)
                        }
                        Align::Center => {
                            let w = match w_pol {
                                SizePolicy::Fixed => sz.x,
                                SizePolicy::Percent(p) => (avail_w * (p / 100.0)).max(0.0),
                                SizePolicy::Fill => avail_w,
                            };
                            let x = pad_left + (avail_w - w).max(0.0) / 2.0;
                            (x, w)
                        }
                        Align::End => {
                            let w = match w_pol {
                                SizePolicy::Fixed => sz.x,
                                SizePolicy::Percent(p) => (avail_w * (p / 100.0)).max(0.0),
                                SizePolicy::Fill => avail_w,
                            };
                            let x = pad_left + (avail_w - w).max(0.0);
                            (x, w)
                        }
                        Align::Stretch => {
                            (pad_left, avail_w)
                        }
                    };

                    results.push((cid, Rect::from_min_size(pos2(x, current_y), vec2(w.max(1.0), h.max(1.0)))));
                    current_y += h + gap_stride;
                }
            }
            LayoutMode::WrapRow => {
                let mut current_x = pad_left;
                let mut current_y = pad_top;
                let mut row_h: f32 = 0.0;

                for &(cid, sz, w_pol, h_pol, _) in children_props {
                    let w = match w_pol {
                        SizePolicy::Fixed => sz.x,
                        SizePolicy::Percent(p) => (avail_w * (p / 100.0)).max(0.0),
                        SizePolicy::Fill => sz.x,
                    };
                    let h = match h_pol {
                        SizePolicy::Fixed => sz.y,
                        SizePolicy::Percent(p) => if hug_height { sz.y } else { (avail_h * (p / 100.0)).max(0.0) },
                        SizePolicy::Fill => sz.y,
                    };

                    if current_x + w > pad_left + avail_w && current_x > pad_left {
                        current_x = pad_left;
                        current_y += row_h + gap;
                        row_h = 0.0;
                    }

                    results.push((cid, Rect::from_min_size(pos2(current_x, current_y), vec2(w.max(1.0), h.max(1.0)))));
                    current_x += w + gap;
                    row_h = row_h.max(h);
                }
            }
            LayoutMode::Grid => {
                let num_cols = cols.max(1);
                let total_gaps = gap * (num_cols.saturating_sub(1) as f32);
                let cell_w = ((avail_w - total_gaps) / num_cols as f32).max(10.0);

                let mut row_idx = 0;
                let mut col_idx = 0;
                let mut row_y = pad_top;
                let mut current_row_max_h: f32 = 0.0;

                for &(cid, sz, w_pol, h_pol, _) in children_props {
                    let w = match w_pol {
                        SizePolicy::Fixed => sz.x.min(cell_w),
                        SizePolicy::Percent(p) => (cell_w * (p / 100.0)).max(0.0),
                        SizePolicy::Fill => cell_w,
                    };
                    let h = match h_pol {
                        SizePolicy::Fixed => sz.y,
                        SizePolicy::Percent(p) => if hug_height { sz.y } else { (avail_h * (p / 100.0)).max(0.0) },
                        SizePolicy::Fill => sz.y,
                    };

                    let cell_x = pad_left + (col_idx as f32) * (cell_w + gap);
                    results.push((cid, Rect::from_min_size(pos2(cell_x, row_y), vec2(w.max(1.0), h.max(1.0)))));

                    current_row_max_h = current_row_max_h.max(h);
                    col_idx += 1;
                    if col_idx >= num_cols {
                        col_idx = 0;
                        row_idx += 1;
                        row_y += current_row_max_h + gap;
                        current_row_max_h = 0.0;
                    }
                }
                let _ = row_idx;
            }
        }

        results
    }

    fn draw_widget_tree(
        ui: &mut egui::Ui,
        container_rect: Rect,
        grid_size: f32,
        selected: &mut Vec<WidgetId>,
        widget_id: WidgetId,
        widgets: &mut [Widget],
        triggered: &mut Vec<ActionEffect>,
        is_in_auto_layout: bool,
        zoom: f32,
    ) {
        let idx = match widgets.iter().position(|w| w.id == widget_id) {
            Some(i) => i,
            None => return,
        };

        let is_edit_mode = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(Id::new("edit_mode")))
            .unwrap_or(true);

        if !widgets[idx].props.active {
            return;
        }

        // ── Responsive visibility check ──────────────────────────────────────
        let logical_container_size = container_rect.size() / zoom;
        if !widgets[idx].props.responsive_vis.is_visible(logical_container_size) {
            return;
        }

        if !is_edit_mode {
            if !widgets[idx].props.initially_visible {
                return;
            }
            if widgets[idx].kind == WidgetKind::Window && !widgets[idx].props.checked {
                return;
            }
        }

        let is_container = widgets[idx].kind.is_container();
        let has_children = widgets.iter().any(|w| w.parent == Some(widget_id) && w.props.active);
        let op = widgets[idx].props.opacity.clamp(0.0, 1.0);

        if (op - 1.0).abs() > 0.001 {
            ui.scope(|ui| {
                ui.set_opacity(op);
                Self::draw_widget(
                    ui,
                    container_rect,
                    grid_size,
                    selected,
                    &mut widgets[idx],
                    has_children,
                    triggered,
                    is_in_auto_layout,
                    zoom,
                );
            });
        } else {
            Self::draw_widget(
                ui,
                container_rect,
                grid_size,
                selected,
                &mut widgets[idx],
                has_children,
                triggered,
                is_in_auto_layout,
                zoom,
            );
        }

        const COLLAPSED_HEADER_H: f32 = 26.0;

        if widgets[idx].kind == WidgetKind::CollapsingHeader {
            let is_open = widgets[idx].props.checked;
            if !is_open && widgets[idx].size.y > COLLAPSED_HEADER_H {
                // Header should be collapsed, but size.y is still at expanded height!
                let old_h = widgets[idx].size.y;
                widgets[idx].props.expanded_height = Some(old_h);
                widgets[idx].size.y = COLLAPSED_HEADER_H;

                let delta_h = COLLAPSED_HEADER_H - old_h;
                let parent_id = widgets[idx].parent;
                let my_top = widgets[idx].pos.y;
                let my_bottom = my_top + old_h;
                // Shift sibling widgets below it in Free mode
                for other in widgets.iter_mut() {
                    if other.id != widget_id && other.parent == parent_id && other.pos.y >= my_bottom - 12.0 {
                        other.pos.y = (other.pos.y + delta_h).max(my_top + COLLAPSED_HEADER_H);
                    }
                }
            } else if is_open && widgets[idx].size.y <= COLLAPSED_HEADER_H {
                // Header should be open, but size.y is at collapsed height!
                let target_h = widgets[idx].props.expanded_height.unwrap_or(80.0).max(COLLAPSED_HEADER_H + 10.0);
                let delta_h = target_h - widgets[idx].size.y;
                widgets[idx].size.y = target_h;

                let parent_id = widgets[idx].parent;
                let my_top = widgets[idx].pos.y;
                // Shift sibling widgets back down in Free mode
                for other in widgets.iter_mut() {
                    if other.id != widget_id && other.parent == parent_id && other.pos.y >= my_top + COLLAPSED_HEADER_H - 4.0 {
                        other.pos.y += delta_h;
                    }
                }
            }
        }

        if is_container {
            let w = &widgets[idx];

            // ── CollapsingHeader: when closed, children are hidden ───────────
            if w.kind == WidgetKind::CollapsingHeader && !w.props.checked {
                return;
            }

            let widget_rect = Rect::from_min_size(container_rect.min + (w.pos * zoom).to_vec2(), w.size * zoom);
            let offset = Self::container_content_offset(w.kind, &w.props.text, w.props.show_tabs) * zoom;
            let inner_rect = if w.kind == WidgetKind::Container {
                widget_rect
            } else {
                Rect::from_min_size(
                    widget_rect.min + offset,
                    (widget_rect.size() - offset - vec2(6.0 * zoom, 6.0 * zoom)).max(vec2(10.0 * zoom, 10.0 * zoom)),
                )
            };

            // Filter children: active, matching parent, and matching TabBar tab if applicable
            let parent_tab_selected = w.props.selected;
            let is_tabbar = w.kind == WidgetKind::TabBar;

            let mut children: Vec<(WidgetId, i32)> = widgets
                .iter()
                .filter(|cw| {
                    if cw.parent != Some(widget_id) || !cw.props.active {
                        return false;
                    }
                    if is_tabbar {
                        if let Some(assigned_tab) = cw.props.tab_page {
                            if assigned_tab != parent_tab_selected {
                                return false;
                            }
                        }
                    }
                    true
                })
                .map(|cw| (cw.id, cw.z))
                .collect();
            children.sort_by_key(|&(_, z)| z);

            let effective_layout_mode = w.props.responsive_layout.resolve_mode(w.props.layout_mode, logical_container_size);

            if effective_layout_mode == LayoutMode::Free {
                for (cid, _) in &children {
                    Self::draw_widget_tree(ui, inner_rect, grid_size, selected, *cid, widgets, triggered, false, zoom);
                }

                // Auto-sizing height in Free mode
                if widgets[idx].props.auto_size_y && !children.is_empty() {
                    let max_bottom = children
                        .iter()
                        .filter_map(|&(cid, _)| widgets.iter().find(|x| x.id == cid).map(|cw| cw.pos.y + cw.size.y))
                        .fold(0.0_f32, f32::max);
                    let required_h = max_bottom + (offset.y / zoom) + 6.0;
                    if required_h > 24.0 && (widgets[idx].size.y - required_h).abs() > 0.5 {
                        widgets[idx].size.y = required_h;
                        widgets[idx].props.expanded_height = Some(required_h);
                    }
                }
            } else {
                let children_info: Vec<(WidgetId, egui::Vec2, SizePolicy, SizePolicy, Option<Align>)> = children
                    .iter()
                    .filter_map(|&(cid, _)| {
                        widgets.iter().find(|x| x.id == cid).map(|cw| {
                            (cw.id, cw.size, cw.props.width_policy, cw.props.height_policy, cw.props.align_self)
                        })
                    })
                    .collect();

                let layout_rects = Self::compute_auto_layout(
                    inner_rect.size() / zoom,
                    effective_layout_mode,
                    w.props.layout_gap,
                    w.props.layout_cols,
                    w.props.layout_align,
                    w.props.layout_justify,
                    w.props.layout_padding,
                    &children_info,
                    w.props.auto_size_y,
                );

                // Auto-sizing height in auto-layout mode
                if widgets[idx].props.auto_size_y && !layout_rects.is_empty() {
                    let max_bottom = layout_rects.iter().map(|(_, r)| r.max.y).fold(0.0_f32, f32::max);
                    let required_h = max_bottom + (offset.y / zoom) + w.props.layout_padding[2] + 6.0;
                    if required_h > 24.0 && (widgets[idx].size.y - required_h).abs() > 0.5 {
                        widgets[idx].size.y = required_h;
                        widgets[idx].props.expanded_height = Some(required_h);
                    }
                }

                for (cid, rel_rect) in layout_rects {
                    let child_container_rect = Rect::from_min_size(
                        inner_rect.min + (rel_rect.min * zoom).to_vec2(),
                        rel_rect.size() * zoom,
                    );
                    if let Some(cw) = widgets.iter_mut().find(|x| x.id == cid) {
                        cw.pos = egui::Pos2::ZERO;
                        cw.size = rel_rect.size().max(egui::Vec2::splat(1.0));
                    }
                    Self::draw_widget_tree(ui, child_container_rect, grid_size, selected, cid, widgets, triggered, true, zoom);
                }
            }
        }
    }

    fn draw_widget(
        ui: &mut egui::Ui,
        canvas_rect: Rect,
        grid: f32,
        selected: &mut Vec<WidgetId>,
        w: &mut Widget,
        has_children: bool,
        triggered: &mut Vec<ActionEffect>,
        is_in_auto_layout: bool,
        zoom: f32,
    ) {
        let is_edit_mode = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(Id::new("edit_mode")))
            .unwrap_or(true);
        let scaled_size = w.size * zoom;
        let rect = Rect::from_min_size(canvas_rect.min + (w.pos * zoom).to_vec2(), scaled_size);
        ui.push_id(("widget", w.id), |ui| {
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                if (zoom - 1.0).abs() > 0.01 {
                    for (_, font_id) in ui.style_mut().text_styles.iter_mut() {
                        font_id.size = (font_id.size * zoom).max(6.0);
                    }
                    ui.spacing_mut().item_spacing *= zoom;
                    ui.spacing_mut().button_padding *= zoom;
                }
                match w.kind {
                    WidgetKind::MenuButton => {
                        let items = if w.props.items.is_empty() {
                            vec!["Item".into()]
                        } else {
                            w.props.items.clone()
                        };
                        let mut sel = w.props.selected.min(items.len() - 1);
                        ui.push_id(("menu_button", w.id), |ui| {
                            ui.menu_button(&w.props.text, |ui| {
                                for (i, it) in items.iter().enumerate() {
                                    if ui.button(it).clicked() {
                                        sel = i;
                                        ui.close_kind(egui::UiKind::Menu);
                                    }
                                }
                            });
                        });
                        w.props.selected = sel;
                    }
                    WidgetKind::Label => {
                        let mut rt = egui::RichText::new(&w.props.text);
                        if let Some(tc) = w.props.text_color {
                            rt = rt.color(Color32::from_rgba_unmultiplied(tc[0], tc[1], tc[2], tc[3]));
                        }
                        if let Some(sz) = w.props.font_size {
                            rt = rt.size(sz * zoom);
                        }
                        if w.props.font_bold {
                            rt = rt.strong();
                        }
                        ui.vertical_centered(|ui| {
                            ui.label(rt);
                        });
                    }
                    WidgetKind::Button => {
                        let mut rt = egui::RichText::new(&w.props.text);
                        if let Some(tc) = w.props.text_color {
                            rt = rt.color(Color32::from_rgba_unmultiplied(tc[0], tc[1], tc[2], tc[3]));
                        }
                        if let Some(sz) = w.props.font_size {
                            rt = rt.size(sz * zoom);
                        }
                        if w.props.font_bold {
                            rt = rt.strong();
                        }
                        let mut btn = egui::Button::new(rt);
                        if let Some(bg) = w.props.bg_color {
                            btn = btn.fill(Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]));
                        }
                        if let Some(bc) = w.props.border_color {
                            let bw = w.props.border_width.unwrap_or(1.0) * zoom;
                            btn = btn.stroke(Stroke::new(bw, Color32::from_rgba_unmultiplied(bc[0], bc[1], bc[2], bc[3])));
                        }
                        if let Some(cr) = w.props.corner_radius {
                            btn = btn.corner_radius((cr * zoom).max(0.0) as u8);
                        }
                        let resp = ui.add_sized(scaled_size, btn);
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::ImageTextButton => {
                        let label = format!("{}  {}", w.props.icon, w.props.text);
                        let mut rt = egui::RichText::new(label);
                        if let Some(tc) = w.props.text_color {
                            rt = rt.color(Color32::from_rgba_unmultiplied(tc[0], tc[1], tc[2], tc[3]));
                        }
                        if let Some(sz) = w.props.font_size {
                            rt = rt.size(sz * zoom);
                        }
                        if w.props.font_bold {
                            rt = rt.strong();
                        }
                        let mut btn = egui::Button::new(rt);
                        if let Some(bg) = w.props.bg_color {
                            btn = btn.fill(Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]));
                        }
                        if let Some(bc) = w.props.border_color {
                            let bw = w.props.border_width.unwrap_or(1.0) * zoom;
                            btn = btn.stroke(Stroke::new(bw, Color32::from_rgba_unmultiplied(bc[0], bc[1], bc[2], bc[3])));
                        }
                        if let Some(cr) = w.props.corner_radius {
                            btn = btn.corner_radius((cr * zoom).max(0.0) as u8);
                        }
                        let resp = ui.add_sized(scaled_size, btn);
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::Checkbox => {
                        let mut checked = w.props.checked;
                        let resp = ui.add_sized(scaled_size, egui::Checkbox::new(&mut checked, &w.props.text));
                        w.props.checked = checked;
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::TextEdit => {
                        let mut buf = w.props.text.clone();
                        let resp = egui::TextEdit::singleline(&mut buf)
                            .id_salt(("text_edit", w.id))
                            .hint_text("text");
                        let r = ui.add_sized(scaled_size, resp);
                        w.props.text = buf;
                        if !is_edit_mode {
                            Self::check_widget_actions(&r, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::Slider => {
                        let mut v = w.props.value;
                        let slider = egui::Slider::new(&mut v, w.props.min..=w.props.max)
                            .text(&w.props.text);
                        let resp = ui.add_sized(scaled_size, slider);
                        w.props.value = v;
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::ProgressBar => {
                        let bar =
                            egui::ProgressBar::new(w.props.value.clamp(0.0, 1.0)).show_percentage();
                        ui.add_sized(scaled_size, bar);
                    }
                    WidgetKind::RadioGroup => {
                        let mut sel = w.props.selected.min(w.props.items.len().saturating_sub(1));
                        ui.vertical(|ui| {
                            for (i, it) in w.props.items.iter().enumerate() {
                                if ui.add(egui::RadioButton::new(sel == i, it)).clicked() {
                                    sel = i;
                                }
                            }
                        });
                        w.props.selected = sel;
                    }
                    WidgetKind::Link => {
                        let resp = ui.link(&w.props.text);
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::Hyperlink => {
                        let resp = ui.hyperlink_to(&w.props.text, &w.props.url);
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::SelectableLabel => {
                        let mut on = w.props.checked;
                        let resp = ui.add(egui::Button::selectable(on, &w.props.text));
                        if resp.clicked() {
                            on = !on;
                        }
                        w.props.checked = on;
                        if !is_edit_mode {
                            Self::check_widget_actions(&resp, &w.props.actions, triggered);
                        }
                    }
                    WidgetKind::ComboBox => {
                        let items = if w.props.items.is_empty() {
                            vec!["Item".into()]
                        } else {
                            w.props.items.clone()
                        };
                        let mut sel = w.props.selected.min(items.len() - 1);
                        egui::ComboBox::from_id_salt(w.id)
                            .width(scaled_size.x)
                            .selected_text(items[sel].clone())
                            .show_ui(ui, |ui| {
                                for (i, it) in items.iter().enumerate() {
                                    ui.selectable_value(&mut sel, i, it.clone());
                                }
                            });
                        w.props.selected = sel;
                    }
                    WidgetKind::Separator => {
                        ui.separator();
                    }
                    WidgetKind::CollapsingHeader => {
                        let is_open = w.props.checked;
                        let resp = egui::CollapsingHeader::new(&w.props.text)
                            .id_salt(("collapsing_header", w.id))
                            .open(Some(is_open))
                            .show(ui, |ui| {
                                if !has_children {
                                    ui.weak("(drop widgets here)");
                                }
                            });
                        if resp.header_response.clicked() {
                            w.props.checked = !is_open;
                        }
                    }
                    WidgetKind::DatePicker => {
                        let mut date = NaiveDate::from_ymd_opt(
                            w.props.year,
                            w.props.month.clamp(1, 12),
                            w.props.day.clamp(1, 28), // simple clamp
                        )
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
                        ui.horizontal(|ui| {
                            ui.label(&w.props.text);
                            let date_picker_id = format!("date_picker_{}", w.id);
                            ui.add(DatePickerButton::new(&mut date).id_salt(&date_picker_id));
                        });
                        w.props.year = date.year();
                        w.props.month = date.month();
                        w.props.day = date.day();
                    }
                    WidgetKind::AngleSelector => {
                        // Angle editor as slider in degrees
                        let mut v = w.props.value.clamp(w.props.min, w.props.max);
                        let slider = egui::Slider::new(&mut v, w.props.min..=w.props.max)
                            .suffix("°")
                            .text(&w.props.text);
                        ui.add_sized(scaled_size, slider);
                        w.props.value = v;
                    }
                    WidgetKind::Password => {
                        let mut buf = w.props.text.clone();
                        let resp = egui::TextEdit::singleline(&mut buf)
                            .id_salt(("password_edit", w.id))
                            .password(true)
                            .hint_text("password");
                        ui.add_sized(scaled_size, resp);
                        w.props.text = buf;
                    }
                    WidgetKind::Tree => {
                        // Parse items (two leading spaces per level) into nodes:
                        #[derive(Clone)]
                        struct Node {
                            label: String,
                            children: Vec<Node>,
                        }

                        fn parse_nodes(lines: &[String]) -> Vec<Node> {
                            // (indent, label)
                            let mut items: Vec<(usize, String)> = lines
                                .iter()
                                .map(|s| {
                                     let indent = s.chars().take_while(|c| *c == ' ').count() / 2;
                                     (indent, s.trim().to_string())
                                 })
                                 .collect();
                            // Remove empties
                            items.retain(|(_, s)| !s.is_empty());

                            fn build<I: Iterator<Item = (usize, String)>>(
                                iter: &mut std::iter::Peekable<I>,
                                level: usize,
                            ) -> Vec<Node> {
                                let mut out = Vec::new();
                                while let Some((ind, _)) = iter.peek().cloned() {
                                    if ind < level {
                                        break;
                                    }
                                    if ind > level {
                                        // child of previous; let outer loop handle
                                        break;
                                    }
                                    // ind == level
                                    let (_, label) = iter.next().unwrap();
                                    // gather children (ind + 1)
                                    let children = build(iter, level + 1);
                                    out.push(Node { label, children });
                                }
                                out
                            }

                            let mut it = items.into_iter().peekable();
                            build(&mut it, 0)
                        }

                        fn show_nodes(ui: &mut egui::Ui, nodes: &[Node], path: &mut Vec<usize>) {
                            for (idx, n) in nodes.iter().enumerate() {
                                if n.children.is_empty() {
                                    ui.label(&n.label);
                                } else {
                                    path.push(idx);
                                    egui::CollapsingHeader::new(&n.label)
                                        .id_salt(("tree_node", path.clone()))
                                        .show(ui, |ui| {
                                            show_nodes(ui, &n.children, path);
                                        });
                                    path.pop();
                                }
                            }
                        }

                        let lines = if w.props.items.is_empty() {
                            vec!["Root".into(), "  Child".into()]
                        } else {
                            w.props.items.clone()
                        };
                        let nodes = parse_nodes(&lines);

                        // Constrain content to the widget rect:
                        egui::Frame::NONE.show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt(("tree_scroll", w.id))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut path = Vec::new();
                                    show_nodes(ui, &nodes, &mut path);
                                });
                        });
                    }
                    WidgetKind::TextArea => {
                        let mut buf = w.props.text.clone();
                        let resp = egui::TextEdit::multiline(&mut buf)
                            .id_salt(("text_area", w.id))
                            .desired_width(scaled_size.x)
                            .desired_rows(5);
                        ui.add_sized(scaled_size, resp);
                        w.props.text = buf;
                    }
                    WidgetKind::DragValue => {
                        let mut v = w.props.value;
                        ui.horizontal(|ui| {
                            ui.label(&w.props.text);
                            ui.add(egui::DragValue::new(&mut v).range(w.props.min..=w.props.max));
                        });
                        w.props.value = v;
                    }
                    WidgetKind::Spinner => {
                        ui.add(egui::Spinner::new());
                    }
                    WidgetKind::ColorPicker => {
                        let mut color = Color32::from_rgba_unmultiplied(
                            w.props.color[0],
                            w.props.color[1],
                            w.props.color[2],
                            w.props.color[3],
                        );
                        ui.horizontal(|ui| {
                            ui.label(&w.props.text);
                            egui::color_picker::color_edit_button_srgba(
                                ui,
                                &mut color,
                                egui::color_picker::Alpha::OnlyBlend,
                            );
                        });
                        w.props.color = [color.r(), color.g(), color.b(), color.a()];
                    }
                    WidgetKind::Code => {
                        let mut buf = w.props.text.clone();
                        egui::ScrollArea::vertical()
                            .id_salt(("code_scroll", w.id))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut buf)
                                        .id_salt(("code_editor", w.id))
                                        .code_editor()
                                        .desired_width(scaled_size.x)
                                        .desired_rows(8),
                                );
                            });
                        w.props.text = buf;
                    }
                    WidgetKind::Heading => {
                        let mut rt = egui::RichText::new(&w.props.text);
                        if let Some(tc) = w.props.text_color {
                            rt = rt.color(Color32::from_rgba_unmultiplied(tc[0], tc[1], tc[2], tc[3]));
                        }
                        if let Some(sz) = w.props.font_size {
                            rt = rt.size(sz * zoom);
                        } else {
                            rt = rt.heading();
                        }
                        if w.props.font_bold {
                            rt = rt.strong();
                        }
                        ui.label(rt);
                    }
                    WidgetKind::Small => {
                        let mut rt = egui::RichText::new(&w.props.text);
                        if let Some(tc) = w.props.text_color {
                            rt = rt.color(Color32::from_rgba_unmultiplied(tc[0], tc[1], tc[2], tc[3]));
                        }
                        if let Some(sz) = w.props.font_size {
                            rt = rt.size(sz * zoom);
                        } else {
                            rt = rt.small();
                        }
                        if w.props.font_bold {
                            rt = rt.strong();
                        }
                        ui.label(rt);
                    }
                    WidgetKind::Monospace => {
                        ui.monospace(&w.props.text);
                    }
                    WidgetKind::Image => {
                        // Show placeholder with image info
                        let color = Color32::from_rgba_unmultiplied(80, 80, 80, 200);
                        egui::Frame::NONE
                            .fill(color)
                            .stroke(Stroke::new(1.0_f32, Color32::GRAY))
                            .show(ui, |ui| {
                                ui.set_min_size(scaled_size);
                                ui.centered_and_justified(|ui| {
                                    ui.label(format!(
                                        "🖼 {}\n{}x{}",
                                        w.props.text, w.size.x as i32, w.size.y as i32
                                    ));
                                });
                            });
                    }
                    WidgetKind::Placeholder => {
                        let color = Color32::from_rgba_unmultiplied(
                            w.props.color[0],
                            w.props.color[1],
                            w.props.color[2],
                            w.props.color[3],
                        );
                        egui::Frame::NONE
                            .fill(color)
                            .stroke(Stroke::new(1.0_f32, Color32::GRAY))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.set_min_size(scaled_size);
                                ui.centered_and_justified(|ui| {
                                    ui.label(&w.props.text);
                                });
                            });
                    }
                    WidgetKind::Group => {
                        let mut frame = egui::Frame::group(ui.style());
                        if let Some(bg) = w.props.bg_color {
                            frame = frame.fill(Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]));
                        }
                        if let Some(bc) = w.props.border_color {
                            let bw = w.props.border_width.unwrap_or(1.0) * zoom;
                            frame = frame.stroke(Stroke::new(bw, Color32::from_rgba_unmultiplied(bc[0], bc[1], bc[2], bc[3])));
                        }
                        if let Some(cr) = w.props.corner_radius {
                            frame = frame.corner_radius((cr * zoom).max(0.0) as u8);
                        }
                        frame.show(ui, |ui| {
                            ui.set_min_size((scaled_size - vec2(12.0 * zoom, 12.0 * zoom)).max(egui::Vec2::ZERO));
                            let add_contents = |ui: &mut egui::Ui| {
                                if !w.props.text.is_empty() {
                                    ui.strong(&w.props.text);
                                    ui.separator();
                                }
                                if !has_children {
                                    ui.weak("(drop widgets here)");
                                }
                            };
                            if w.props.horizontal {
                                ui.horizontal(add_contents);
                            } else {
                                ui.vertical(add_contents);
                            }
                        });
                    }
                    WidgetKind::Container => {
                        let mut frame = egui::Frame::NONE;
                        if let Some(bg) = w.props.bg_color {
                            frame = frame.fill(Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]));
                        }
                        if let Some(bc) = w.props.border_color {
                            let bw = w.props.border_width.unwrap_or(1.0) * zoom;
                            frame = frame.stroke(Stroke::new(bw, Color32::from_rgba_unmultiplied(bc[0], bc[1], bc[2], bc[3])));
                        }
                        if let Some(cr) = w.props.corner_radius {
                            frame = frame.corner_radius((cr * zoom).max(0.0) as u8);
                        }
                        frame.show(ui, |ui| {
                            ui.set_min_size(scaled_size);
                            if !has_children && is_edit_mode {
                                ui.weak("(container)");
                            }
                        });
                    }
                    WidgetKind::ScrollBox => {
                        egui::Frame::NONE
                            .stroke(Stroke::new(1.0_f32, Color32::GRAY))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                egui::ScrollArea::both()
                                    .id_salt(("scroll_box", w.id))
                                    .max_width((scaled_size.x - 4.0 * zoom).max(0.0))
                                    .max_height((scaled_size.y - 4.0 * zoom).max(0.0))
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        if !w.props.text.is_empty() {
                                            ui.label(&w.props.text);
                                        } else if !has_children {
                                            ui.weak("(drop widgets here)");
                                        }
                                    });
                            });
                    }
                    WidgetKind::TabBar => {
                        if w.props.show_tabs {
                            ui.horizontal(|ui| {
                                for (i, item) in w.props.items.iter().enumerate() {
                                    let selected = i == w.props.selected;
                                    let resp = ui.selectable_label(selected, item);
                                    if resp.clicked() {
                                        w.props.selected = i;
                                    }
                                    if !is_edit_mode {
                                        Self::check_widget_actions(&resp, &w.props.actions, triggered);
                                    }
                                }
                            });
                        } else {
                            // View Container / Screen Switcher: no visible tab header.
                            // In edit mode show a subtle picker so designer can switch active view.
                            if is_edit_mode {
                                let view_name = w.props.items.get(w.props.selected)
                                    .map(|s| s.as_str())
                                    .unwrap_or("View");
                                ui.horizontal(|ui| {
                                    ui.weak(format!("[View Container] Active: {}", view_name));
                                    for (i, item) in w.props.items.iter().enumerate() {
                                        if ui.selectable_label(i == w.props.selected, item).clicked() {
                                            w.props.selected = i;
                                        }
                                    }
                                });
                            }
                        }
                    }
                    WidgetKind::Columns => {
                        let cols = w.props.columns.max(1);
                        egui::Frame::NONE
                            .stroke(Stroke::new(1.0_f32, Color32::GRAY))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.columns(cols, |columns| {
                                    for (i, col) in columns.iter_mut().enumerate() {
                                        col.label(format!("Col {}", i + 1));
                                        if !w.props.text.is_empty() {
                                            col.label(&w.props.text);
                                        }
                                    }
                                });
                            });
                    }
                    WidgetKind::Window => {
                        egui::Frame::window(ui.style()).show(ui, |ui| {
                            ui.set_min_size((scaled_size - vec2(16.0 * zoom, 16.0 * zoom)).max(egui::Vec2::ZERO));
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.strong(&w.props.text);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("✕").clicked() && !is_edit_mode {
                                                w.props.checked = false;
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                                if !has_children {
                                    ui.weak("(window contents)");
                                }
                            });
                        });
                    }
                }

                // General fallback: if this is a non-standard widget with actions, handle clicks/hovers
                if !is_edit_mode && !w.props.actions.is_empty() && matches!(
                    w.kind,
                    WidgetKind::Label
                        | WidgetKind::Heading
                        | WidgetKind::Small
                        | WidgetKind::Monospace
                        | WidgetKind::Image
                        | WidgetKind::Placeholder
                        | WidgetKind::Group
                ) {
                    let resp = ui.interact(rect, ui.make_persistent_id(("action_interact", w.id)), Sense::click());
                    Self::check_widget_actions(&resp, &w.props.actions, triggered);
                }
            });
        });
        let painter = ui.painter();
        let is_selected = selected.contains(&w.id);
        let stroke = if is_selected {
            Stroke::new((2.0 * zoom).max(1.5), Color32::LIGHT_BLUE)
        } else if !w.props.initially_visible {
            Stroke::new((1.0 * zoom).max(1.0), Color32::from_rgba_unmultiplied(120, 160, 255, 160))
        } else {
            Stroke::new((1.0 * zoom).max(1.0), Color32::from_gray(90))
        };
        painter.rect_stroke(
            rect,
            CornerRadius::same(((6.0 * zoom).max(2.0).round()) as u8),
            stroke,
            egui::StrokeKind::Outside,
        );
        if !w.props.initially_visible && is_edit_mode {
            painter.text(
                rect.right_top() + vec2(-4.0 * zoom, 4.0 * zoom),
                egui::Align2::RIGHT_TOP,
                "hidden",
                egui::FontId::proportional((10.0 * zoom).max(8.0)),
                Color32::from_rgba_unmultiplied(120, 160, 255, 200),
            );
        }
        if is_edit_mode {
            let pad = (6.0 * zoom).max(4.0);
            let expanded = rect.expand(pad);
            let top = Rect::from_min_max(expanded.min, pos2(expanded.max.x, rect.min.y));
            let bottom = Rect::from_min_max(pos2(expanded.min.x, rect.max.y), expanded.max);
            let left = Rect::from_min_max(
                pos2(expanded.min.x, rect.min.y),
                pos2(rect.min.x, rect.max.y),
            );
            let right = Rect::from_min_max(
                pos2(rect.max.x, rect.min.y),
                pos2(expanded.max.x, rect.max.y),
            );

            let mut any_clicked = false;
            let mut drag_delta = egui::Vec2::ZERO;
            for (i, edge) in [top, right, bottom, left].into_iter().enumerate() {
                let id = ui.make_persistent_id(("edge", w.id, i as u8));
                let resp = ui.interact(edge, id, Sense::click_and_drag());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                if resp.clicked() {
                    any_clicked = true;
                }
                if resp.dragged() {
                    drag_delta += resp.drag_delta();
                }
            }
            if any_clicked {
                // Check if Shift is held for multi-select
                let shift_held = ui.ctx().input(|i| i.modifiers.shift);
                if shift_held {
                    // Toggle selection
                    if let Some(pos) = selected.iter().position(|&x| x == w.id) {
                        selected.remove(pos);
                    } else {
                        selected.push(w.id);
                    }
                } else {
                    // Single select
                    selected.clear();
                    selected.push(w.id);
                }
            }
            if drag_delta != egui::Vec2::ZERO && !is_in_auto_layout {
                let scaled_delta = drag_delta / zoom;
                w.pos += scaled_delta;
                w.pos = snap_pos_with_grid(w.pos, grid);
                let logical_canvas_w = canvas_rect.width() / zoom;
                let logical_canvas_h = canvas_rect.height() / zoom;
                let maxx = (logical_canvas_w - w.size.x).max(0.0);
                let maxy = (logical_canvas_h - w.size.y).max(0.0);
                w.pos.x = w.pos.x.clamp(0.0, maxx);
                w.pos.y = w.pos.y.clamp(0.0, maxy);
            }

            // resize handle scaled by zoom, plus clamp
            let hs = (12.0 * zoom).clamp(8.0, 24.0);
            let handle = Rect::from_min_size(expanded.max - vec2(hs, hs), vec2(hs, hs));
            let rid = ui.make_persistent_id(("resize", w.id));
            let rresp = ui.interact(handle, rid, Sense::click_and_drag());
            if rresp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
            }
            if rresp.dragged() {
                let delta = rresp.drag_delta() / zoom;
                w.size += delta;
                let logical_canvas_w = canvas_rect.width() / zoom;
                let logical_canvas_h = canvas_rect.height() / zoom;
                w.size.x = w.size.x.max(20.0).min(logical_canvas_w);
                w.size.y = w.size.y.max(16.0).min(logical_canvas_h);
            }
            ui.painter()
                .rect_filled(handle, 2.0, Color32::from_rgb(100, 160, 255));
        }
    }

    fn snap_pos(&self, p: Pos2) -> Pos2 {
        pos2(
            (p.x / self.grid_size).round() * self.grid_size,
            (p.y / self.grid_size).round() * self.grid_size,
        )
    }

    fn palette_ui(&mut self, ui: &mut egui::Ui) {
        ui.push_id("palette_ui", |ui| {
            ui.heading("Palette");
            ui.separator();
            ui.label("Drag any control onto the canvas");
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("palette_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::CollapsingHeader::new("Basic")
                        .id_salt("palette_basic")
                        .default_open(true)
                        .show(ui, |ui| {
                            self.palette_item(ui, "Label", WidgetKind::Label);
                            self.palette_item(ui, "Button", WidgetKind::Button);
                            self.palette_item(
                                ui,
                                "Image + Text Button",
                                WidgetKind::ImageTextButton,
                            );
                            self.palette_item(ui, "Checkbox", WidgetKind::Checkbox);
                            self.palette_item(ui, "Link", WidgetKind::Link);
                            self.palette_item(ui, "Hyperlink", WidgetKind::Hyperlink);
                            self.palette_item(ui, "Selectable Label", WidgetKind::SelectableLabel);
                            self.palette_item(ui, "Separator", WidgetKind::Separator);
                        });

                    egui::CollapsingHeader::new("Input")
                        .id_salt("palette_input")
                        .default_open(true)
                        .show(ui, |ui| {
                            self.palette_item(ui, "TextEdit", WidgetKind::TextEdit);
                            self.palette_item(ui, "Text Area", WidgetKind::TextArea);
                            self.palette_item(ui, "Password", WidgetKind::Password);
                            self.palette_item(ui, "Slider", WidgetKind::Slider);
                            self.palette_item(ui, "Drag Value", WidgetKind::DragValue);
                            self.palette_item(ui, "Combo Box", WidgetKind::ComboBox);
                            self.palette_item(ui, "Radio Group", WidgetKind::RadioGroup);
                            self.palette_item(ui, "Date Picker", WidgetKind::DatePicker);
                            self.palette_item(ui, "Angle Selector", WidgetKind::AngleSelector);
                            self.palette_item(ui, "Color Picker", WidgetKind::ColorPicker);
                        });

                    egui::CollapsingHeader::new("Display")
                        .id_salt("palette_display")
                        .default_open(true)
                        .show(ui, |ui| {
                            self.palette_item(ui, "Heading", WidgetKind::Heading);
                            self.palette_item(ui, "Small", WidgetKind::Small);
                            self.palette_item(ui, "Monospace", WidgetKind::Monospace);
                            self.palette_item(ui, "ProgressBar", WidgetKind::ProgressBar);
                            self.palette_item(ui, "Spinner", WidgetKind::Spinner);
                            self.palette_item(ui, "Image", WidgetKind::Image);
                            self.palette_item(ui, "Placeholder", WidgetKind::Placeholder);
                        });

                    egui::CollapsingHeader::new("Containers")
                        .id_salt("palette_containers")
                        .default_open(true)
                        .show(ui, |ui| {
                            self.palette_item(ui, "Container (Div)", WidgetKind::Container);
                            self.palette_item(ui, "Group", WidgetKind::Group);
                            self.palette_item(ui, "Scroll Box", WidgetKind::ScrollBox);
                            self.palette_item(ui, "Columns", WidgetKind::Columns);
                            self.palette_item(ui, "Tab Bar", WidgetKind::TabBar);
                            self.palette_item(ui, "Window", WidgetKind::Window);
                            self.palette_item(
                                ui,
                                "Collapsing Header",
                                WidgetKind::CollapsingHeader,
                            );
                        });

                    egui::CollapsingHeader::new("Advanced")
                        .id_salt("palette_advanced")
                        .default_open(false)
                        .show(ui, |ui| {
                            self.palette_item(ui, "Menu Button", WidgetKind::MenuButton);
                            self.palette_item(ui, "Tree", WidgetKind::Tree);
                            self.palette_item(ui, "Code Editor", WidgetKind::Code);
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    egui::CollapsingHeader::new("Shortcuts")
                        .id_salt("palette_shortcuts")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.small("Ctrl+Z: undo");
                            ui.small("Ctrl+Y: redo");
                            ui.small("Arrows: nudge widget");
                            ui.small("Delete: remove");
                            ui.small("Ctrl+C/V: copy/paste");
                            ui.small("Ctrl+D: duplicate");
                            ui.small("] / [: z-order");
                            ui.small("Ctrl+G: generate");
                            ui.small("F5: toggle preview");
                        });
                });
        });
    }

    fn palette_item(&mut self, ui: &mut egui::Ui, label: &str, kind: WidgetKind) {
        let r = ui.add(egui::Button::new(label).sense(Sense::drag()));
        if r.drag_started() || r.clicked() {
            self.spawning = Some(kind);
        }
    }

    fn layers_ui(&mut self, ui: &mut egui::Ui) {
        ui.push_id("layers_ui", |ui| {
            ui.heading("Layers");
            ui.separator();
            ui.label("Hierarchy, reordering, and visibility");
            ui.add_space(4.0);

            let mut reparent_action: Option<(WidgetId, Option<WidgetId>)> = None;
            let mut z_reorder_action: Option<(WidgetId, i32)> = None;
            let mut undo_needed = false;
            let mut ctx_action: Option<ContextMenuAction> = None;
            let clipboard_has_widget = self.clipboard.is_some();

            egui::ScrollArea::vertical()
                .id_salt("layers_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.project.widgets.is_empty() {
                        ui.label("No widgets yet.\nDrag from 🎨 Palette to add one.");
                        return;
                    }

                    // Top drop zone: drop here to unparent to Root
                    if let Some(dragged_id) = self.layer_drag {
                        let top_zone = ui.allocate_rect(
                            egui::Rect::from_min_size(
                                ui.cursor().min,
                                egui::vec2(ui.available_width(), 6.0),
                            ),
                            egui::Sense::hover(),
                        );
                        if top_zone.hovered() {
                            ui.painter().hline(
                                top_zone.rect.x_range(),
                                top_zone.rect.center().y,
                                egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_BLUE),
                            );
                            if ui.input(|i| i.pointer.any_released()) {
                                reparent_action = Some((dragged_id, None));
                                undo_needed = true;
                            }
                        }
                    }

                    // Render Screen (Root) header item
                    let is_screen_selected = self.selected.is_empty();
                    let mode_name = self.project.root_layout_mode.display_name();
                    let screen_title = format!("Screen (Root) [{}]", mode_name);
                    ui.horizontal(|ui| {
                        let btn = ui.selectable_label(is_screen_selected, screen_title);
                        if btn.clicked() {
                            self.selected.clear();
                        }
                    });
                    ui.add_space(2.0);

                    // Render tree recursively starting from root widgets (parent == None)
                    let root_ids: Vec<WidgetId> = {
                        let mut roots: Vec<(WidgetId, i32)> = self
                            .project
                            .widgets
                            .iter()
                            .filter(|w| w.parent.is_none())
                            .map(|w| (w.id, w.z))
                            .collect();
                        roots.sort_by_key(|&(_, z)| std::cmp::Reverse(z));
                        roots.into_iter().map(|(id, _)| id).collect()
                    };

                    for &wid in &root_ids {
                        self.render_layer_tree_node(
                            ui,
                            wid,
                            0,
                            &mut reparent_action,
                            &mut z_reorder_action,
                            &mut undo_needed,
                            &mut ctx_action,
                            clipboard_has_widget,
                        );
                    }

                    // Clear drag state if mouse released anywhere
                    if ui.input(|i| i.pointer.any_released()) {
                        self.layer_drag = None;
                        self.layer_drag_zone = None;
                    }
                });

            if undo_needed {
                self.push_undo();
            }
            if let Some((child_id, new_parent)) = reparent_action {
                self.reparent_widget(child_id, new_parent);
            }
            if let Some((src_id, new_z)) = z_reorder_action {
                if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == src_id) {
                    w.z = new_z;
                }
            }
            // Apply any deferred context-menu action last (avoids borrow conflicts)
            if let Some(action) = ctx_action {
                self.apply_context_menu_action(action);
            }
        });
    }

    fn render_layer_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        wid: WidgetId,
        depth: usize,
        reparent_action: &mut Option<(WidgetId, Option<WidgetId>)>,
        z_reorder_action: &mut Option<(WidgetId, i32)>,
        undo_needed: &mut bool,
        ctx_action: &mut Option<ContextMenuAction>,
        clipboard_has_widget: bool,
    ) {
        let Some(idx) = self.project.widgets.iter().position(|w| w.id == wid) else {
            return;
        };

        let is_selected = self.selected.contains(&wid);
        let is_container = self.project.widgets[idx].kind.is_container();
        let (label, active, init_vis, current_z) = {
            let w = &self.project.widgets[idx];
            let n = if w.props.name.is_empty() {
                w.kind.display_name().to_string()
            } else {
                w.props.name.clone()
            };
            (n, w.props.active, w.props.initially_visible, w.z)
        };

        let children_ids: Vec<WidgetId> = {
            let mut list: Vec<(WidgetId, i32)> = self
                .project
                .widgets
                .iter()
                .filter(|w| w.parent == Some(wid))
                .map(|w| (w.id, w.z))
                .collect();
            list.sort_by_key(|&(_, z)| std::cmp::Reverse(z));
            list.into_iter().map(|(id, _)| id).collect()
        };

        // ── Row ──────────────────────────────────────────────────────────────
        let row_resp = ui.horizontal(|ui| {
            // Indentation
            if depth > 0 {
                ui.add_space((depth as f32) * 16.0);
                ui.label("↳");
            }

            // Active toggle
            let active_btn = ui
                .small_button(if active { "✅" } else { "🚫" })
                .on_hover_text(if active {
                    "Active (in canvas & codegen)\nClick to deactivate"
                } else {
                    "Inactive (excluded from canvas & codegen)\nClick to activate"
                });
            if active_btn.clicked() {
                *undo_needed = true;
                self.project.widgets[idx].props.active = !active;
            }

            // Starts-visible toggle
            let vis_btn = ui
                .add_enabled(
                    active,
                    egui::Button::new(if init_vis { "👁" } else { "🙈" }).small(),
                )
                .on_hover_text(if init_vis {
                    "Starts visible\nClick to start hidden"
                } else {
                    "Starts hidden\nClick to start visible"
                });
            if vis_btn.clicked() {
                *undo_needed = true;
                self.project.widgets[idx].props.initially_visible = !init_vis;
            }

            let container_icon = if is_container { "📁 " } else { "📄 " };
            let label_text = if active {
                if init_vis {
                    label.clone()
                } else {
                    format!("({label})")
                }
            } else {
                format!("~~{label}~~")
            };

            // Drag handle + selection label
            let row = ui.selectable_label(
                is_selected,
                format!("⣿ {}{}", container_icon, label_text),
            );

            if row.drag_started() {
                self.layer_drag = Some(wid);
            }
            if row.clicked() {
                let shift = ui.input(|i| i.modifiers.shift);
                if shift {
                    if is_selected {
                        self.selected.retain(|&x| x != wid);
                    } else {
                        self.selected.push(wid);
                    }
                } else {
                    self.selected = vec![wid];
                }
            }

            // ── ⬆/⬇ buttons (right-aligned) ─────────────────────────────────
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("⬇").on_hover_text("Bajar una posición").clicked() {
                    *ctx_action = Some(ContextMenuAction::MoveDown(wid));
                }
                if ui.small_button("⬆").on_hover_text("Subir una posición").clicked() {
                    *ctx_action = Some(ContextMenuAction::MoveUp(wid));
                }
            });
        });

        // ── Context menu (right-click on the row) ────────────────────────────
        row_resp.response.context_menu(|ui| {
            ui.set_min_width(190.0);

            if ui.button("⧉  Duplicar").clicked() {
                *ctx_action = Some(ContextMenuAction::Duplicate(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            if ui.button("📋  Copiar").clicked() {
                *ctx_action = Some(ContextMenuAction::Copy(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            ui.add_enabled_ui(clipboard_has_widget, |ui| {
                if ui.button("📌  Pegar").clicked() {
                    *ctx_action = Some(ContextMenuAction::Paste);
                    ui.close_kind(egui::UiKind::Menu);
                }
            });
            ui.separator();
            if ui.button("🗑  Eliminar").clicked() {
                *ctx_action = Some(ContextMenuAction::Delete(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            ui.separator();
            let vis_label = if init_vis { "🙈  Ocultar al inicio" } else { "👁  Mostrar al inicio" };
            if ui.button(vis_label).clicked() {
                *ctx_action = Some(ContextMenuAction::ToggleVisible(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            let act_label = if active { "🚫  Desactivar" } else { "✅  Activar" };
            if ui.button(act_label).clicked() {
                *ctx_action = Some(ContextMenuAction::ToggleActive(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            ui.separator();
            if ui.button("⬆  Subir").clicked() {
                *ctx_action = Some(ContextMenuAction::MoveUp(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            if ui.button("⬇  Bajar").clicked() {
                *ctx_action = Some(ContextMenuAction::MoveDown(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            if ui.button("⏫  Traer al frente").clicked() {
                *ctx_action = Some(ContextMenuAction::BringToFront(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
            if ui.button("⏬  Enviar al fondo").clicked() {
                *ctx_action = Some(ContextMenuAction::SendToBack(wid));
                ui.close_kind(egui::UiKind::Menu);
            }
        });

        // ── Improved D&D with 3-zone insertion indicator ─────────────────────
        if let Some(dragged_id) = self.layer_drag {
            if dragged_id != wid && !self.project.is_descendant_of(wid, dragged_id) {
                let rect = row_resp.response.rect;
                let top_thresh = rect.min.y + rect.height() * 0.28;
                let bot_thresh = rect.max.y - rect.height() * 0.28;
                let pointer_y = ui.input(|i| i.pointer.hover_pos()).map(|p| p.y);

                if row_resp.response.hovered() {
                    if let Some(py) = pointer_y {
                        let zone = if py < top_thresh {
                            DragZone::Before
                        } else if py > bot_thresh && is_container {
                            // Show AFTER only when container; otherwise treat bottom as Before too
                            DragZone::After
                        } else if is_container {
                            DragZone::Inside
                        } else if py < rect.center().y {
                            DragZone::Before
                        } else {
                            DragZone::After
                        };
                        self.layer_drag_zone = Some(zone);

                        let painter = ui.painter();
                        match zone {
                            DragZone::Before => {
                                painter.hline(
                                    rect.x_range(),
                                    rect.min.y,
                                    egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_BLUE),
                                );
                            }
                            DragZone::After => {
                                painter.hline(
                                    rect.x_range(),
                                    rect.max.y,
                                    egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_BLUE),
                                );
                            }
                            DragZone::Inside => {
                                painter.rect_stroke(
                                    rect,
                                    egui::CornerRadius::same(2),
                                    egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_BLUE),
                                    egui::StrokeKind::Outside,
                                );
                            }
                        }

                        if ui.input(|i| i.pointer.any_released()) {
                            match zone {
                                DragZone::Before => {
                                    // Same parent as target, z = target.z + 1
                                    let target_parent = self.project.widgets[idx].parent;
                                    *reparent_action = Some((dragged_id, target_parent));
                                    *z_reorder_action = Some((dragged_id, current_z + 1));
                                    *undo_needed = true;
                                }
                                DragZone::Inside => {
                                    // Nest inside container
                                    *reparent_action = Some((dragged_id, Some(wid)));
                                    *undo_needed = true;
                                }
                                DragZone::After => {
                                    // Same parent as target, z = target.z - 1
                                    let target_parent = self.project.widgets[idx].parent;
                                    *reparent_action = Some((dragged_id, target_parent));
                                    *z_reorder_action = Some((dragged_id, current_z - 1));
                                    *undo_needed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Render children indented ──────────────────────────────────────────
        for child_id in children_ids {
            self.render_layer_tree_node(
                ui,
                child_id,
                depth + 1,
                reparent_action,
                z_reorder_action,
                undo_needed,
                ctx_action,
                clipboard_has_widget,
            );
        }
    }

    fn root_screen_inspector_ui(&mut self, ui: &mut egui::Ui) {
        ui.push_id("screen_root_inspector", |ui| {
            ui.heading("Screen (Root Container)");
            ui.separator();

            ui.label(egui::RichText::new("Device Viewport & Canvas").strong());

            // Device preset selector
            ui.horizontal(|ui| {
                ui.label("Preset:");
                egui::ComboBox::from_id_salt("screen_preset_combo")
                    .selected_text(self.project.screen_preset.display_name())
                    .show_ui(ui, |ui| {
                        for preset in ScreenPreset::all() {
                            let selected = self.project.screen_preset == *preset;
                            if ui.selectable_label(selected, preset.display_name()).clicked() {
                                self.push_undo();
                                self.project.screen_preset = *preset;
                                if let Some(dim) = preset.dimensions() {
                                    self.project.canvas_size = dim;
                                }
                            }
                        }
                    });
            });

            // Dimensions + Rotate
            ui.horizontal(|ui| {
                ui.label("Size:");
                let w_resp = ui.add(egui::DragValue::new(&mut self.project.canvas_size.x).range(200.0..=4096.0).suffix(" w"));
                let h_resp = ui.add(egui::DragValue::new(&mut self.project.canvas_size.y).range(200.0..=4096.0).suffix(" h"));
                if w_resp.changed() || h_resp.changed() {
                    self.project.screen_preset = ScreenPreset::Custom;
                }
                if ui.button("Rotate").on_hover_text("Swap width and height").clicked() {
                    self.push_undo();
                    let temp = self.project.canvas_size.x;
                    self.project.canvas_size.x = self.project.canvas_size.y;
                    self.project.canvas_size.y = temp;
                    self.project.screen_preset = ScreenPreset::Custom;
                }
            });

            ui.separator();
            ui.label(egui::RichText::new("Canvas Zoom & Viewport").strong());
            ui.horizontal(|ui| {
                ui.label("Zoom:");
                let mut pct = (self.canvas_zoom * 100.0).round() as i32;
                if ui.add(egui::DragValue::new(&mut pct).range(10..=400).suffix("%")).changed() {
                    self.canvas_auto_fit = false;
                    self.canvas_zoom = pct as f32 / 100.0;
                }
                if ui.button("100%").clicked() {
                    self.canvas_auto_fit = false;
                    self.canvas_zoom = 1.0;
                }
                let fit_btn = ui.selectable_label(self.canvas_auto_fit, "Fit to Screen");
                if fit_btn.clicked() {
                    self.canvas_auto_fit = !self.canvas_auto_fit;
                }
            });

            ui.separator();
            ui.label(egui::RichText::new("Screen Auto-Layout").strong());

            // Root Layout Mode ComboBox
            ui.horizontal(|ui| {
                ui.label("Layout Mode:");
                let prev_mode = self.project.root_layout_mode;
                egui::ComboBox::from_id_salt("root_layout_mode_combo")
                    .selected_text(self.project.root_layout_mode.display_name())
                    .show_ui(ui, |ui| {
                        for m in &[
                            LayoutMode::Free,
                            LayoutMode::Column,
                            LayoutMode::Row,
                            LayoutMode::WrapRow,
                            LayoutMode::Grid,
                        ] {
                            ui.selectable_value(&mut self.project.root_layout_mode, *m, m.display_name());
                        }
                    });
                if self.project.root_layout_mode != prev_mode {
                    self.push_undo();
                }
            });

            if self.project.root_layout_mode != LayoutMode::Free {
                // Gap
                ui.horizontal(|ui| {
                    ui.label("Item Gap:");
                    ui.add(egui::DragValue::new(&mut self.project.root_layout_gap).range(0.0..=100.0).suffix(" px"));
                });

                // Columns (if Grid)
                if self.project.root_layout_mode == LayoutMode::Grid {
                    ui.horizontal(|ui| {
                        ui.label("Columns:");
                        ui.add(egui::DragValue::new(&mut self.project.root_layout_cols).range(1..=12));
                    });
                }

                // Align & Justify
                if matches!(self.project.root_layout_mode, LayoutMode::Row | LayoutMode::Column) {
                    ui.horizontal(|ui| {
                        ui.label("Align (Cross):");
                        egui::ComboBox::from_id_salt("root_align_combo")
                            .selected_text(self.project.root_layout_align.display_name())
                            .show_ui(ui, |ui| {
                                for a in &[Align::Start, Align::Center, Align::End, Align::Stretch] {
                                    ui.selectable_value(&mut self.project.root_layout_align, *a, a.display_name());
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Justify (Main):");
                        egui::ComboBox::from_id_salt("root_justify_combo")
                            .selected_text(self.project.root_layout_justify.display_name())
                            .show_ui(ui, |ui| {
                                for j in &[
                                    Justify::Start,
                                    Justify::Center,
                                    Justify::End,
                                    Justify::SpaceBetween,
                                    Justify::SpaceAround,
                                    Justify::SpaceEvenly,
                                ] {
                                    ui.selectable_value(&mut self.project.root_layout_justify, *j, j.display_name());
                                }
                            });
                    });
                }

                // Padding: Top, Right, Bottom, Left
                ui.label(egui::RichText::new("Padding").strong());
                ui.horizontal(|ui| {
                    ui.label("T:");
                    ui.add(egui::DragValue::new(&mut self.project.root_layout_padding[0]).range(0.0..=100.0));
                    ui.label("R:");
                    ui.add(egui::DragValue::new(&mut self.project.root_layout_padding[1]).range(0.0..=100.0));
                    ui.label("B:");
                    ui.add(egui::DragValue::new(&mut self.project.root_layout_padding[2]).range(0.0..=100.0));
                    ui.label("L:");
                    ui.add(egui::DragValue::new(&mut self.project.root_layout_padding[3]).range(0.0..=100.0));
                });

                // Responsive Layout Rule
                ui.separator();
                ui.label(egui::RichText::new("Responsive Rule (Media Query)").strong());
                ui.horizontal(|ui| {
                    ui.label("Rule:");
                    egui::ComboBox::from_id_salt("root_responsive_layout_combo")
                        .selected_text(self.project.root_responsive_layout.display_name())
                        .show_ui(ui, |ui| {
                            for r in &[
                                crate::widget::ResponsiveLayout::None,
                                crate::widget::ResponsiveLayout::RowToColumnOnPortrait,
                            ] {
                                ui.selectable_value(&mut self.project.root_responsive_layout, *r, r.display_name());
                            }
                        });
                });
                if self.project.root_responsive_layout == crate::widget::ResponsiveLayout::RowToColumnOnPortrait {
                    ui.label(
                        egui::RichText::new("Automatically switches Row to Column when viewport height > width (portrait / mobile).")
                            .italics()
                            .weak(),
                    );
                }
            }
        });
    }

    fn inspector_ui(&mut self, ui: &mut egui::Ui) {
        let grid = self.grid_size; // read before mutably borrowing self
        let Some(&sel_id) = self.selected.first() else {
            self.root_screen_inspector_ui(ui);
            return;
        };

        // Query parent information before borrowing self mutably
        let current_parent = self
            .project
            .widgets
            .iter()
            .find(|x| x.id == sel_id)
            .and_then(|x| x.parent);

        let potential_parents: Vec<(WidgetId, String)> = self
            .project
            .widgets
            .iter()
            .filter(|pw| {
                pw.id != sel_id
                    && pw.kind.is_container()
                    && !self.project.is_descendant_of(pw.id, sel_id)
            })
            .map(|pw| {
                let name = if pw.props.name.is_empty() {
                    pw.kind.display_name().to_string()
                } else {
                    pw.props.name.clone()
                };
                (pw.id, format!("{} (#{})", name, pw.id))
            })
            .collect();

        let parent_label = match current_parent {
            None => "Screen (Root)".to_string(),
            Some(pid) => {
                if let Some(pw) = self.project.widgets.iter().find(|x| x.id == pid) {
                    let name = if pw.props.name.is_empty() {
                        pw.kind.display_name().to_string()
                    } else {
                        pw.props.name.clone()
                    };
                    format!("{} (#{})", name, pid)
                } else {
                    format!("#{}", pid)
                }
            }
        };

        let parent_layout_mode = match current_parent {
            Some(pid) => self
                .project
                .widgets
                .iter()
                .find(|x| x.id == pid)
                .map(|pw| pw.props.layout_mode)
                .unwrap_or(LayoutMode::Free),
            None => self.project.root_layout_mode,
        };

        let parent_tab_items: Vec<String> = current_parent
            .and_then(|pid| self.project.widgets.iter().find(|x| x.id == pid))
            .filter(|pw| pw.kind == WidgetKind::TabBar)
            .map(|pw| pw.props.items.clone())
            .unwrap_or_default();

        let all_target_widgets: Vec<(WidgetId, String)> = self
            .project
            .widgets
            .iter()
            .map(|pw| {
                let name = if pw.props.name.is_empty() {
                    pw.kind.display_name().to_string()
                } else {
                    pw.props.name.clone()
                };
                (pw.id, format!("{} (#{})", name, pw.id))
            })
            .collect();

        let target_widget_names: HashMap<WidgetId, String> =
            all_target_widgets.iter().cloned().collect();

        let mut action_draft = std::mem::take(&mut self.action_draft);
        let mut action_to_add: Option<WidgetAction> = None;
        let mut test_action: Option<ActionEffect> = None;
        let mut undo_needed = false;

        let mut new_parent = current_parent;
        let mut do_delete = false;

        ui.push_id("inspector_ui", |ui| {
            ui.heading("Inspector");
            ui.separator();
            if let Some(w) = self.selected_mut() {
                ui.label(format!("ID: {:?}", w.id));
                ui.add_space(4.0);

                // Friendly Name
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut w.props.name)
                            .hint_text(w.kind.display_name())
                            .id_salt(("inspector_name", w.id)),
                    );
                });

                ui.add_space(4.0);
                ui.label(egui::RichText::new("Display").strong());

                ui.checkbox(&mut w.props.active, "Active")
                    .on_hover_text("If unchecked, excluded from canvas and codegen completely.");

                ui.add_enabled_ui(w.props.active, |ui| {
                    ui.checkbox(&mut w.props.initially_visible, "Starts visible")
                        .on_hover_text("If unchecked, starts hidden in runtime/codegen; shown with dashed border in canvas.");

                    ui.horizontal(|ui| {
                        ui.label("Opacity:");
                        ui.add(
                            egui::Slider::new(&mut w.props.opacity, 0.0..=1.0)
                                .step_by(0.05)
                                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                        );
                    });
                });

                ui.separator();
                ui.add_space(4.0);

                match w.kind {
                    WidgetKind::Label
                    | WidgetKind::Heading
                    | WidgetKind::Small
                    | WidgetKind::Monospace
                    | WidgetKind::Button
                    | WidgetKind::ImageTextButton
                    | WidgetKind::TextEdit
                    | WidgetKind::Checkbox
                    | WidgetKind::Slider
                    | WidgetKind::Link
                    | WidgetKind::Hyperlink
                    | WidgetKind::SelectableLabel
                    | WidgetKind::CollapsingHeader
                    | WidgetKind::Password
                    | WidgetKind::AngleSelector
                    | WidgetKind::DatePicker
                    | WidgetKind::DragValue
                    | WidgetKind::ColorPicker
                    | WidgetKind::Placeholder
                    | WidgetKind::Group
                    | WidgetKind::Window
                    | WidgetKind::Columns => {
                        ui.label("Text");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.props.text)
                                .id_salt(("inspector_text", w.id)),
                        );
                    }
                    WidgetKind::ProgressBar
                    | WidgetKind::RadioGroup
                    | WidgetKind::ComboBox
                    | WidgetKind::Tree
                    | WidgetKind::Separator
                    | WidgetKind::Spinner
                    | WidgetKind::TabBar
                    | WidgetKind::Container => {}
                    WidgetKind::MenuButton => {
                        ui.label("Text");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.props.text)
                                .id_salt(("inspector_menu_text", w.id)),
                        );
                    }
                    WidgetKind::TextArea | WidgetKind::Code | WidgetKind::ScrollBox => {
                        ui.label("Content");
                        ui.add(
                            egui::TextEdit::multiline(&mut w.props.text)
                                .id_salt(("inspector_content", w.id))
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        );
                    }
                    WidgetKind::Image => {
                        ui.label("Filename");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.props.text)
                                .id_salt(("inspector_image_filename", w.id)),
                        );
                        ui.label("URI");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.props.url)
                                .id_salt(("inspector_image_uri", w.id)),
                        );
                    }
                }
                match w.kind {
                    WidgetKind::ImageTextButton => {
                        ui.label("Icon / Emoji");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.props.icon)
                                .id_salt(("inspector_icon", w.id)),
                        );
                    }
                    WidgetKind::Checkbox => {
                        ui.checkbox(&mut w.props.checked, "checked");
                    }
                    WidgetKind::Slider => {
                        ui.add(
                            egui::Slider::new(&mut w.props.value, w.props.min..=w.props.max)
                                .text("value"),
                        );
                        ui.add(
                            egui::Slider::new(&mut w.props.min, -1000.0..=w.props.max).text("min"),
                        );
                        ui.add(
                            egui::Slider::new(&mut w.props.max, w.props.min..=1000.0).text("max"),
                        );
                    }
                    WidgetKind::ProgressBar => {
                        ui.add(egui::Slider::new(&mut w.props.value, 0.0..=1.0).text("progress"));
                    }
                    WidgetKind::Hyperlink => {
                        ui.label("URL");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.props.url)
                                .id_salt(("inspector_hyperlink_url", w.id)),
                        );
                    }
                    WidgetKind::RadioGroup
                    | WidgetKind::ComboBox
                    | WidgetKind::Tree
                    | WidgetKind::MenuButton
                    | WidgetKind::TabBar => {
                        // TabBar-specific: show_tabs toggle
                        if w.kind == WidgetKind::TabBar {
                            ui.separator();
                            ui.checkbox(&mut w.props.show_tabs, "Show tab header buttons");
                            if !w.props.show_tabs {
                                ui.weak("Acting as View Container (tab buttons hidden; use SwitchTab actions to navigate)");
                            }
                            ui.label("Views / Tabs (one per line)");
                        } else {
                            ui.label(match w.kind {
                                WidgetKind::Tree => "Nodes (indent with spaces; 2 spaces per level)",
                                _ => "Items (one per line)",
                            });
                        }
                        let mut buf = w.props.items.join("\n");
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut buf)
                                    .id_salt(("inspector_items", w.id))
                                    .desired_rows(8)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed()
                        {
                            w.props.items = buf.lines().map(|s| s.to_string()).collect();
                            if w.props.selected >= w.props.items.len() {
                                w.props.selected = w.props.items.len().saturating_sub(1);
                            }
                        }
                        if !matches!(w.kind, WidgetKind::Tree) && !w.props.items.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label(if w.kind == WidgetKind::TabBar && !w.props.show_tabs {
                                    "Active view (edit mode)"
                                } else {
                                    "Selected index"
                                });
                                ui.add(
                                    egui::DragValue::new(&mut w.props.selected)
                                        .range(0..=w.props.items.len().saturating_sub(1)),
                                );
                                if w.kind == WidgetKind::TabBar && !w.props.items.is_empty() {
                                    let label = w.props.items.get(w.props.selected)
                                        .map(|s| s.as_str()).unwrap_or("");
                                    ui.weak(format!("({})", label));
                                }
                            });
                        }
                    }
                    WidgetKind::CollapsingHeader => {
                        ui.checkbox(&mut w.props.checked, "open by default");
                    }
                    WidgetKind::DatePicker => {
                        ui.horizontal(|ui| {
                            ui.label("Year");
                            ui.add(egui::DragValue::new(&mut w.props.year));
                            ui.label("Month");
                            ui.add(egui::DragValue::new(&mut w.props.month).range(1..=12));
                            ui.label("Day");
                            ui.add(egui::DragValue::new(&mut w.props.day).range(1..=31));
                        });
                    }
                    WidgetKind::AngleSelector => {
                        ui.add(
                            egui::Slider::new(&mut w.props.value, w.props.min..=w.props.max)
                                .text("value (deg)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut w.props.min, -1080.0..=w.props.max)
                                .text("min (deg)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut w.props.max, w.props.min..=1080.0)
                                .text("max (deg)"),
                        );
                    }
                    WidgetKind::Password => { /* no extra props */ }
                    WidgetKind::DragValue => {
                        ui.add(
                            egui::Slider::new(&mut w.props.value, w.props.min..=w.props.max)
                                .text("value"),
                        );
                        ui.add(
                            egui::Slider::new(&mut w.props.min, -1000.0..=w.props.max).text("min"),
                        );
                        ui.add(
                            egui::Slider::new(&mut w.props.max, w.props.min..=1000.0).text("max"),
                        );
                    }
                    WidgetKind::ColorPicker | WidgetKind::Placeholder => {
                        let mut color = Color32::from_rgba_unmultiplied(
                            w.props.color[0],
                            w.props.color[1],
                            w.props.color[2],
                            w.props.color[3],
                        );
                        ui.horizontal(|ui| {
                            ui.label("Color");
                            egui::color_picker::color_edit_button_srgba(
                                ui,
                                &mut color,
                                egui::color_picker::Alpha::OnlyBlend,
                            );
                        });
                        w.props.color = [color.r(), color.g(), color.b(), color.a()];
                    }
                    WidgetKind::Group => {
                        ui.checkbox(&mut w.props.horizontal, "horizontal layout");
                    }
                    WidgetKind::Columns => {
                        ui.horizontal(|ui| {
                            ui.label("Columns");
                            ui.add(egui::DragValue::new(&mut w.props.columns).range(1..=10));
                        });
                    }
                    _ => {}
                }
                ui.separator();

                // Parent container selection
                ui.horizontal(|ui| {
                    ui.label("Parent:");
                    egui::ComboBox::from_id_salt(("parent_select", w.id))
                        .selected_text(&parent_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut new_parent, None, "Screen (Root)");
                            for (pid, label) in &potential_parents {
                                ui.selectable_value(&mut new_parent, Some(*pid), label);
                            }
                        });
                });

                // Assigned Tab Page (when parent is a TabBar)
                if !parent_tab_items.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("Assigned Tab:");
                        let current_tab_label = match w.props.tab_page {
                            None => "All Tabs".to_string(),
                            Some(idx) => parent_tab_items
                                .get(idx)
                                .map(|s| format!("Tab {}: {}", idx + 1, s))
                                .unwrap_or_else(|| format!("Tab {}", idx + 1)),
                        };
                        egui::ComboBox::from_id_salt(("tab_page_select", w.id))
                            .selected_text(current_tab_label)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut w.props.tab_page, None, "All Tabs (visible on any tab)");
                                for (i, it) in parent_tab_items.iter().enumerate() {
                                    ui.selectable_value(&mut w.props.tab_page, Some(i), format!("Tab {}: {}", i + 1, it));
                                }
                            });
                    });
                }

                ui.horizontal(|ui| {
                    ui.label("Area");
                    let mut area = w.area;
                    egui::ComboBox::from_id_salt(("area", w.id))
                        .selected_text(format!("{:?}", area))
                        .show_ui(ui, |ui| {
                            for a in [
                                DockArea::Free,
                                DockArea::Top,
                                DockArea::Bottom,
                                DockArea::Left,
                                DockArea::Right,
                                DockArea::Center,
                            ] {
                                ui.selectable_value(&mut area, a, format!("{:?}", a));
                            }
                        });
                    if area != w.area {
                        w.area = area;
                        // reset pos within new area (keeps roughly same coords snapped)
                        w.pos = snap_pos_with_grid(w.pos, grid);
                    }
                });

                // ── Container Layout Settings ──────────────────────────────────
                if w.kind.is_container() {
                    ui.separator();
                    ui.label(egui::RichText::new("📐 Container Layout").strong());
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        egui::ComboBox::from_id_salt(("container_layout_mode", w.id))
                            .selected_text(w.props.layout_mode.display_name())
                            .show_ui(ui, |ui| {
                                for m in LayoutMode::all() {
                                    ui.selectable_value(&mut w.props.layout_mode, *m, m.display_name());
                                }
                            });
                    });

                    ui.checkbox(&mut w.props.auto_size_y, "Auto height (hug content)")
                        .on_hover_text("Automatically adjusts container height to encompass all child widgets");

                    ui.horizontal(|ui| {
                        ui.label("Adaptive layout:");
                        egui::ComboBox::from_id_salt(("container_responsive_layout", w.id))
                            .selected_text(w.props.responsive_layout.display_name())
                            .show_ui(ui, |ui| {
                                for rl in ResponsiveLayout::all() {
                                    ui.selectable_value(&mut w.props.responsive_layout, *rl, rl.display_name());
                                }
                            });
                    });

                    if w.props.layout_mode != LayoutMode::Free {
                        ui.horizontal(|ui| {
                            ui.label("Gap:");
                            ui.add(egui::DragValue::new(&mut w.props.layout_gap).range(0.0..=100.0).suffix(" px"));
                        });

                        if w.props.layout_mode == LayoutMode::Grid {
                            ui.horizontal(|ui| {
                                ui.label("Columns:");
                                ui.add(egui::DragValue::new(&mut w.props.layout_cols).range(1..=12));
                            });
                        }

                        if matches!(w.props.layout_mode, LayoutMode::Row | LayoutMode::Column) {
                            ui.horizontal(|ui| {
                                ui.label("Align items:");
                                egui::ComboBox::from_id_salt(("container_layout_align", w.id))
                                    .selected_text(w.props.layout_align.display_name())
                                    .show_ui(ui, |ui| {
                                        for a in Align::all() {
                                            ui.selectable_value(&mut w.props.layout_align, *a, a.display_name());
                                        }
                                    });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Justify content:");
                                egui::ComboBox::from_id_salt(("container_layout_justify", w.id))
                                    .selected_text(w.props.layout_justify.display_name())
                                    .show_ui(ui, |ui| {
                                        for j in Justify::all() {
                                            ui.selectable_value(&mut w.props.layout_justify, *j, j.display_name());
                                        }
                                    });
                            });
                        }

                        ui.collapsing("Padding (T, R, B, L)", |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Top:");
                                ui.add(egui::DragValue::new(&mut w.props.layout_padding[0]).range(0.0..=100.0));
                                ui.label("Right:");
                                ui.add(egui::DragValue::new(&mut w.props.layout_padding[1]).range(0.0..=100.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Bottom:");
                                ui.add(egui::DragValue::new(&mut w.props.layout_padding[2]).range(0.0..=100.0));
                                ui.label("Left:");
                                ui.add(egui::DragValue::new(&mut w.props.layout_padding[3]).range(0.0..=100.0));
                            });
                        });
                    }
                }

                // ── Position / Size ──────────────────────────────────────────
                ui.separator();
                let parent_has_auto_layout = parent_layout_mode != LayoutMode::Free;

                if parent_has_auto_layout {
                    let parent_desc = if current_parent.is_some() { "parent container" } else { "Screen (Root)" };
                    ui.label(egui::RichText::new(format!("Position: Auto-managed by {}", parent_desc)).italics().weak());
                    ui.label(egui::RichText::new("Size Policy").strong());

                    // Width Policy
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        let mut w_kind = match w.props.width_policy {
                            SizePolicy::Fixed => 0,
                            SizePolicy::Percent(_) => 1,
                            SizePolicy::Fill => 2,
                        };
                        egui::ComboBox::from_id_salt(("width_policy_combo", w.id))
                            .selected_text(match w_kind {
                                0 => "Fixed",
                                1 => "Percent",
                                _ => "Fill",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut w_kind, 0, "Fixed (px)");
                                ui.selectable_value(&mut w_kind, 1, "Percent (%)");
                                ui.selectable_value(&mut w_kind, 2, "Fill");
                            });

                        match w_kind {
                            0 => {
                                if !matches!(w.props.width_policy, SizePolicy::Fixed) {
                                    w.props.width_policy = SizePolicy::Fixed;
                                }
                                ui.add(egui::DragValue::new(&mut w.size.x).range(8.0..=2000.0).suffix(" px"));
                            }
                            1 => {
                                let mut pct = match w.props.width_policy {
                                    SizePolicy::Percent(p) => p,
                                    _ => 100.0,
                                };
                                if ui.add(egui::DragValue::new(&mut pct).range(1.0..=100.0).suffix(" %")).changed() || !matches!(w.props.width_policy, SizePolicy::Percent(_)) {
                                    w.props.width_policy = SizePolicy::Percent(pct);
                                }
                            }
                            2 => {
                                if !matches!(w.props.width_policy, SizePolicy::Fill) {
                                    w.props.width_policy = SizePolicy::Fill;
                                }
                                ui.label("(fills remaining space)");
                            }
                            _ => {}
                        }
                    });

                    // Height Policy
                    ui.horizontal(|ui| {
                        ui.label("Height:");
                        let mut h_kind = match w.props.height_policy {
                            SizePolicy::Fixed => 0,
                            SizePolicy::Percent(_) => 1,
                            SizePolicy::Fill => 2,
                        };
                        egui::ComboBox::from_id_salt(("height_policy_combo", w.id))
                            .selected_text(match h_kind {
                                0 => "Fixed",
                                1 => "Percent",
                                _ => "Fill",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut h_kind, 0, "Fixed (px)");
                                ui.selectable_value(&mut h_kind, 1, "Percent (%)");
                                ui.selectable_value(&mut h_kind, 2, "Fill");
                            });

                        match h_kind {
                            0 => {
                                if !matches!(w.props.height_policy, SizePolicy::Fixed) {
                                    w.props.height_policy = SizePolicy::Fixed;
                                }
                                ui.add(egui::DragValue::new(&mut w.size.y).range(8.0..=2000.0).suffix(" px"));
                            }
                            1 => {
                                let mut pct = match w.props.height_policy {
                                    SizePolicy::Percent(p) => p,
                                    _ => 100.0,
                                };
                                if ui.add(egui::DragValue::new(&mut pct).range(1.0..=100.0).suffix(" %")).changed() || !matches!(w.props.height_policy, SizePolicy::Percent(_)) {
                                    w.props.height_policy = SizePolicy::Percent(pct);
                                }
                            }
                            2 => {
                                if !matches!(w.props.height_policy, SizePolicy::Fill) {
                                    w.props.height_policy = SizePolicy::Fill;
                                }
                                ui.label("(fills remaining space)");
                            }
                            _ => {}
                        }
                    });

                    // Align Self
                    ui.horizontal(|ui| {
                        ui.label("Align self:");
                        let mut current_align = w.props.align_self;
                        egui::ComboBox::from_id_salt(("align_self_combo", w.id))
                            .selected_text(match current_align {
                                None => "Inherit (parent)",
                                Some(a) => a.display_name(),
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut current_align, None, "Inherit (parent)");
                                for a in Align::all() {
                                    ui.selectable_value(&mut current_align, Some(*a), a.display_name());
                                }
                            });
                        w.props.align_self = current_align;
                    });
                } else {
                    ui.label("Position / Size");
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(egui::DragValue::new(&mut w.pos.x));
                        ui.label("y");
                        ui.add(egui::DragValue::new(&mut w.pos.y));
                    });
                    ui.horizontal(|ui| {
                        ui.label("w");
                        ui.add(egui::DragValue::new(&mut w.size.x).range(16.0..=2000.0));
                        ui.label("h");
                        ui.add(egui::DragValue::new(&mut w.size.y).range(12.0..=2000.0));
                    });
                }

                ui.separator();
                ui.label(egui::RichText::new("📱 Responsive").strong());
                ui.horizontal(|ui| {
                    ui.label("Visibility:");
                    egui::ComboBox::from_id_salt(("widget_responsive_vis", w.id))
                        .selected_text(w.props.responsive_vis.display_name())
                        .show_ui(ui, |ui| {
                            for rv in ResponsiveVisibility::all() {
                                ui.selectable_value(&mut w.props.responsive_vis, *rv, rv.display_name());
                            }
                        });
                });

                ui.separator();
                ui.collapsing(egui::RichText::new("🎨 Appearance & Style").strong(), |ui| {
                    // Background Color
                    ui.horizontal(|ui| {
                        let mut has_bg = w.props.bg_color.is_some();
                        if ui.checkbox(&mut has_bg, "Background").changed() {
                            w.props.bg_color = if has_bg { Some([35, 35, 45, 255]) } else { None };
                        }
                        if let Some(ref mut c) = w.props.bg_color {
                            let mut color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                            if egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                                *c = [color.r(), color.g(), color.b(), color.a()];
                            }
                        }
                    });

                    // Text / Foreground Color
                    ui.horizontal(|ui| {
                        let mut has_tc = w.props.text_color.is_some();
                        if ui.checkbox(&mut has_tc, "Text Color").changed() {
                            w.props.text_color = if has_tc { Some([255, 255, 255, 255]) } else { None };
                        }
                        if let Some(ref mut c) = w.props.text_color {
                            let mut color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                            if egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                                *c = [color.r(), color.g(), color.b(), color.a()];
                            }
                        }
                    });

                    // Typography: Font Size & Bold
                    ui.horizontal(|ui| {
                        let mut has_fs = w.props.font_size.is_some();
                        if ui.checkbox(&mut has_fs, "Font Size").changed() {
                            w.props.font_size = if has_fs { Some(14.0) } else { None };
                        }
                        if let Some(ref mut sz) = w.props.font_size {
                            ui.add(egui::DragValue::new(sz).range(8.0..=72.0).suffix(" pt"));
                        }
                        ui.checkbox(&mut w.props.font_bold, "Bold");
                    });

                    // Border / Stroke
                    ui.horizontal(|ui| {
                        let mut has_border = w.props.border_color.is_some();
                        if ui.checkbox(&mut has_border, "Border").changed() {
                            if has_border {
                                w.props.border_color = Some([80, 80, 100, 255]);
                                if w.props.border_width.is_none() {
                                    w.props.border_width = Some(1.0);
                                }
                            } else {
                                w.props.border_color = None;
                                w.props.border_width = None;
                            }
                        }
                        if let Some(ref mut bc) = w.props.border_color {
                            let mut color = Color32::from_rgba_unmultiplied(bc[0], bc[1], bc[2], bc[3]);
                            if egui::color_picker::color_edit_button_srgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                                *bc = [color.r(), color.g(), color.b(), color.a()];
                            }
                            let mut bw = w.props.border_width.unwrap_or(1.0);
                            if ui.add(egui::DragValue::new(&mut bw).range(0.5..=10.0).suffix(" px")).changed() {
                                w.props.border_width = Some(bw);
                            }
                        }
                    });

                    // Corner Radius (Border Radius)
                    ui.horizontal(|ui| {
                        let mut has_cr = w.props.corner_radius.is_some();
                        if ui.checkbox(&mut has_cr, "Corner Radius").changed() {
                            w.props.corner_radius = if has_cr { Some(4.0) } else { None };
                        }
                        if let Some(ref mut cr) = w.props.corner_radius {
                            ui.add(egui::DragValue::new(cr).range(0.0..=50.0).suffix(" px"));
                        }
                    });
                });

                ui.separator();
                ui.label("Tooltip (optional)");
                ui.add(
                    egui::TextEdit::singleline(&mut w.props.tooltip)
                        .id_salt(("inspector_tooltip", w.id)),
                );

                ui.separator();
                ui.label(egui::RichText::new("⚡ Actions").strong());

                let mut remove_action_idx: Option<usize> = None;

                if w.props.actions.is_empty() {
                    ui.weak("No actions configured");
                } else {
                    for (i, act) in w.props.actions.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let summary = match &act.effect {
                                ActionEffect::ShowWidget(t) => {
                                    let t_name = target_widget_names.get(t).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Show {}", t_name)
                                }
                                ActionEffect::HideWidget(t) => {
                                    let t_name = target_widget_names.get(t).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Hide {}", t_name)
                                }
                                ActionEffect::ToggleWidget(t) => {
                                    let t_name = target_widget_names.get(t).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Toggle {}", t_name)
                                }
                                ActionEffect::SetText { target, text } => {
                                    let t_name = target_widget_names.get(target).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Set {} text = \"{}\"", t_name, text)
                                }
                                ActionEffect::SwitchTab { target, tab_index } => {
                                    let t_name = target_widget_names.get(target).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Switch {} tab = {}", t_name, tab_index)
                                }
                                ActionEffect::OpenModal(t) => {
                                    let t_name = target_widget_names.get(t).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Open {}", t_name)
                                }
                                ActionEffect::CloseModal(t) => {
                                    let t_name = target_widget_names.get(t).map(|s| s.as_str()).unwrap_or("Unknown");
                                    format!("Close {}", t_name)
                                }
                                ActionEffect::CustomRustCode(code) => {
                                    let snippet = if code.len() > 15 {
                                        format!("{}...", &code[..15])
                                    } else {
                                        code.clone()
                                    };
                                    format!("Code: {}", snippet)
                                }
                            };

                            ui.label(format!("• [{}]: {}", act.trigger.display_name(), summary));
                            if ui.small_button("⚡").on_hover_text("Test this action now").clicked() {
                                test_action = Some(act.effect.clone());
                            }
                            if ui.small_button("❌").on_hover_text("Delete this action").clicked() {
                                remove_action_idx = Some(i);
                            }
                        });
                    }
                }

                if let Some(idx) = remove_action_idx {
                    w.props.actions.remove(idx);
                    undo_needed = true;
                }

                ui.add_space(2.0);
                egui::CollapsingHeader::new("+ Add Action")
                    .id_salt(("add_action_section", w.id))
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Trigger:");
                            egui::ComboBox::from_id_salt(("action_draft_trigger", w.id))
                                .selected_text(action_draft.trigger.display_name())
                                .show_ui(ui, |ui| {
                                    for &trig in ActionTrigger::ALL {
                                        ui.selectable_value(&mut action_draft.trigger, trig, trig.display_name());
                                    }
                                });
                        });

                        const EFFECT_LABELS: &[&str] = &[
                            "Toggle Visibility",
                            "Show Widget",
                            "Hide Widget",
                            "Set Text",
                            "Switch Tab",
                            "Open Window/Modal",
                            "Close Window/Modal",
                            "Custom Rust Code",
                        ];

                        ui.horizontal(|ui| {
                            ui.label("Effect:");
                            egui::ComboBox::from_id_salt(("action_draft_effect", w.id))
                                .selected_text(EFFECT_LABELS[action_draft.effect_kind.min(EFFECT_LABELS.len() - 1)])
                                .show_ui(ui, |ui| {
                                    for (k, &name) in EFFECT_LABELS.iter().enumerate() {
                                        ui.selectable_value(&mut action_draft.effect_kind, k, name);
                                    }
                                });
                        });

                        // Target selector for effects 0..=6
                        if action_draft.effect_kind < 7 {
                            let current_target_label = match action_draft.target {
                                Some(tid) => target_widget_names.get(&tid).cloned().unwrap_or_else(|| format!("#{tid}")),
                                None => "(Select Target)".to_string(),
                            };
                            ui.horizontal(|ui| {
                                ui.label("Target:");
                                egui::ComboBox::from_id_salt(("action_draft_target", w.id))
                                    .selected_text(current_target_label)
                                    .show_ui(ui, |ui| {
                                        for (tid, label) in &all_target_widgets {
                                            ui.selectable_value(&mut action_draft.target, Some(*tid), label);
                                        }
                                    });
                            });
                        }

                        // Specific parameter inputs
                        match action_draft.effect_kind {
                            3 => {
                                ui.horizontal(|ui| {
                                    ui.label("Text:");
                                    ui.text_edit_singleline(&mut action_draft.text);
                                });
                            }
                            4 => {
                                ui.horizontal(|ui| {
                                    ui.label("Tab index:");
                                    ui.add(egui::DragValue::new(&mut action_draft.tab).range(0..=50));
                                });
                            }
                            7 => {
                                ui.label("Rust code:");
                                ui.text_edit_multiline(&mut action_draft.code);
                            }
                            _ => {}
                        }

                        if ui.button("Confirm Add Action").clicked() {
                            let maybe_effect = match action_draft.effect_kind {
                                0 => action_draft.target.map(ActionEffect::ToggleWidget),
                                1 => action_draft.target.map(ActionEffect::ShowWidget),
                                2 => action_draft.target.map(ActionEffect::HideWidget),
                                3 => action_draft.target.map(|t| ActionEffect::SetText { target: t, text: action_draft.text.clone() }),
                                4 => action_draft.target.map(|t| ActionEffect::SwitchTab { target: t, tab_index: action_draft.tab }),
                                5 => action_draft.target.map(ActionEffect::OpenModal),
                                6 => action_draft.target.map(ActionEffect::CloseModal),
                                7 => Some(ActionEffect::CustomRustCode(action_draft.code.clone())),
                                _ => None,
                            };

                            if let Some(effect) = maybe_effect {
                                action_to_add = Some(WidgetAction {
                                    trigger: action_draft.trigger,
                                    effect,
                                });
                            }
                        }
                    });

                ui.add_space(6.0);
                if ui.button("Delete").clicked() {
                    do_delete = true;
                }
            } else {
                ui.weak("No selection");
            }
        });

        self.action_draft = action_draft;

        if let Some(effect) = test_action {
            Self::apply_action_effect(&effect, &mut self.project.widgets);
        }
        if let Some(act) = action_to_add {
            self.push_undo();
            if let Some(w) = self.selected_mut() {
                w.props.actions.push(act);
            }
        }
        if undo_needed {
            self.push_undo();
        }

        if new_parent != current_parent {
            self.push_undo();
            self.reparent_widget(sel_id, new_parent);
        }
        if do_delete {
            self.delete_selected();
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.push_id("top_bar", |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
            ui.push_id("menu_file", |ui| {
                ui.menu_button("File", |ui| {
                if ui
                    .button("New Project")
                    .on_hover_text("Create a new empty project")
                    .clicked()
                {
                    self.project = Project::default();
                    self.next_id = 1;
                    self.selected.clear();
                    self.history.clear();
                    self.current_file = None;
                    self.set_status("New project created".into());
                    ui.close_kind(egui::UiKind::Menu);
                }
                ui.separator();
                // Examples submenu (Arduino IDE style)
                ui.menu_button("Examples", |ui| {
                    if ui.button("01. Mobile Navigation & Multi-View").on_hover_text(
                        "Mobile app: Top Bar + 3 views (Home/Detail/Profile) + Bottom Navigation with Back button"
                    ).clicked() {
                        let proj = crate::examples::mobile_navigation_example();
                        self.load_example(proj, "Mobile Navigation & Multi-View");
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui.button("02. Hamburger Drawer (Side Menu)").on_hover_text(
                        "App with hamburger button that opens/closes a side navigation drawer"
                    ).clicked() {
                        let proj = crate::examples::hamburger_drawer_example();
                        self.load_example(proj, "Hamburger Drawer (Side Menu)");
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui.button("03. Adaptive Responsive Screen").on_hover_text(
                        "Portrait/Landscape responsive layout: single column in portrait, 2-column side-by-side in landscape"
                    ).clicked() {
                        let proj = crate::examples::responsive_adaptive_example();
                        self.load_example(proj, "Adaptive Responsive Screen");
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui.button("04. Multi-Step Form Wizard").on_hover_text(
                        "3-step wizard form with Next and Back navigation between steps"
                    ).clicked() {
                        let proj = crate::examples::multi_step_wizard_example();
                        self.load_example(proj, "Multi-Step Form Wizard");
                        ui.close_kind(egui::UiKind::Menu);
                    }
                });
                ui.separator();
                if ui
                    .button("Open...")
                    .on_hover_text("Open a project file (Ctrl+O)")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RAD Project", &["json", "rad"])
                        .pick_file()
                    {
                        self.load_project(path);
                    }
                    ui.close_kind(egui::UiKind::Menu);
                }
                if ui
                    .button("Save")
                    .on_hover_text("Save project (Ctrl+S)")
                    .clicked()
                {
                    if let Some(path) = self.current_file.clone() {
                        self.save_project(path);
                    } else if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RAD Project", &["json", "rad"])
                        .set_file_name("project.json")
                        .save_file()
                    {
                        self.save_project(path);
                    }
                    ui.close_kind(egui::UiKind::Menu);
                }
                if ui
                    .button("Save As...")
                    .on_hover_text("Save project to a new file")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RAD Project", &["json", "rad"])
                        .set_file_name("project.json")
                        .save_file()
                    {
                        self.save_project(path);
                    }
                    ui.close_kind(egui::UiKind::Menu);
                }
                ui.separator();
                if ui
                    .button("Generate Code")
                    .on_hover_text("Generate Rust code (Ctrl+G)")
                    .clicked()
                {
                    self.generated = self.generate_code();
                    ui.close_kind(egui::UiKind::Menu);
                }
                if ui
                    .button("Export JSON")
                    .on_hover_text("Export project as JSON to the editor")
                    .clicked()
                {
                    if let Ok(s) = serde_json::to_string_pretty(&self.project) {
                        self.generated = s;
                    }
                    ui.close_kind(egui::UiKind::Menu);
                }
                if ui
                    .button("Import JSON")
                    .on_hover_text("Import project from the editor below")
                    .clicked()
                {
                    if let Ok(p) = serde_json::from_str::<Project>(&self.generated) {
                        self.project = p;
                        self.normalize_project_widget_ids();
                        self.selected.clear();
                        self.history.clear();
                    }
                    ui.close_kind(egui::UiKind::Menu);
                }
                });
            });

            ui.push_id("menu_edit", |ui| {
                ui.menu_button("Edit", |ui| {
                let can_undo = self.history.can_undo();
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .on_hover_text("Undo last change (Ctrl+Z)")
                    .clicked()
                {
                    self.undo();
                    ui.close_kind(egui::UiKind::Menu);
                }
                let can_redo = self.history.can_redo();
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .on_hover_text("Redo last undone change (Ctrl+Y or Ctrl+Shift+Z)")
                    .clicked()
                {
                    self.redo();
                    ui.close_kind(egui::UiKind::Menu);
                }
                ui.separator();
                let has_selection = !self.selected.is_empty();
                let _multi_selected = self.selected.len() > 1;

                ui.add_enabled_ui(has_selection, |ui| {
                    if ui
                        .button("Delete")
                        .on_hover_text("Delete selected (Del)")
                        .clicked()
                    {
                        self.delete_selected();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("Duplicate")
                        .on_hover_text("Duplicate selected (Ctrl+D)")
                        .clicked()
                    {
                        self.duplicate_selected();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("Copy")
                        .on_hover_text("Copy selected (Ctrl+C)")
                        .clicked()
                    {
                        if let Some(&sel_id) = self.selected.first()
                            && let Some(w) = self.project.widgets.iter().find(|w| w.id == sel_id)
                        {
                            self.clipboard = Some(w.clone());
                        }
                        ui.close_kind(egui::UiKind::Menu);
                    }
                });
                if ui
                    .add_enabled(self.clipboard.is_some(), egui::Button::new("Paste"))
                    .on_hover_text("Paste from clipboard (Ctrl+V)")
                    .clicked()
                {
                    self.paste();
                    ui.close_kind(egui::UiKind::Menu);
                }
                ui.separator();
                if ui
                    .button("Select All")
                    .on_hover_text("Select all widgets")
                    .clicked()
                {
                    self.selected = self.project.widgets.iter().map(|w| w.id).collect();
                    ui.close_kind(egui::UiKind::Menu);
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("Deselect All"))
                    .on_hover_text("Clear selection")
                    .clicked()
                {
                    self.selected.clear();
                    ui.close_kind(egui::UiKind::Menu);
                }
                });
            });

            // Alignment menu (only enabled with multi-select)
            ui.push_id("menu_align", |ui| {
                ui.menu_button("Align", |ui| {
                let multi_selected = self.selected.len() > 1;
                ui.add_enabled_ui(multi_selected, |ui| {
                    ui.label("Horizontal:");
                    if ui
                        .button("⬅ Left")
                        .on_hover_text("Align left edges")
                        .clicked()
                    {
                        self.align_left();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("⬌ Center")
                        .on_hover_text("Align centers horizontally")
                        .clicked()
                    {
                        self.align_center_h();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("➡ Right")
                        .on_hover_text("Align right edges")
                        .clicked()
                    {
                        self.align_right();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    ui.separator();
                    ui.label("Vertical:");
                    if ui
                        .button("⬆ Top")
                        .on_hover_text("Align top edges")
                        .clicked()
                    {
                        self.align_top();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("⬍ Middle")
                        .on_hover_text("Align centers vertically")
                        .clicked()
                    {
                        self.align_center_v();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("⬇ Bottom")
                        .on_hover_text("Align bottom edges")
                        .clicked()
                    {
                        self.align_bottom();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    ui.separator();
                    ui.label("Distribute:");
                    if ui
                        .button("↔ Horizontal")
                        .on_hover_text("Distribute evenly horizontally")
                        .clicked()
                    {
                        self.distribute_horizontal();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("↕ Vertical")
                        .on_hover_text("Distribute evenly vertically")
                        .clicked()
                    {
                        self.distribute_vertical();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    ui.separator();
                    ui.label("Size:");
                    if ui
                        .button("Match Width")
                        .on_hover_text("Make all same width")
                        .clicked()
                    {
                        self.match_width();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .button("Match Height")
                        .on_hover_text("Make all same height")
                        .clicked()
                    {
                        self.match_height();
                        ui.close_kind(egui::UiKind::Menu);
                    }
                });
                if !multi_selected {
                    ui.label("Select 2+ widgets to align");
                }
            });
            });

            ui.push_id("menu_view", |ui| {
                ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.palette_open, "Show Palette");
                ui.checkbox(&mut self.show_grid, "Show Grid");
                ui.checkbox(&mut self.syntax_highlighting, "Syntax Highlighting")
                    .on_hover_text("Enable syntax highlighting in code output");
                ui.separator();
                ui.checkbox(&mut self.preview_mode, "Preview Mode (F5)")
                    .on_hover_text("Toggle preview mode: interact with widgets without selection handles");
            });
            });

            ui.push_id("menu_settings", |ui| {
                ui.menu_button("Settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Grid Size");
                    ui.add(egui::DragValue::new(&mut self.grid_size).range(1.0..=64.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Canvas size");
                    ui.add(egui::DragValue::new(&mut self.project.canvas_size.x));
                    ui.add(egui::DragValue::new(&mut self.project.canvas_size.y));
                });
                ui.separator();
                ui.strong("Panels (inside viewport)");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.project.panel_top_enabled, "Top");
                    if self.project.panel_top_enabled {
                        ui.label("h:");
                        ui.add(egui::DragValue::new(&mut self.project.panel_top_height).range(20.0..=300.0).speed(1.0).suffix(" px"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.project.panel_bottom_enabled, "Bottom");
                    if self.project.panel_bottom_enabled {
                        ui.label("h:");
                        ui.add(egui::DragValue::new(&mut self.project.panel_bottom_height).range(20.0..=300.0).speed(1.0).suffix(" px"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.project.panel_left_enabled, "Left");
                    if self.project.panel_left_enabled {
                        ui.label("w:");
                        ui.add(egui::DragValue::new(&mut self.project.panel_left_width).range(20.0..=600.0).speed(1.0).suffix(" px"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.project.panel_right_enabled, "Right");
                    if self.project.panel_right_enabled {
                        ui.label("w:");
                        ui.add(egui::DragValue::new(&mut self.project.panel_right_width).range(20.0..=600.0).speed(1.0).suffix(" px"));
                    }
                });
                ui.separator();
                ui.strong("Code Generation");
                ui.add_space(4.0);
                ui.checkbox(&mut self.auto_generate, "Auto-generate code")
                    .on_hover_text("Automatically regenerate code when widgets change");
                ui.checkbox(&mut self.codegen_comments, "Include comments")
                    .on_hover_text("Add explanatory comments to generated code");
                ui.separator();
                ui.label(format!(
                    "Output format: {}",
                    self.codegen_format.display_name()
                ));
                ui.selectable_value(
                    &mut self.codegen_format,
                    CodeGenFormat::SingleFile,
                    "Single File",
                );
                ui.selectable_value(
                    &mut self.codegen_format,
                    CodeGenFormat::SeparateFiles,
                    "Separate Files",
                );
                ui.selectable_value(
                    &mut self.codegen_format,
                    CodeGenFormat::UiOnly,
                    "UI Function Only",
                );
                });
            });

            ui.separator();
            ui.push_id("menu_device_presets", |ui| {
                ui.menu_button(self.project.screen_preset.display_name(), |ui| {
                    for preset in ScreenPreset::all() {
                        let selected = self.project.screen_preset == *preset;
                        if ui.selectable_label(selected, preset.display_name()).clicked() {
                            self.project.screen_preset = *preset;
                            if let Some(dim) = preset.dimensions() {
                                self.project.canvas_size = dim;
                            }
                            ui.close_kind(egui::UiKind::Menu);
                        }
                    }
                });
            });

            if ui.button("🔄").on_hover_text("Rotate screen orientation (swap width <-> height)").clicked() {
                let temp = self.project.canvas_size.x;
                self.project.canvas_size.x = self.project.canvas_size.y;
                self.project.canvas_size.y = temp;
                self.project.screen_preset = ScreenPreset::Custom;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Generate Code").on_hover_text("Ctrl+G").clicked() {
                    self.generated = self.generate_code();
                    self.set_status("Code generated".into());
                }
                // Preview/Edit mode toggle button
                ui.separator();
                let mode_label = if self.preview_mode { "Preview" } else { "Edit" };
                let mode_color = if self.preview_mode {
                    Color32::from_rgb(100, 180, 100) // Green for preview
                } else {
                    Color32::from_rgb(100, 160, 255) // Blue for edit
                };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(mode_label).color(mode_color))
                            .min_size(vec2(60.0, 0.0)),
                    )
                    .on_hover_text("Toggle Preview/Edit mode (F5)")
                    .clicked()
                {
                    self.preview_mode = !self.preview_mode;
                }
                // Show selection count
                if !self.selected.is_empty() {
                    ui.separator();
                    ui.label(format!("{} selected", self.selected.len()));
                }
            });
            });
        });
    }

    /// Permanent bottom status bar displaying status messages, platform, and version.
    fn status_bar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Left: status message (no emojis)
            ui.label(&self.status_message);

            // Right: status details (aligned right-to-left, no emojis)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Version & Platform
                let version = env!("CARGO_PKG_VERSION");
                let platform = Self::platform_info();
                ui.label(format!("v{} ({})", version, platform));
                ui.separator();

                // Canvas dimensions & preset
                ui.label(format!(
                    "{:.0}x{:.0} ({})",
                    self.project.canvas_size.x,
                    self.project.canvas_size.y,
                    self.project.screen_preset.display_name()
                ));
                ui.separator();

                // Selection & widget count
                let widget_count = self.project.widgets.len();
                if !self.selected.is_empty() {
                    ui.label(format!("{}/{} selected", self.selected.len(), widget_count));
                } else {
                    ui.label(format!("{} widgets", widget_count));
                }
            });
        });
    }

    // Alignment functions
    fn align_left(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let min_x = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.pos.x)
            .fold(f32::INFINITY, f32::min);
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.x = min_x;
            }
        }
    }

    fn align_right(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let max_right = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.pos.x + w.size.x)
            .fold(f32::NEG_INFINITY, f32::max);
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.x = max_right - w.size.x;
            }
        }
    }

    fn align_center_h(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let centers: Vec<f32> = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.pos.x + w.size.x / 2.0)
            .collect();
        let avg_center = centers.iter().sum::<f32>() / centers.len() as f32;
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.x = avg_center - w.size.x / 2.0;
            }
        }
    }

    fn align_top(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let min_y = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.pos.y)
            .fold(f32::INFINITY, f32::min);
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.y = min_y;
            }
        }
    }

    fn align_bottom(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let max_bottom = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.pos.y + w.size.y)
            .fold(f32::NEG_INFINITY, f32::max);
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.y = max_bottom - w.size.y;
            }
        }
    }

    fn align_center_v(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let centers: Vec<f32> = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.pos.y + w.size.y / 2.0)
            .collect();
        let avg_center = centers.iter().sum::<f32>() / centers.len() as f32;
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.y = avg_center - w.size.y / 2.0;
            }
        }
    }

    fn distribute_horizontal(&mut self) {
        if self.selected.len() < 3 {
            return;
        }
        self.push_undo();
        let mut widgets: Vec<_> = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| (w.id, w.pos.x, w.size.x))
            .collect();
        widgets.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let first_left = widgets.first().map(|w| w.1).unwrap_or(0.0);
        let last_right = widgets.last().map(|w| w.1 + w.2).unwrap_or(0.0);
        let total_width: f32 = widgets.iter().map(|w| w.2).sum();
        let spacing = (last_right - first_left - total_width) / (widgets.len() - 1) as f32;

        let mut x = first_left;
        for (id, _, width) in &widgets {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.x = x;
            }
            x += width + spacing;
        }
    }

    fn distribute_vertical(&mut self) {
        if self.selected.len() < 3 {
            return;
        }
        self.push_undo();
        let mut widgets: Vec<_> = self
            .selected
            .iter()
            .filter_map(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| (w.id, w.pos.y, w.size.y))
            .collect();
        widgets.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let first_top = widgets.first().map(|w| w.1).unwrap_or(0.0);
        let last_bottom = widgets.last().map(|w| w.1 + w.2).unwrap_or(0.0);
        let total_height: f32 = widgets.iter().map(|w| w.2).sum();
        let spacing = (last_bottom - first_top - total_height) / (widgets.len() - 1) as f32;

        let mut y = first_top;
        for (id, _, height) in &widgets {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.pos.y = y;
            }
            y += height + spacing;
        }
    }

    fn match_width(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        // Use width of first selected widget
        let target_width = self
            .selected
            .first()
            .and_then(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.size.x)
            .unwrap_or(100.0);
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.size.x = target_width;
            }
        }
    }

    fn match_height(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        // Use height of first selected widget
        let target_height = self
            .selected
            .first()
            .and_then(|id| self.project.widgets.iter().find(|w| w.id == *id))
            .map(|w| w.size.y)
            .unwrap_or(30.0);
        for id in &self.selected {
            if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *id) {
                w.size.y = target_height;
            }
        }
    }

    fn generated_panel(&mut self, ui: &mut egui::Ui) {
        ui.push_id("generated_panel", |ui| {
            ui.horizontal(|ui| {
                ui.heading("Generated Output");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.syntax_highlighting, "Syntax Highlighting")
                        .on_hover_text(
                            "Toggle syntax highlighting (may affect performance with large code)",
                        );
                });
            });
            ui.label("Rust code (or JSON export) will appear here. Copy-paste into your app.");

            // A scrollable viewport for the generated text:
            egui::ScrollArea::vertical()
                .id_salt("generated_output_scroll")
                .max_height(280.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.syntax_highlighting && !self.generated.is_empty() {
                        // Display with syntax highlighting (read-only view)
                        let job = self.highlighter.layout_job(&self.generated);
                        ui.add(egui::Label::new(job).selectable(true));
                    } else {
                        // Plain text editor (editable)
                        let editor = egui::TextEdit::multiline(&mut self.generated)
                            .id_salt("generated_output_editor")
                            .code_editor()
                            .lock_focus(true)
                            .desired_rows(18)
                            .desired_width(f32::INFINITY);
                        ui.add(editor);
                    }
                });
        });
    }

    fn generate_code(&self) -> String {
        match self.codegen_format {
            CodeGenFormat::SingleFile => self.generate_single_file(),
            CodeGenFormat::SeparateFiles => self.generate_separate_files(),
            CodeGenFormat::UiOnly => self.generate_ui_only(),
        }
    }

    /// Generate all code in a single file
    fn generate_single_file(&self) -> String {
        use DockArea::*;
        let mut out = String::new();

        // Header comment
        if self.codegen_comments {
            out.push_str("// =============================================================================\n");
            out.push_str("// Generated by egui RAD GUI Builder\n");
            out.push_str("// https://github.com/timschmidt/egui-rad-builder\n");
            out.push_str("// =============================================================================\n\n");
        } else {
            out.push_str("// --- generated by egui RAD GUI Builder ---\n");
        }

        out.push_str("use eframe::egui;\n");
        out.push_str("use egui_extras::DatePickerButton;\n");
        out.push_str("use chrono::NaiveDate;\n\n");

        let has_tree = self
            .project
            .widgets
            .iter()
            .any(|w| matches!(w.kind, WidgetKind::Tree));
        if has_tree {
            out.push_str(
                "#[derive(Clone)]\n\
				 struct GenTreeNode { label: String, children: Vec<GenTreeNode> }\n\
				 \n\
				 fn gen_show_tree(ui: &mut egui::Ui, nodes: &[GenTreeNode], path: &mut Vec<usize>) {\n\
				 \tfor (idx, n) in nodes.iter().enumerate() {\n\
				 \t\tif n.children.is_empty() { ui.label(&n.label); }\n\
				 \t\telse {\n\
				 \t\t\tpath.push(idx);\n\
				 \t\t\tegui::CollapsingHeader::new(&n.label).id_salt((\"tree_node\", path.clone())).show(ui, |ui| gen_show_tree(ui, &n.children, path));\n\
				 \t\t\tpath.pop();\n\
				 \t\t}\n\
				 \t}\n\
				 }\n\n",
            );
        }

        let visibility_toggled_ids: HashSet<WidgetId> = self
            .project
            .widgets
            .iter()
            .flat_map(|w| &w.props.actions)
            .filter_map(|a| match a.effect {
                ActionEffect::ShowWidget(id)
                | ActionEffect::HideWidget(id)
                | ActionEffect::ToggleWidget(id) => Some(id),
                _ => None,
            })
            .collect();

        let text_updated_ids: HashSet<WidgetId> = self
            .project
            .widgets
            .iter()
            .flat_map(|w| &w.props.actions)
            .filter_map(|a| match a.effect {
                ActionEffect::SetText { target, .. } => Some(target),
                _ => None,
            })
            .collect();

        out.push_str("struct GeneratedState {\n");
        out.push_str(
            "    enable_top: bool, enable_bottom: bool, enable_left: bool, enable_right: bool,\n",
        );
        for w in &self.project.widgets {
            if !w.props.active {
                continue;
            }
            if !w.props.initially_visible || visibility_toggled_ids.contains(&w.id) {
                out.push_str(&format!("    show_{}: bool,\n", w.id));
            }
            if text_updated_ids.contains(&w.id) && w.kind != WidgetKind::TextEdit {
                out.push_str(&format!("    text_{}: String,\n", w.id));
            }
            match w.kind {
                WidgetKind::TextEdit => out.push_str(&format!("    text_{}: String,\n", w.id)),
                WidgetKind::Checkbox => out.push_str(&format!("    checked_{}: bool,\n", w.id)),
                WidgetKind::Slider => out.push_str(&format!("    value_{}: f32,\n", w.id)),
                WidgetKind::ProgressBar => out.push_str(&format!("    progress_{}: f32,\n", w.id)),
                WidgetKind::SelectableLabel => out.push_str(&format!("    sel_{}: bool,\n", w.id)),
                WidgetKind::RadioGroup | WidgetKind::ComboBox | WidgetKind::MenuButton => {
                    out.push_str(&format!("    sel_{}: usize,\n", w.id))
                }
                WidgetKind::TabBar => out.push_str(&format!("    tab_{}: usize,\n", w.id)),
                WidgetKind::Window => out.push_str(&format!("    window_{}_open: bool,\n", w.id)),
                WidgetKind::CollapsingHeader => {
                    out.push_str(&format!("    open_{}: bool,\n", w.id))
                }
                WidgetKind::DatePicker => out.push_str(&format!("    date_{}: NaiveDate,\n", w.id)),
                WidgetKind::Password => out.push_str(&format!("    pass_{}: String,\n", w.id)),
                WidgetKind::AngleSelector => out.push_str(&format!("    angle_{}: f32,\n", w.id)),
                WidgetKind::TextArea => out.push_str(&format!("    textarea_{}: String,\n", w.id)),
                WidgetKind::DragValue => out.push_str(&format!("    drag_{}: f32,\n", w.id)),
                WidgetKind::ColorPicker => {
                    out.push_str(&format!("    color_{}: egui::Color32,\n", w.id))
                }
                WidgetKind::Code => out.push_str(&format!("    code_{}: String,\n", w.id)),
                _ => {}
            }
        }
        out.push_str("}\n\n");

        out.push_str("impl Default for GeneratedState {\n");
        out.push_str("    fn default() -> Self {\n");
        out.push_str("        Self {\n");
        out.push_str(&format!(
            "            enable_top: {}, enable_bottom: {}, enable_left: {}, enable_right: {},\n",
            if self.project.panel_top_enabled {
                "true"
            } else {
                "false"
            },
            if self.project.panel_bottom_enabled {
                "true"
            } else {
                "false"
            },
            if self.project.panel_left_enabled {
                "true"
            } else {
                "false"
            },
            if self.project.panel_right_enabled {
                "true"
            } else {
                "false"
            },
        ));

        for w in &self.project.widgets {
            if !w.props.active {
                continue;
            }
            if !w.props.initially_visible || visibility_toggled_ids.contains(&w.id) {
                out.push_str(&format!(
                    "            show_{}: {},\n",
                    w.id,
                    if w.props.initially_visible { "true" } else { "false" }
                ));
            }
            if text_updated_ids.contains(&w.id) && w.kind != WidgetKind::TextEdit {
                out.push_str(&format!(
                    "            text_{}: \"{}\".to_owned(),\n",
                    w.id,
                    widget::escape(&w.props.text)
                ));
            }
            match w.kind {
                WidgetKind::TextEdit => {
                    out.push_str(&format!(
                        "            text_{}: \"{}\".to_owned(),\n",
                        w.id,
                        widget::escape(&w.props.text)
                    ));
                }
                WidgetKind::Checkbox => {
                    out.push_str(&format!(
                        "            checked_{}: {},\n",
                        w.id,
                        if w.props.checked { "true" } else { "false" }
                    ));
                }
                WidgetKind::Slider => {
                    out.push_str(&format!(
                        "            value_{}: {:.3},\n",
                        w.id, w.props.value
                    ));
                }
                WidgetKind::ProgressBar => {
                    let p = w.props.value.clamp(0.0, 1.0);
                    out.push_str(&format!("            progress_{}: {:.3},\n", w.id, p));
                }
                WidgetKind::SelectableLabel => {
                    out.push_str(&format!(
                        "            sel_{}: {},\n",
                        w.id,
                        if w.props.checked { "true" } else { "false" }
                    ));
                }
                WidgetKind::RadioGroup | WidgetKind::ComboBox | WidgetKind::MenuButton => {
                    let sel = if w.props.items.is_empty() {
                        0
                    } else {
                        w.props.selected.min(w.props.items.len() - 1)
                    };
                    out.push_str(&format!("            sel_{}: {},\n", w.id, sel));
                }
                WidgetKind::TabBar => {
                    out.push_str(&format!("            tab_{}: {},\n", w.id, w.props.selected));
                }
                WidgetKind::Window => {
                    out.push_str(&format!(
                        "            window_{}_open: {},\n",
                        w.id,
                        if w.props.checked { "true" } else { "false" }
                    ));
                }
                WidgetKind::CollapsingHeader => {
                    out.push_str(&format!(
                        "            open_{}: {},\n",
                        w.id,
                        if w.props.checked { "true" } else { "false" }
                    ));
                }
                WidgetKind::DatePicker => {
                    let y = w.props.year;
                    let m = w.props.month.clamp(1, 12);
                    let d = w.props.day.clamp(1, 28);
                    out.push_str(&format!(
                        "            date_{}: NaiveDate::from_ymd_opt({}, {}, {}).unwrap(),\n",
                        w.id, y, m, d
                    ));
                }
                WidgetKind::Password => {
                    out.push_str(&format!(
                        "            pass_{}: \"{}\".to_owned(),\n",
                        w.id,
                        widget::escape(&w.props.text)
                    ));
                }
                WidgetKind::AngleSelector => {
                    out.push_str(&format!(
                        "            angle_{}: {:.3},\n",
                        w.id, w.props.value
                    ));
                }
                WidgetKind::TextArea => {
                    out.push_str(&format!(
                        "            textarea_{}: \"{}\".to_owned(),\n",
                        w.id,
                        widget::escape(&w.props.text)
                    ));
                }
                WidgetKind::DragValue => {
                    out.push_str(&format!(
                        "            drag_{}: {:.3},\n",
                        w.id, w.props.value
                    ));
                }
                WidgetKind::ColorPicker => {
                    out.push_str(&format!(
                        "            color_{}: egui::Color32::from_rgba_unmultiplied({}, {}, {}, {}),\n",
                        w.id, w.props.color[0], w.props.color[1], w.props.color[2], w.props.color[3]
                    ));
                }
                WidgetKind::Code => {
                    out.push_str(&format!(
                        "            code_{}: \"{}\".to_owned(),\n",
                        w.id,
                        widget::escape(&w.props.text)
                    ));
                }
                _ => {}
            }
        }
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");

        struct EmitCtx<'a> {
            project: &'a Project,
            visibility_toggled_ids: &'a HashSet<WidgetId>,
            text_updated_ids: &'a HashSet<WidgetId>,
        }

        fn emit_actions(actions: &[WidgetAction], out: &mut String, resp_var: &str) {
            for action in actions {
                let trigger_check = match action.trigger {
                    ActionTrigger::OnClick => format!("{resp_var}.clicked()"),
                    ActionTrigger::OnHover => format!("{resp_var}.hovered()"),
                    ActionTrigger::OnChanged => format!("{resp_var}.changed()"),
                    ActionTrigger::OnDoubleClick => format!("{resp_var}.double_clicked()"),
                };
                out.push_str(&format!("        if {} {{\n", trigger_check));
                match &action.effect {
                    ActionEffect::ShowWidget(target) => {
                        out.push_str(&format!("            state.show_{} = true;\n", target));
                    }
                    ActionEffect::HideWidget(target) => {
                        out.push_str(&format!("            state.show_{} = false;\n", target));
                    }
                    ActionEffect::ToggleWidget(target) => {
                        out.push_str(&format!("            state.show_{} = !state.show_{};\n", target, target));
                    }
                    ActionEffect::SetText { target, text } => {
                        out.push_str(&format!("            state.text_{} = \"{}\".to_owned();\n", target, escape(text)));
                    }
                    ActionEffect::SwitchTab { target, tab_index } => {
                        out.push_str(&format!("            state.tab_{} = {};\n", target, tab_index));
                    }
                    ActionEffect::OpenModal(target) => {
                        out.push_str(&format!("            state.window_{}_open = true;\n", target));
                    }
                    ActionEffect::CloseModal(target) => {
                        out.push_str(&format!("            state.window_{}_open = false;\n", target));
                    }
                    ActionEffect::CustomRustCode(code) => {
                        out.push_str(&format!("            {}\n", code.trim()));
                    }
                }
                out.push_str("        }\n");
            }
        }

        // helper function to emit a widget block at rect (origin + local pos) recursively
        fn emit_widget(ctx: &EmitCtx, w: &Widget, out: &mut String, origin: &str) {
            if !w.props.active {
                return;
            }

            let pos = w.pos;
            let size = w.size;

            let responsive_cond = match w.props.responsive_vis {
                crate::widget::ResponsiveVisibility::Always => None,
                crate::widget::ResponsiveVisibility::PortraitOnly => Some("ui.available_height() > ui.available_width()"),
                crate::widget::ResponsiveVisibility::LandscapeOnly => Some("ui.available_width() >= ui.available_height()"),
                crate::widget::ResponsiveVisibility::MobileOnly => Some("ui.available_width() <= 600.0"),
                crate::widget::ResponsiveVisibility::DesktopOnly => Some("ui.available_width() > 600.0"),
            };
            if let Some(cond) = responsive_cond {
                out.push_str(&format!("    if {} {{\n", cond));
            }

            let needs_vis = !w.props.initially_visible || ctx.visibility_toggled_ids.contains(&w.id);
            if needs_vis {
                let name = if w.props.name.is_empty() {
                    w.kind.display_name().to_string()
                } else {
                    w.props.name.clone()
                };
                let status = if w.props.initially_visible { "starts visible" } else { "starts hidden" };
                out.push_str(&format!("    // {} ({})\n", escape(&name), status));
                out.push_str(&format!("    if state.show_{} {{\n", w.id));
            }

            let needs_opacity = (w.props.opacity - 1.0).abs() > 0.001;
            if needs_opacity {
                out.push_str(&format!(
                    "    ui.scope(|ui| {{ ui.set_opacity({:.2}_f32);\n",
                    w.props.opacity
                ));
            }

            let label_expr = if ctx.text_updated_ids.contains(&w.id) {
                format!("&state.text_{}", w.id)
            } else {
                format!("\"{}\"", escape(&w.props.text))
            };

            out.push_str(&format!("    ui.push_id((\"widget\", {}), |ui| {{\n", w.id));
            match w.kind {
				WidgetKind::MenuButton=>{
					let items_code = if w.props.items.is_empty() {
						"\"Item\".to_string()".to_owned()
					} else {
						w.props.items.iter().map(|s| format!("\"{}\".to_string()", escape(s))).collect::<Vec<_>>().join(", ")
					};
					out.push_str(&format!(
						"    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{\n",
						x=w.pos.x, y=w.pos.y, w=w.size.x, h=w.size.y
					));
					out.push_str(&format!("        let items = vec![{items}];\n", items=items_code));
					out.push_str(&format!("        ui.push_id((\"menu_button\", {}), |ui| {{\n", w.id));
					out.push_str(&format!(
						"            ui.menu_button(\"{}\", |ui| {{\n", escape(&w.props.text)
					));
					out.push_str(&format!(
						"                for (i, it) in items.iter().enumerate() {{ if ui.button(it).clicked() {{ state.sel_{id} = i; ui.close_kind(egui::UiKind::Menu); }} }}\n",
						id = w.id
					));
					out.push_str("            });\n");
					out.push_str("        });\n");
					out.push_str("    });\n");
				}
                WidgetKind::Label => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.label({}); }});\n",
                            pos.x, pos.y, size.x, size.y, label_expr
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.label({});\n",
                            pos.x, pos.y, size.x, size.y, label_expr
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::Small => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.small({}); }});\n",
                            pos.x, pos.y, size.x, size.y, label_expr
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.small({});\n",
                            pos.x, pos.y, size.x, size.y, label_expr
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::Monospace => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.monospace({}); }});\n",
                            pos.x, pos.y, size.x, size.y, label_expr
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.monospace({});\n",
                            pos.x, pos.y, size.x, size.y, label_expr
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::Button => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.add_sized(egui::vec2({:.1},{:.1}), egui::Button::new({})); }});\n",
                            pos.x, pos.y, size.x, size.y, size.x, size.y, label_expr
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.add_sized(egui::vec2({:.1},{:.1}), egui::Button::new({}));\n",
                            pos.x, pos.y, size.x, size.y, size.x, size.y, label_expr
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::ImageTextButton => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                                {origin} + egui::vec2({x:.1},{y:.1}), \
                                egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                                ui.add_sized(egui::vec2({w:.1},{h:.1}), \
                                    egui::Button::new(format!(\"{{}}  {{}}\", \"{icon}\", {text})) \
                                ); \
                            }});\n",
                            x = pos.x,
                            y = pos.y,
                            w = size.x,
                            h = size.y,
                            icon = escape(&w.props.icon),
                            text = label_expr,
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                                {origin} + egui::vec2({x:.1},{y:.1}), \
                                egui::vec2({w:.1},{h:.1}))), |ui| {{\n\
                                let resp = ui.add_sized(egui::vec2({w:.1},{h:.1}), \
                                    egui::Button::new(format!(\"{{}}  {{}}\", \"{icon}\", {text})) \
                                );\n",
                            x = pos.x,
                            y = pos.y,
                            w = size.x,
                            h = size.y,
                            icon = escape(&w.props.icon),
                            text = label_expr,
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::Checkbox => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.checkbox(&mut state.checked_{}, \"{}\"); }});\n",
                            pos.x, pos.y, size.x, size.y, w.id, escape(&w.props.text)
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.checkbox(&mut state.checked_{}, \"{}\");\n",
                            pos.x, pos.y, size.x, size.y, w.id, escape(&w.props.text)
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::TextEdit => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.add_sized(egui::vec2({:.1},{:.1}), egui::TextEdit::singleline(&mut state.text_{}).hint_text(\"{}\")); }});\n",
                        pos.x, pos.y, size.x, size.y, size.x, size.y, w.id, escape(&w.props.text)
                    ));
                }
                WidgetKind::Slider => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.add_sized(egui::vec2({:.1},{:.1}), egui::Slider::new(&mut state.value_{}, {:.3}..={:.3}).text(\"{}\")); }});\n",
                            pos.x, pos.y, size.x, size.y, size.x, size.y, w.id, w.props.min, w.props.max, escape(&w.props.text)
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.add_sized(egui::vec2({:.1},{:.1}), egui::Slider::new(&mut state.value_{}, {:.3}..={:.3}).text(\"{}\"));\n",
                            pos.x, pos.y, size.x, size.y, size.x, size.y, w.id, w.props.min, w.props.max, escape(&w.props.text)
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::ProgressBar => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.add_sized(egui::vec2({:.1},{:.1}), egui::ProgressBar::new(state.progress_{}).show_percentage()); }});\n",
                        pos.x, pos.y, size.x, size.y, size.x, size.y, w.id
                    ));
                }
                WidgetKind::RadioGroup => {
                    let items_code = if w.props.items.is_empty() {
                        "\"Item\".to_string()".to_owned()
                    } else {
                        w.props
                            .items
                            .iter()
                            .map(|s| format!("\"{}\".to_string()", escape(s)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n",
                        pos.x, pos.y, size.x, size.y
                    ));
                    out.push_str(&format!("        let items = vec![{}];\n", items_code));
                    out.push_str(&format!(
                        "        for (i, it) in items.iter().enumerate() {{ if ui.add(egui::RadioButton::new(state.sel_{} == i, it)).clicked() {{ state.sel_{} = i; }} }}\n",
                        w.id, w.id
                    ));
                    out.push_str("    });\n");
                }
                WidgetKind::Link => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.link(\"{}\"); }});\n",
                            pos.x, pos.y, size.x, size.y, escape(&w.props.text)
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.link(\"{}\");\n",
                            pos.x, pos.y, size.x, size.y, escape(&w.props.text)
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::Hyperlink => {
                    if w.props.actions.is_empty() {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.hyperlink_to(\"{}\", \"{}\"); }});\n",
                            pos.x, pos.y, size.x, size.y, escape(&w.props.text), escape(&w.props.url)
                        ));
                    } else {
                        out.push_str(&format!(
                            "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                             \x20       let resp = ui.hyperlink_to(\"{}\", \"{}\");\n",
                            pos.x, pos.y, size.x, size.y, escape(&w.props.text), escape(&w.props.url)
                        ));
                        emit_actions(&w.props.actions, out, "resp");
                        out.push_str("    });\n");
                    }
                }
                WidgetKind::SelectableLabel => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                         \x20       let resp = ui.selectable_label(state.sel_{}, \"{}\");\n\
                         \x20       if resp.clicked() {{ state.sel_{} = !state.sel_{}; }}\n",
                        pos.x, pos.y, size.x, size.y, w.id, escape(&w.props.text), w.id, w.id
                    ));
                    if !w.props.actions.is_empty() {
                        emit_actions(&w.props.actions, out, "resp");
                    }
                    out.push_str("    });\n");
                }
                WidgetKind::ComboBox => {
                    let items_code = if w.props.items.is_empty() {
                        "\"Item\".to_string()".to_owned()
                    } else {
                        w.props
                            .items
                            .iter()
                            .map(|s| format!("\"{}\".to_string()", escape(s)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };

                    out.push_str(&format!(
						"    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{\n",
						x = pos.x, y = pos.y, w = size.x, h = size.y
					));
                    out.push_str(&format!(
                        "        let items = vec![{items}];\n",
                        items = items_code
                    ));
                    out.push_str(&format!(
                        "        egui::ComboBox::from_id_source({id})\n",
                        id = w.id
                    ));
                    out.push_str(&format!("            .width({:.1})\n", size.x));
                    out.push_str(&format!(
						"            .selected_text(items.get(state.sel_{id}).cloned().unwrap_or_else(|| \"\".to_string()))\n",
						id = w.id
					));
                    out.push_str("            .show_ui(ui, |ui| {\n");
                    out.push_str(&format!(
						"                for (i, it) in items.iter().enumerate() {{ ui.selectable_value(&mut state.sel_{id}, i, it.clone()); }}\n",
						id = w.id
					));
                    out.push_str("            });\n");
                    out.push_str("    });\n");
                }
                WidgetKind::Separator => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.separator(); }});\n",
                        pos.x, pos.y, size.x, size.y
                    ));
                }
                WidgetKind::CollapsingHeader => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{\n\
                            egui::CollapsingHeader::new(\"{}\").id_salt((\"collapsing_header\", {})).default_open(state.open_{}).show(ui, |ui| {{\n",
                        pos.x, pos.y, size.x, size.y, escape(&w.props.text), w.id, w.id
                    ));
                    let children = ctx.project.children_of(w.id);
                    if children.is_empty() {
                        out.push_str("                ui.label(\"… place your inner content here …\");\n");
                    } else if w.props.layout_mode == LayoutMode::Free {
                        for child in children {
                            emit_widget(ctx, child, out, "ui.min_rect().min");
                        }
                    } else {
                        match w.props.layout_mode {
                            LayoutMode::Free => unreachable!(),
                            LayoutMode::Row => {
                                let justify_str = match w.props.layout_justify {
                                    Justify::Start => "egui::Align::Min",
                                    Justify::Center => "egui::Align::Center",
                                    Justify::End => "egui::Align::Max",
                                    _ => "egui::Align::Min",
                                };
                                if w.props.responsive_layout == crate::widget::ResponsiveLayout::RowToColumnOnPortrait {
                                    out.push_str("                    if ui.available_height() > ui.available_width() {\n");
                                    out.push_str("                        ui.vertical(|ui| {\n");
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                            ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                        });\n");
                                    out.push_str("                    } else {\n");
                                    out.push_str(&format!("                        ui.with_layout(egui::Layout::left_to_right({justify_str}), |ui| {{\n"));
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                            ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                        });\n");
                                    out.push_str("                    }\n");
                                } else {
                                    out.push_str(&format!("                    ui.with_layout(egui::Layout::left_to_right({justify_str}), |ui| {{\n"));
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                        ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                    });\n");
                                }
                            }
                            LayoutMode::Column => {
                                out.push_str("                    ui.vertical(|ui| {\n");
                                for (i, child) in children.iter().enumerate() {
                                    if i > 0 && w.props.layout_gap > 0.0 {
                                        out.push_str(&format!("                        ui.add_space({:.1});\n", w.props.layout_gap));
                                    }
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                }
                                out.push_str("                    });\n");
                            }
                            LayoutMode::WrapRow => {
                                out.push_str("                    ui.horizontal_wrapped(|ui| {\n");
                                for (i, child) in children.iter().enumerate() {
                                    if i > 0 && w.props.layout_gap > 0.0 {
                                        out.push_str(&format!("                        ui.add_space({:.1});\n", w.props.layout_gap));
                                    }
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                }
                                out.push_str("                    });\n");
                            }
                            LayoutMode::Grid => {
                                out.push_str(&format!(
                                    "                    egui::Grid::new(\"grid_{id}\").num_columns({cols}).spacing([{gap:.1}, {gap:.1}]).show(ui, |ui| {{\n",
                                    id = w.id,
                                    cols = w.props.layout_cols.max(1),
                                    gap = w.props.layout_gap
                                ));
                                for (i, child) in children.iter().enumerate() {
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                    if (i + 1) % w.props.layout_cols.max(1) == 0 {
                                        out.push_str("                        ui.end_row();\n");
                                    }
                                }
                                if children.len() % w.props.layout_cols.max(1) != 0 {
                                    out.push_str("                        ui.end_row();\n");
                                }
                                out.push_str("                    });\n");
                            }
                        }
                    }
                    out.push_str("            });\n    });\n");
                }
                WidgetKind::DatePicker => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size({origin} + egui::vec2({:.1},{:.1}), egui::vec2({:.1},{:.1}))), |ui| {{ ui.horizontal(|ui| {{ ui.label(\"{}\"); let date_picker_id = format!(\"date_picker_{}\", {}); ui.add(DatePickerButton::new(&mut state.date_{}).id_salt(&date_picker_id)); }}); }});\n",
                        pos.x, pos.y, size.x, size.y, escape(&w.props.text), w.id, w.id, w.id
                    ));
                }
                WidgetKind::Password => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
							{origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
							ui.add_sized(egui::vec2({w:.1},{h:.1}), \
								egui::TextEdit::singleline(&mut state.pass_{id}).password(true).hint_text(\"password\") \
							); \
						}});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                    ));
                }
                WidgetKind::AngleSelector => {
                    out.push_str(&format!(
						"    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
							{origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
							ui.add_sized(egui::vec2({w:.1},{h:.1}), \
								egui::Slider::new(&mut state.angle_{id}, {min:.3}..={max:.3}).suffix(\"°\").text(\"{label}\") \
							); \
						}});\n",
						x=pos.x,y=pos.y,w=size.x,h=size.y,id=w.id,
						min=w.props.min, max=w.props.max, label=escape(&w.props.text)
					));
                }
                WidgetKind::Tree => {
                    // Helpers live only in the generator (not emitted), so we can use any Rust we want here:
                    #[derive(Clone)]
                    struct Node {
                        label: String,
                        children: Vec<Node>,
                    }

                    fn parse_nodes(lines: &[String]) -> Vec<Node> {
                        let items: Vec<(usize, String)> = lines
                            .iter()
                            .map(|s| {
                                let indent = s.chars().take_while(|c| *c == ' ').count() / 2;
                                (indent, s.trim().to_string())
                            })
                            .filter(|(_, s)| !s.is_empty())
                            .collect();

                        fn build<I: Iterator<Item = (usize, String)>>(
                            it: &mut std::iter::Peekable<I>,
                            level: usize,
                        ) -> Vec<Node> {
                            let mut out = Vec::new();
                            while let Some((ind, _)) = it.peek().cloned() {
                                if ind < level {
                                    break;
                                }
                                if ind > level {
                                    break;
                                }
                                let (_, label) = it.next().unwrap();
                                let children = build(it, level + 1);
                                out.push(Node { label, children });
                            }
                            out
                        }

                        let mut it = items.into_iter().peekable();
                        build(&mut it, 0)
                    }

                    fn nodes_to_literal(nodes: &[Node]) -> String {
                        fn one(n: &Node) -> String {
                            let kids = if n.children.is_empty() {
                                "vec![]".to_string()
                            } else {
                                format!(
                                    "vec![{}]",
                                    n.children.iter().map(one).collect::<Vec<_>>().join(", ")
                                )
                            };
                            format!(
                                "GenTreeNode {{ label: \"{}\".to_string(), children: {} }}",
                                crate::widget::escape(&n.label),
                                kids
                            )
                        }
                        format!(
                            "vec![{}]",
                            nodes.iter().map(one).collect::<Vec<_>>().join(", ")
                        )
                    }

                    let items = if w.props.items.is_empty() {
                        vec!["Root".into(), "  Child".into()]
                    } else {
                        w.props.items.clone()
                    };

                    let nodes_literal = {
                        let nodes = parse_nodes(&items);
                        nodes_to_literal(&nodes)
                    };

                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
							{origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
							let nodes: Vec<GenTreeNode> = {nodes}; \
							egui::ScrollArea::vertical().id_salt((\"tree_scroll\", {id})).auto_shrink([false,false]).show(ui, |ui| {{ \
								let mut path = Vec::new(); \
								gen_show_tree(ui, &nodes, &mut path); \
							}}); \
						}});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                        nodes = nodes_literal,
                    ));
                }
                WidgetKind::TextArea => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            ui.add_sized(egui::vec2({w:.1},{h:.1}), \
                                egui::TextEdit::multiline(&mut state.textarea_{id}).desired_rows(5) \
                            ); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                    ));
                }
                WidgetKind::DragValue => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            ui.horizontal(|ui| {{ \
                                ui.label(\"{label}\"); \
                                ui.add(egui::DragValue::new(&mut state.drag_{id}).range({min:.3}..={max:.3})); \
                            }}); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                        label = escape(&w.props.text),
                        min = w.props.min,
                        max = w.props.max,
                    ));
                }
                WidgetKind::Spinner => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            ui.add(egui::Spinner::new()); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                    ));
                }
                WidgetKind::ColorPicker => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            ui.horizontal(|ui| {{ \
                                ui.label(\"{label}\"); \
                                egui::color_picker::color_edit_button_srgba(ui, &mut state.color_{id}, egui::color_picker::Alpha::OnlyBlend); \
                            }}); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                        label = escape(&w.props.text),
                    ));
                }
                WidgetKind::Code => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            egui::ScrollArea::vertical().id_salt((\"code_scroll\", {id})).auto_shrink([false,false]).show(ui, |ui| {{ \
                                ui.add(egui::TextEdit::multiline(&mut state.code_{id}).code_editor().desired_width({w:.1}).desired_rows(8)); \
                            }}); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                    ));
                }
                WidgetKind::Heading => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            ui.heading(\"{text}\"); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        text = escape(&w.props.text),
                    ));
                }
                WidgetKind::Image => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            ui.add(egui::Image::new(\"{uri}\").fit_to_exact_size(egui::vec2({w:.1},{h:.1}))); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        uri = escape(&w.props.url),
                    ));
                }
                WidgetKind::Placeholder => {
                    let c = w.props.color;
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{ \
                            egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied({r},{g},{b},{a})).corner_radius(4.0).show(ui, |ui| {{ \
                                ui.set_min_size(egui::vec2({w:.1},{h:.1})); \
                                ui.centered_and_justified(|ui| ui.label(\"{text}\")); \
                            }}); \
                        }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        r = c[0], g = c[1], b = c[2], a = c[3],
                        text = escape(&w.props.text),
                    ));
                }
                WidgetKind::Group | WidgetKind::Container => {
                    let title_code = if w.kind == WidgetKind::Container || w.props.text.is_empty() {
                        String::new()
                    } else {
                        format!("ui.strong(\"{}\"); ui.separator(); ", escape(&w.props.text))
                    };
                    let frame_str = if w.kind == WidgetKind::Container {
                        "egui::Frame::NONE"
                    } else {
                        "egui::Frame::group(ui.style())"
                    };
                    let layout_fn = if w.props.horizontal { "horizontal" } else { "vertical" };
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{\n\
                            {frame}.show(ui, |ui| {{\n\
                                ui.set_min_size(egui::vec2({iw:.1},{ih:.1}));\n\
                                ui.{layout_fn}(|ui| {{\n\
                                    {title}\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        iw = (size.x - 12.0).max(10.0),
                        ih = (size.y - 12.0).max(10.0),
                        frame = frame_str,
                        title = title_code,
                        layout_fn = layout_fn,
                    ));
                    let children = ctx.project.children_of(w.id);
                    if children.is_empty() {
                        out.push_str("                    /* group contents */\n");
                    } else if w.props.layout_mode == LayoutMode::Free {
                        for child in children {
                            emit_widget(ctx, child, out, "ui.min_rect().min");
                        }
                    } else {
                        match w.props.layout_mode {
                            LayoutMode::Free => unreachable!(),
                            LayoutMode::Row => {
                                let justify_str = match w.props.layout_justify {
                                    Justify::Start => "egui::Align::Min",
                                    Justify::Center => "egui::Align::Center",
                                    Justify::End => "egui::Align::Max",
                                    _ => "egui::Align::Min",
                                };
                                if w.props.responsive_layout == crate::widget::ResponsiveLayout::RowToColumnOnPortrait {
                                    out.push_str("                    if ui.available_height() > ui.available_width() {\n");
                                    out.push_str("                        ui.vertical(|ui| {\n");
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                            ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                        });\n");
                                    out.push_str("                    } else {\n");
                                    out.push_str(&format!("                        ui.with_layout(egui::Layout::left_to_right({justify_str}), |ui| {{\n"));
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                            ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                        });\n");
                                    out.push_str("                    }\n");
                                } else {
                                    out.push_str(&format!("                    ui.with_layout(egui::Layout::left_to_right({justify_str}), |ui| {{\n"));
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                        ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                    });\n");
                                }
                            }
                            LayoutMode::Column => {
                                out.push_str("                    ui.vertical(|ui| {\n");
                                for (i, child) in children.iter().enumerate() {
                                    if i > 0 && w.props.layout_gap > 0.0 {
                                        out.push_str(&format!("                        ui.add_space({:.1});\n", w.props.layout_gap));
                                    }
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                }
                                out.push_str("                    });\n");
                            }
                            LayoutMode::WrapRow => {
                                out.push_str("                    ui.horizontal_wrapped(|ui| {\n");
                                for (i, child) in children.iter().enumerate() {
                                    if i > 0 && w.props.layout_gap > 0.0 {
                                        out.push_str(&format!("                        ui.add_space({:.1});\n", w.props.layout_gap));
                                    }
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                }
                                out.push_str("                    });\n");
                            }
                            LayoutMode::Grid => {
                                out.push_str(&format!(
                                    "                    egui::Grid::new(\"grid_{id}\").num_columns({cols}).spacing([{gap:.1}, {gap:.1}]).show(ui, |ui| {{\n",
                                    id = w.id,
                                    cols = w.props.layout_cols.max(1),
                                    gap = w.props.layout_gap
                                ));
                                for (i, child) in children.iter().enumerate() {
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                    if (i + 1) % w.props.layout_cols.max(1) == 0 {
                                        out.push_str("                        ui.end_row();\n");
                                    }
                                }
                                if children.len() % w.props.layout_cols.max(1) != 0 {
                                    out.push_str("                        ui.end_row();\n");
                                }
                                out.push_str("                    });\n");
                            }
                        }
                    }
                    out.push_str("                });\n            });\n    });\n");
                }
                WidgetKind::ScrollBox => {
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{\n\
                            egui::ScrollArea::both().id_salt((\"scroll_box\", {id})).max_width({sw:.1}).max_height({sh:.1}).auto_shrink([false,false]).show(ui, |ui| {{\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                        sw = (size.x - 4.0).max(10.0),
                        sh = (size.y - 4.0).max(10.0),
                    ));
                    let children = ctx.project.children_of(w.id);
                    if children.is_empty() {
                        if !w.props.text.is_empty() {
                            out.push_str(&format!("                ui.label(\"{}\");\n", escape(&w.props.text)));
                        } else {
                            out.push_str("                /* scroll area contents */\n");
                        }
                    } else if w.props.layout_mode == LayoutMode::Free {
                        for child in children {
                            emit_widget(ctx, child, out, "ui.min_rect().min");
                        }
                    } else {
                        match w.props.layout_mode {
                            LayoutMode::Free => unreachable!(),
                            LayoutMode::Row => {
                                if w.props.responsive_layout == crate::widget::ResponsiveLayout::RowToColumnOnPortrait {
                                    out.push_str("                    if ui.available_height() > ui.available_width() {\n");
                                    out.push_str("                        ui.vertical(|ui| {\n");
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                            ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                        });\n");
                                    out.push_str("                    } else {\n");
                                    out.push_str("                        ui.horizontal(|ui| {\n");
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                            ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                        });\n");
                                    out.push_str("                    }\n");
                                } else {
                                    out.push_str("                ui.horizontal(|ui| {\n");
                                    for (i, child) in children.iter().enumerate() {
                                        if i > 0 && w.props.layout_gap > 0.0 {
                                            out.push_str(&format!("                    ui.add_space({:.1});\n", w.props.layout_gap));
                                        }
                                        emit_widget(ctx, child, out, "ui.min_rect().min");
                                    }
                                    out.push_str("                });\n");
                                }
                            }
                            LayoutMode::Column => {
                                out.push_str("                ui.vertical(|ui| {\n");
                                for (i, child) in children.iter().enumerate() {
                                    if i > 0 && w.props.layout_gap > 0.0 {
                                        out.push_str(&format!("                    ui.add_space({:.1});\n", w.props.layout_gap));
                                    }
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                }
                                out.push_str("                });\n");
                            }
                            LayoutMode::WrapRow => {
                                out.push_str("                ui.horizontal_wrapped(|ui| {\n");
                                for (i, child) in children.iter().enumerate() {
                                    if i > 0 && w.props.layout_gap > 0.0 {
                                        out.push_str(&format!("                    ui.add_space({:.1});\n", w.props.layout_gap));
                                    }
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                }
                                out.push_str("                });\n");
                            }
                            LayoutMode::Grid => {
                                out.push_str(&format!(
                                    "                egui::Grid::new(\"grid_{id}\").num_columns({cols}).spacing([{gap:.1}, {gap:.1}]).show(ui, |ui| {{\n",
                                    id = w.id,
                                    cols = w.props.layout_cols.max(1),
                                    gap = w.props.layout_gap
                                ));
                                for (i, child) in children.iter().enumerate() {
                                    emit_widget(ctx, child, out, "ui.min_rect().min");
                                    if (i + 1) % w.props.layout_cols.max(1) == 0 {
                                        out.push_str("                    ui.end_row();\n");
                                    }
                                }
                                if children.len() % w.props.layout_cols.max(1) != 0 {
                                    out.push_str("                    ui.end_row();\n");
                                }
                                out.push_str("                });\n");
                            }
                        }
                    }
                    out.push_str("            });\n    });\n");
                }
                WidgetKind::TabBar => {
                    let tabs_code: String = w.props.items.iter().enumerate().map(|(i, tab)| {
                        format!("ui.selectable_value(&mut state.tab_{id}, {i}, \"{tab}\"); ",
                            id = w.id, i = i, tab = escape(tab))
                    }).collect();
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{\n\
                            ui.horizontal(|ui| {{ {tabs} }});\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        tabs = tabs_code,
                    ));
                    let children = ctx.project.children_of(w.id);
                    for child in &children {
                        if child.props.tab_page.is_none() {
                            emit_widget(ctx, child, out, "ui.min_rect().min");
                        }
                    }
                    for (i, _) in w.props.items.iter().enumerate() {
                        let page_children: Vec<&Widget> = children.iter().filter(|c| c.props.tab_page == Some(i)).copied().collect();
                        if !page_children.is_empty() {
                            out.push_str(&format!("        if state.tab_{} == {} {{\n", w.id, i));
                            for child in page_children {
                                emit_widget(ctx, child, out, "ui.min_rect().min");
                            }
                            out.push_str("        }\n");
                        }
                    }
                    out.push_str("    });\n");
                }
                WidgetKind::Columns => {
                    let cols = w.props.columns.max(1);
                    out.push_str(&format!(
                        "    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(\
                            {origin} + egui::vec2({x:.1},{y:.1}), egui::vec2({w:.1},{h:.1}))), |ui| {{\n\
                            ui.columns({cols}, |columns| {{\n",
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        cols = cols,
                    ));
                    let children = ctx.project.children_of(w.id);
                    if children.is_empty() {
                        out.push_str(&format!(
                            "                for col in columns.iter_mut() {{ col.label(\"{}\"); }}\n",
                            escape(&w.props.text)
                        ));
                    } else {
                        for (i, child) in children.into_iter().enumerate() {
                            let col_idx = i % (cols as usize);
                            out.push_str(&format!("                let ui = &mut columns[{}];\n", col_idx));
                            emit_widget(ctx, child, out, "ui.min_rect().min");
                        }
                    }
                    out.push_str("            });\n    });\n");
                }
                WidgetKind::Window => {
                    let title = escape(&w.props.text);
                    out.push_str(&format!(
                        "    egui::Window::new(\"{title}\").default_pos({origin} + egui::vec2({x:.1},{y:.1})).default_size(egui::vec2({w:.1},{h:.1})).open(&mut state.window_{id}_open).show(ctx, |ui| {{\n",
                        title = title,
                        x = pos.x,
                        y = pos.y,
                        w = size.x,
                        h = size.y,
                        id = w.id,
                    ));
                    let children = ctx.project.children_of(w.id);
                    if children.is_empty() {
                        out.push_str("        /* window contents */\n");
                    } else {
                        for child in children {
                            emit_widget(ctx, child, out, "ui.min_rect().min");
                        }
                    }
                    out.push_str("    });\n");
                }
            }
            out.push_str("    });\n");

            if !w.props.tooltip.is_empty() {
                out.push_str(&format!(
                    "    // tooltip\n    // .on_hover_text(\"{}\")\n",
                    escape(&w.props.tooltip)
                ));
            }

            if needs_opacity {
                out.push_str("    });\n");
            }

            if needs_vis {
                out.push_str("    }\n");
            }

            if responsive_cond.is_some() {
                out.push_str("    }\n");
            }
        }

        let emit_ctx = EmitCtx {
            project: &self.project,
            visibility_toggled_ids: &visibility_toggled_ids,
            text_updated_ids: &text_updated_ids,
        };

        let mut top = Vec::new();
        let mut bottom = Vec::new();
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut center = Vec::new();
        let mut free = Vec::new();
        for w in &self.project.widgets {
            if w.parent.is_none() {
                match w.area {
                    Top => top.push(w),
                    Bottom => bottom.push(w),
                    Left => left.push(w),
                    Right => right.push(w),
                    Center => center.push(w),
                    Free => free.push(w),
                }
            }
        }

        out.push_str("fn generated_ui(ctx: &egui::Context, state: &mut GeneratedState) {\n");

        // TOP
        out.push_str("    if state.enable_top {\n");
        out.push_str("        egui::TopBottomPanel::top(\"gen_top\")\n");
        out.push_str("            .resizable(true)\n");
        out.push_str("            .show(ctx, |ui| {\n");
        for w in top {
            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
        }
        out.push_str("            });\n");
        out.push_str("    }\n");

        // BOTTOM
        out.push_str("    if state.enable_bottom {\n");
        out.push_str("        egui::TopBottomPanel::bottom(\"gen_bottom\")\n");
        out.push_str("            .resizable(true)\n");
        out.push_str("            .show(ctx, |ui| {\n");
        for w in bottom {
            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
        }
        out.push_str("            });\n");
        out.push_str("    }\n");

        // LEFT
        out.push_str("    if state.enable_left {\n");
        out.push_str("        egui::SidePanel::left(\"gen_left\")\n");
        out.push_str("            .resizable(true)\n");
        out.push_str("            .show(ctx, |ui| {\n");
        for w in left {
            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
        }
        out.push_str("            });\n");
        out.push_str("    }\n");

        // RIGHT
        out.push_str("    if state.enable_right {\n");
        out.push_str("        egui::SidePanel::right(\"gen_right\")\n");
        out.push_str("            .resizable(true)\n");
        out.push_str("            .show(ctx, |ui| {\n");
        for w in right {
            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
        }
        out.push_str("            });\n");
        out.push_str("    }\n");

        // CENTER (+ FREE): use CentralPanel; widgets are placed within it.
        out.push_str("    egui::CentralPanel::default().show(ctx, |ui| {\n");
        out.push_str("        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {\n");

        if self.project.root_layout_mode == LayoutMode::Free {
            out.push_str(&format!(
                "            let canvas = egui::Rect::from_min_size(ui.min_rect().min, egui::vec2({:.1}, {:.1}));\n",
                self.project.canvas_size.x, self.project.canvas_size.y
            ));
            out.push_str("            let _ = ui.allocate_painter(canvas.size(), egui::Sense::hover());\n");
            for w in center {
                emit_widget(&emit_ctx, w, &mut out, "canvas.min");
            }
            for w in free {
                emit_widget(&emit_ctx, w, &mut out, "canvas.min");
            }
        } else {
            let mut all_center_roots = center;
            all_center_roots.extend(free);
            all_center_roots.sort_by_key(|w| w.z);

            let p_top = self.project.root_layout_padding[0];
            let p_right = self.project.root_layout_padding[1];
            let p_bottom = self.project.root_layout_padding[2];
            let p_left = self.project.root_layout_padding[3];
            let has_padding = p_top > 0.0 || p_right > 0.0 || p_bottom > 0.0 || p_left > 0.0;

            let indent = if has_padding { "                " } else { "            " };

            if has_padding {
                out.push_str(&format!(
                    "            egui::Frame::NONE.inner_margin(egui::Margin {{ top: {:.1}, right: {:.1}, bottom: {:.1}, left: {:.1} }}).show(ui, |ui| {{\n",
                    p_top, p_right, p_bottom, p_left
                ));
            }

            match self.project.root_layout_mode {
                LayoutMode::Free => unreachable!(),
                LayoutMode::Column => {
                    out.push_str(&format!("{indent}ui.vertical(|ui| {{\n"));
                    for (i, w) in all_center_roots.iter().enumerate() {
                        if i > 0 && self.project.root_layout_gap > 0.0 {
                            out.push_str(&format!("{indent}    ui.add_space({:.1});\n", self.project.root_layout_gap));
                        }
                        emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
                    }
                    out.push_str(&format!("{indent}}});\n"));
                }
                LayoutMode::Row => {
                    let justify_str = match self.project.root_layout_justify {
                        Justify::Start => "egui::Align::Min",
                        Justify::Center => "egui::Align::Center",
                        Justify::End => "egui::Align::Max",
                        _ => "egui::Align::Min",
                    };
                    if self.project.root_responsive_layout == crate::widget::ResponsiveLayout::RowToColumnOnPortrait {
                        out.push_str(&format!("{indent}if ui.available_height() > ui.available_width() {{\n"));
                        out.push_str(&format!("{indent}    ui.vertical(|ui| {{\n"));
                        for (i, w) in all_center_roots.iter().enumerate() {
                            if i > 0 && self.project.root_layout_gap > 0.0 {
                                out.push_str(&format!("{indent}        ui.add_space({:.1});\n", self.project.root_layout_gap));
                            }
                            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
                        }
                        out.push_str(&format!("{indent}    }});\n"));
                        out.push_str(&format!("{indent}}} else {{\n"));
                        out.push_str(&format!("{indent}    ui.with_layout(egui::Layout::left_to_right({justify_str}), |ui| {{\n"));
                        for (i, w) in all_center_roots.iter().enumerate() {
                            if i > 0 && self.project.root_layout_gap > 0.0 {
                                out.push_str(&format!("{indent}        ui.add_space({:.1});\n", self.project.root_layout_gap));
                            }
                            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
                        }
                        out.push_str(&format!("{indent}    }});\n"));
                        out.push_str(&format!("{indent}}}\n"));
                    } else {
                        out.push_str(&format!("{indent}ui.with_layout(egui::Layout::left_to_right({justify_str}), |ui| {{\n"));
                        for (i, w) in all_center_roots.iter().enumerate() {
                            if i > 0 && self.project.root_layout_gap > 0.0 {
                                out.push_str(&format!("{indent}    ui.add_space({:.1});\n", self.project.root_layout_gap));
                            }
                            emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
                        }
                        out.push_str(&format!("{indent}}});\n"));
                    }
                }
                LayoutMode::WrapRow => {
                    out.push_str(&format!("{indent}ui.horizontal_wrapped(|ui| {{\n"));
                    for (i, w) in all_center_roots.iter().enumerate() {
                        if i > 0 && self.project.root_layout_gap > 0.0 {
                            out.push_str(&format!("{indent}    ui.add_space({:.1});\n", self.project.root_layout_gap));
                        }
                        emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
                    }
                    out.push_str(&format!("{indent}}});\n"));
                }
                LayoutMode::Grid => {
                    let cols = self.project.root_layout_cols.max(1);
                    out.push_str(&format!("{indent}egui::Grid::new(\"root_screen_grid\").spacing([{:?}, {:?}]).show(ui, |ui| {{\n", self.project.root_layout_gap, self.project.root_layout_gap));
                    for (i, w) in all_center_roots.iter().enumerate() {
                        emit_widget(&emit_ctx, w, &mut out, "ui.min_rect().min");
                        if (i + 1) % cols == 0 {
                            out.push_str(&format!("{indent}    ui.end_row();\n"));
                        }
                    }
                    if all_center_roots.len() % cols != 0 {
                        out.push_str(&format!("{indent}    ui.end_row();\n"));
                    }
                    out.push_str(&format!("{indent}}});\n"));
                }
            }

            if has_padding {
                out.push_str("            });\n");
            }
        }

        out.push_str("        });\n");
        out.push_str("    });\n");

        out.push_str("}\n\n");

        // ---------- Example eframe app (updated to call generated_ui with ctx) ----------
        if self.codegen_comments {
            out.push_str("// =============================================================================\n");
            out.push_str("// Application entry point\n");
            out.push_str("// =============================================================================\n\n");
        }

        out.push_str(
            "pub struct GeneratedApp {\n\
			     state: GeneratedState,\n\
			 }\n\n\
			 impl Default for GeneratedApp {\n\
			     fn default() -> Self {\n\
			         Self { state: Default::default() }\n\
			     }\n\
			 }\n\n\
			 impl eframe::App for GeneratedApp {\n\
			     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {\n\
			         generated_ui(ctx, &mut self.state);\n\
			     }\n\
			 }\n\n\
			 fn main() -> eframe::Result<()> {\n\
			     let native_options = eframe::NativeOptions::default();\n\
			     eframe::run_native(\n\
			         \"Generated UI\",\n\
			         native_options,\n\
			         Box::new(|_cc| Ok(Box::new(GeneratedApp::default()))),\n\
			     )\n\
			 }\n",
        );

        out
    }

    /// Generate code split into separate conceptual files (shown with file headers)
    fn generate_separate_files(&self) -> String {
        let single = self.generate_single_file();

        // For now, show the code with clear section headers
        // A future enhancement could actually save separate files
        let mut out = String::new();

        out.push_str(
            "// =============================================================================\n",
        );
        out.push_str("// FILE: Cargo.toml\n");
        out.push_str(
            "// =============================================================================\n",
        );
        out.push_str("[package]\n");
        out.push_str("name = \"generated-ui\"\n");
        out.push_str("version = \"0.1.0\"\n");
        out.push_str("edition = \"2021\"\n\n");
        out.push_str("[dependencies]\n");
        out.push_str("eframe = \"0.33\"\n");
        out.push_str("egui = \"0.33\"\n");
        out.push_str("egui_extras = { version = \"0.33\", features = [\"chrono\"] }\n");
        out.push_str("chrono = \"0.4\"\n\n");

        out.push_str(
            "// =============================================================================\n",
        );
        out.push_str("// FILE: src/main.rs\n");
        out.push_str(
            "// =============================================================================\n",
        );
        out.push_str(&single);

        out
    }

    /// Generate only the UI function (for embedding in existing code)
    fn generate_ui_only(&self) -> String {
        let mut out = String::new();

        if self.codegen_comments {
            out.push_str("// UI function generated by egui RAD GUI Builder\n");
            out.push_str("// Embed this in your existing application\n\n");
        }

        // We need to include the state struct since UI references it
        out.push_str("// Required state struct for the UI\n");

        // Generate just the state struct and the UI function
        // We'll extract parts from generate_single_file
        let has_tree = self
            .project
            .widgets
            .iter()
            .any(|w| matches!(w.kind, WidgetKind::Tree));
        if has_tree {
            out.push_str(
                "#[derive(Clone)]\n\
                 struct GenTreeNode { label: String, children: Vec<GenTreeNode> }\n\
                 \n\
                 fn gen_show_tree(ui: &mut egui::Ui, nodes: &[GenTreeNode], path: &mut Vec<usize>) {\n\
                     for (idx, n) in nodes.iter().enumerate() {\n\
                         if n.children.is_empty() { ui.label(&n.label); }\n\
                         else {\n\
                             path.push(idx);\n\
                             egui::CollapsingHeader::new(&n.label).id_salt((\"tree_node\", path.clone())).show(ui, |ui| gen_show_tree(ui, &n.children, path));\n\
                             path.pop();\n\
                         }\n\
                     }\n\
                 }\n\n",
            );
        }

        out.push_str("struct GeneratedState {\n");
        out.push_str(
            "    enable_top: bool, enable_bottom: bool, enable_left: bool, enable_right: bool,\n",
        );
        for w in &self.project.widgets {
            match w.kind {
                WidgetKind::TextEdit => out.push_str(&format!("    text_{}: String,\n", w.id)),
                WidgetKind::Checkbox => out.push_str(&format!("    checked_{}: bool,\n", w.id)),
                WidgetKind::Slider => out.push_str(&format!("    value_{}: f32,\n", w.id)),
                WidgetKind::ProgressBar => out.push_str(&format!("    progress_{}: f32,\n", w.id)),
                WidgetKind::SelectableLabel => out.push_str(&format!("    sel_{}: bool,\n", w.id)),
                WidgetKind::RadioGroup | WidgetKind::ComboBox | WidgetKind::MenuButton => {
                    out.push_str(&format!("    sel_{}: usize,\n", w.id))
                }
                WidgetKind::CollapsingHeader => {
                    out.push_str(&format!("    open_{}: bool,\n", w.id))
                }
                WidgetKind::DatePicker => {
                    out.push_str(&format!("    date_{}: chrono::NaiveDate,\n", w.id))
                }
                WidgetKind::Password => out.push_str(&format!("    pass_{}: String,\n", w.id)),
                WidgetKind::AngleSelector => out.push_str(&format!("    angle_{}: f32,\n", w.id)),
                WidgetKind::TextArea => out.push_str(&format!("    textarea_{}: String,\n", w.id)),
                WidgetKind::DragValue => out.push_str(&format!("    drag_{}: f32,\n", w.id)),
                WidgetKind::ColorPicker => {
                    out.push_str(&format!("    color_{}: egui::Color32,\n", w.id))
                }
                WidgetKind::Code => out.push_str(&format!("    code_{}: String,\n", w.id)),
                _ => {}
            }
        }
        out.push_str("}\n\n");

        out.push_str("// Call this function from your eframe::App::update method:\n");
        out.push_str("// generated_ui(ctx, &mut self.state);\n\n");

        // Extract just the generated_ui function from single file output
        let single = self.generate_single_file();
        if let Some(start) = single.find("fn generated_ui(") {
            // Find the end of the function (look for the closing brace followed by app struct)
            if let Some(end) = single[start..].find("\npub struct GeneratedApp") {
                out.push_str(&single[start..start + end]);
            } else {
                // Fallback: include from generated_ui to end
                out.push_str(&single[start..]);
            }
        }

        out
    }
}

impl eframe::App for RadBuilderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let snapshot_before_frame = self.current_snapshot();

        // Keyboard shortcuts - check input first, then apply changes
        let wants_kb = ctx.wants_keyboard_input();
        let (
            undo_pressed,
            redo_pressed,
            delete_pressed,
            duplicate_pressed,
            generate_pressed,
            copy_pressed,
            paste_pressed,
            arrow_up,
            arrow_down,
            arrow_left,
            arrow_right,
            bring_front,
            send_back,
            toggle_preview,
        ) = ctx.input(|i| {
            let undo = !wants_kb
                && i.modifiers.command
                && !i.modifiers.shift
                && i.key_pressed(egui::Key::Z);
            let redo = !wants_kb
                && ((i.modifiers.command && i.key_pressed(egui::Key::Y))
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z)));
            let del = !wants_kb
                && (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
            let dup = !wants_kb && i.modifiers.command && i.key_pressed(egui::Key::D);
            let gencode = !wants_kb && i.modifiers.command && i.key_pressed(egui::Key::G);
            let copy = !wants_kb && i.modifiers.command && i.key_pressed(egui::Key::C);
            let paste = !wants_kb && i.modifiers.command && i.key_pressed(egui::Key::V);
            // Arrow keys for nudging
            let up = !wants_kb && i.key_pressed(egui::Key::ArrowUp);
            let down = !wants_kb && i.key_pressed(egui::Key::ArrowDown);
            let left = !wants_kb && i.key_pressed(egui::Key::ArrowLeft);
            let right = !wants_kb && i.key_pressed(egui::Key::ArrowRight);
            // Z-order: ] = bring to front, [ = send to back
            let front = !wants_kb && i.key_pressed(egui::Key::CloseBracket);
            let back = !wants_kb && i.key_pressed(egui::Key::OpenBracket);
            // F5: Toggle preview mode
            let preview = i.key_pressed(egui::Key::F5);
            (
                undo, redo, del, dup, gencode, copy, paste, up, down, left, right, front, back,
                preview,
            )
        });

        // Undo / Redo
        if undo_pressed {
            self.undo();
        }
        if redo_pressed {
            self.redo();
        }

        // F5: Toggle preview mode
        if toggle_preview {
            self.preview_mode = !self.preview_mode;
        }

        // Delete selected widgets
        if delete_pressed && !self.selected.is_empty() {
            self.delete_selected();
        }

        // Arrow keys: Nudge all selected widgets
        if !self.selected.is_empty() && (arrow_up || arrow_down || arrow_left || arrow_right) {
            self.push_undo();
            let nudge = self.grid_size.max(1.0);
            let selected_ids: Vec<_> = self.selected.clone();
            for sel_id in selected_ids {
                if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == sel_id) {
                    if arrow_up {
                        w.pos.y -= nudge;
                    }
                    if arrow_down {
                        w.pos.y += nudge;
                    }
                    if arrow_left {
                        w.pos.x -= nudge;
                    }
                    if arrow_right {
                        w.pos.x += nudge;
                    }
                    // Clamp position
                    w.pos.x = w.pos.x.max(0.0);
                    w.pos.y = w.pos.y.max(0.0);
                }
            }
        }

        // Z-order controls (apply to all selected)
        if bring_front && !self.selected.is_empty() {
            self.push_undo();
            let max_z = self.project.widgets.iter().map(|w| w.z).max().unwrap_or(0);
            let selected_ids: Vec<_> = self.selected.clone();
            for (i, sel_id) in selected_ids.iter().enumerate() {
                if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *sel_id) {
                    w.z = max_z + 1 + i as i32;
                }
            }
        }
        if send_back && !self.selected.is_empty() {
            self.push_undo();
            let min_z = self.project.widgets.iter().map(|w| w.z).min().unwrap_or(0);
            let selected_ids: Vec<_> = self.selected.clone();
            for (i, sel_id) in selected_ids.iter().enumerate() {
                if let Some(w) = self.project.widgets.iter_mut().find(|w| w.id == *sel_id) {
                    w.z = min_z - 1 - i as i32;
                }
            }
        }

        // Ctrl+C: Copy first selected widget
        if copy_pressed
            && let Some(&sel_id) = self.selected.first()
            && let Some(w) = self.project.widgets.iter().find(|w| w.id == sel_id)
        {
            self.clipboard = Some(w.clone());
        }

        // Ctrl+V: Paste widget from clipboard
        if paste_pressed && self.clipboard.is_some() {
            self.paste();
        }

        // Ctrl+D: Duplicate all selected widgets
        if duplicate_pressed && !self.selected.is_empty() {
            self.duplicate_selected();
        }

        // Ctrl+G: Generate code
        if generate_pressed {
            self.generated = self.generate_code();
            self.set_status("Code generated".into());
        }

        egui::TopBottomPanel::top("menubar").show(ctx, |ui| self.top_bar(ui));
        egui::TopBottomPanel::bottom("app_status_bar").show(ctx, |ui| self.status_bar_ui(ui));
        if self.palette_open {
            egui::SidePanel::left("palette")
                .resizable(true)
                .min_width(180.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.left_panel_tab == 0, "🎨 Palette")
                            .clicked()
                        {
                            self.left_panel_tab = 0;
                        }
                        if ui
                            .selectable_label(self.left_panel_tab == 1, "🗂 Layers")
                            .clicked()
                        {
                            self.left_panel_tab = 1;
                        }
                    });
                    ui.separator();

                    match self.left_panel_tab {
                        0 => self.palette_ui(ui),
                        1 => self.layers_ui(ui),
                        _ => {}
                    }
                });
        }
        egui::SidePanel::right("inspector")
            .default_width(260.0)
            .show(ctx, |ui| {
                // Tab bar for right panel
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.right_panel_tab == 0, "Inspector")
                        .clicked()
                    {
                        self.right_panel_tab = 0;
                    }
                    if ui
                        .selectable_label(self.right_panel_tab == 1, "Code")
                        .clicked()
                    {
                        self.right_panel_tab = 1;
                    }
                });
                ui.separator();

                match self.right_panel_tab {
                    0 => self.inspector_ui(ui),
                    1 => self.generated_panel(ui),
                    _ => {}
                }
            });

        // Set edit mode for widget rendering (inverse of preview mode)
        ctx.data_mut(|d| d.insert_temp(Id::new("edit_mode"), !self.preview_mode));

        self.preview_panels_ui(ctx);

        // Auto-generate code if enabled and widgets exist
        if self.auto_generate && !self.project.widgets.is_empty() {
            self.generated = self.generate_code();
        }

        if self.spawning.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        // Interactive history commit (canvas dragging, resize handles, inspector edits)
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        let kb_active = ctx.wants_keyboard_input();
        if pointer_down || kb_active {
            if self.project != snapshot_before_frame.project {
                self.history.set_pending_if_none(snapshot_before_frame);
            }
        } else {
            self.history.commit_pending_if_changed(&self.project);
        }
    }
}
