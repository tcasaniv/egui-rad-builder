//! Widget metadata and defaults used by the palette, inspector, and generator.
//!
//! The builder stores each dropped control as a `Widget` with a stable
//! `WidgetId`, a `WidgetKind`, layout metadata, and serializable
//! `WidgetProps`. Only palette categories are public for now; the concrete
//! project model remains crate-internal while the generator is still evolving.

use egui::{Pos2, Vec2, pos2};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct WidgetId(u64);

impl WidgetId {
    pub(crate) const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn as_z(&self) -> i32 {
        self.0 as i32
    }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum DockArea {
    #[default]
    Free,
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

/// High-level palette groups used to organize available widget kinds.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetCategory {
    /// Common text, click, selection, and separator controls.
    Basic,
    /// Controls that accept text, numeric, date, color, or selection input.
    Input,
    /// Passive or feedback-oriented controls.
    Display,
    /// Layout, grouping, and window-like controls.
    Containers,
    /// Specialized controls with more complex behavior or generated code.
    Advanced,
}

#[allow(dead_code)]
impl WidgetCategory {
    /// Returns all categories in palette display order.
    pub const fn all() -> &'static [WidgetCategory] {
        &[
            WidgetCategory::Basic,
            WidgetCategory::Input,
            WidgetCategory::Display,
            WidgetCategory::Containers,
            WidgetCategory::Advanced,
        ]
    }

    /// Returns the human-readable label for this category.
    pub const fn display_name(&self) -> &'static str {
        match self {
            WidgetCategory::Basic => "Basic",
            WidgetCategory::Input => "Input",
            WidgetCategory::Display => "Display",
            WidgetCategory::Containers => "Containers",
            WidgetCategory::Advanced => "Advanced",
        }
    }

    /// Returns whether this category should be expanded by default.
    pub const fn default_open(&self) -> bool {
        !matches!(self, WidgetCategory::Advanced)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Widget {
    pub(crate) id: WidgetId,
    pub(crate) kind: WidgetKind,
    pub(crate) pos: Pos2,  // Top-left relative to canvas (or relative to parent container)
    pub(crate) size: Vec2, // Desired size on canvas
    pub(crate) z: i32,     // draw order
    pub(crate) area: DockArea,
    pub(crate) props: WidgetProps,
    /// Optional parent container widget ID.
    /// If `Some(parent_id)`, `pos` is relative to the parent widget's inner content origin.
    #[serde(default)]
    pub(crate) parent: Option<WidgetId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "t", content = "c")]
pub(crate) enum WidgetKind {
    MenuButton,
    Label,
    Heading,
    Small,
    Monospace,
    Button,
    ImageTextButton,
    Checkbox,
    TextEdit,
    TextArea,
    Slider,
    ProgressBar,
    RadioGroup,
    Link,
    Hyperlink,
    SelectableLabel,
    ComboBox,
    Separator,
    CollapsingHeader,
    DatePicker,
    AngleSelector,
    Password,
    Tree,
    DragValue,
    Spinner,
    ColorPicker,
    Code,
    Image,
    Placeholder,
    Group,
    ScrollBox,
    TabBar,
    Columns,
    Window,
    Container,
}

impl WidgetKind {
    /// Returns the category this widget belongs to (for palette organization)
    #[allow(dead_code)]
    pub const fn category(&self) -> WidgetCategory {
        match self {
            // Basic: simple display and interaction elements
            WidgetKind::Label
            | WidgetKind::Button
            | WidgetKind::ImageTextButton
            | WidgetKind::Checkbox
            | WidgetKind::Link
            | WidgetKind::Hyperlink
            | WidgetKind::SelectableLabel
            | WidgetKind::Separator => WidgetCategory::Basic,

            // Input: user data entry widgets
            WidgetKind::TextEdit
            | WidgetKind::TextArea
            | WidgetKind::Password
            | WidgetKind::Slider
            | WidgetKind::DragValue
            | WidgetKind::ComboBox
            | WidgetKind::RadioGroup
            | WidgetKind::DatePicker
            | WidgetKind::AngleSelector
            | WidgetKind::ColorPicker => WidgetCategory::Input,

            // Display: output and feedback widgets
            WidgetKind::Heading
            | WidgetKind::Small
            | WidgetKind::Monospace
            | WidgetKind::ProgressBar
            | WidgetKind::Spinner
            | WidgetKind::Image
            | WidgetKind::Placeholder => WidgetCategory::Display,

            // Containers: layout and grouping widgets
            WidgetKind::Group
            | WidgetKind::ScrollBox
            | WidgetKind::Columns
            | WidgetKind::TabBar
            | WidgetKind::Window
            | WidgetKind::CollapsingHeader
            | WidgetKind::Container => WidgetCategory::Containers,

            // Advanced: complex or specialized widgets
            WidgetKind::MenuButton | WidgetKind::Tree | WidgetKind::Code => {
                WidgetCategory::Advanced
            }
        }
    }

    /// Returns the display name for this widget kind (used in palette)
    #[allow(dead_code)]
    pub const fn display_name(&self) -> &'static str {
        match self {
            WidgetKind::MenuButton => "Menu Button",
            WidgetKind::Label => "Label",
            WidgetKind::Heading => "Heading",
            WidgetKind::Small => "Small",
            WidgetKind::Monospace => "Monospace",
            WidgetKind::Button => "Button",
            WidgetKind::ImageTextButton => "Image + Text Button",
            WidgetKind::Checkbox => "Checkbox",
            WidgetKind::TextEdit => "TextEdit",
            WidgetKind::TextArea => "Text Area",
            WidgetKind::Slider => "Slider",
            WidgetKind::ProgressBar => "ProgressBar",
            WidgetKind::RadioGroup => "Radio Group",
            WidgetKind::Link => "Link",
            WidgetKind::Hyperlink => "Hyperlink",
            WidgetKind::SelectableLabel => "Selectable Label",
            WidgetKind::ComboBox => "Combo Box",
            WidgetKind::Separator => "Separator",
            WidgetKind::CollapsingHeader => "Collapsing Header",
            WidgetKind::DatePicker => "Date Picker",
            WidgetKind::AngleSelector => "Angle Selector",
            WidgetKind::Password => "Password",
            WidgetKind::Tree => "Tree",
            WidgetKind::DragValue => "Drag Value",
            WidgetKind::Spinner => "Spinner",
            WidgetKind::ColorPicker => "Color Picker",
            WidgetKind::Code => "Code Editor",
            WidgetKind::Image => "Image",
            WidgetKind::Placeholder => "Placeholder",
            WidgetKind::Group => "Group",
            WidgetKind::ScrollBox => "Scroll Box",
            WidgetKind::TabBar => "Tab Bar",
            WidgetKind::Columns => "Columns",
            WidgetKind::Window => "Window",
            WidgetKind::Container => "Container",
        }
    }

    /// Returns `true` if this widget kind can contain child widgets.
    pub const fn is_container(&self) -> bool {
        matches!(
            self,
            WidgetKind::Group
                | WidgetKind::ScrollBox
                | WidgetKind::Columns
                | WidgetKind::TabBar
                | WidgetKind::Window
                | WidgetKind::CollapsingHeader
                | WidgetKind::Container
        )
    }

    /// Returns all widget kinds in a given category
    #[allow(dead_code)]
    pub fn widgets_in_category(category: WidgetCategory) -> Vec<WidgetKind> {
        Self::all()
            .iter()
            .filter(|k| k.category() == category)
            .copied()
            .collect()
    }

    /// Returns all widget kinds
    #[allow(dead_code)]
    pub const fn all() -> &'static [WidgetKind] {
        &[
            WidgetKind::Label,
            WidgetKind::Button,
            WidgetKind::ImageTextButton,
            WidgetKind::Checkbox,
            WidgetKind::Link,
            WidgetKind::Hyperlink,
            WidgetKind::SelectableLabel,
            WidgetKind::Separator,
            WidgetKind::TextEdit,
            WidgetKind::TextArea,
            WidgetKind::Password,
            WidgetKind::Slider,
            WidgetKind::DragValue,
            WidgetKind::ComboBox,
            WidgetKind::RadioGroup,
            WidgetKind::DatePicker,
            WidgetKind::AngleSelector,
            WidgetKind::ColorPicker,
            WidgetKind::Heading,
            WidgetKind::Small,
            WidgetKind::Monospace,
            WidgetKind::ProgressBar,
            WidgetKind::Spinner,
            WidgetKind::Image,
            WidgetKind::Placeholder,
            WidgetKind::Group,
            WidgetKind::ScrollBox,
            WidgetKind::Columns,
            WidgetKind::TabBar,
            WidgetKind::Window,
            WidgetKind::Container,
            WidgetKind::CollapsingHeader,
            WidgetKind::MenuButton,
            WidgetKind::Tree,
            WidgetKind::Code,
        ]
    }

    /// Returns the default size for a widget of this kind.
    /// Centralized to avoid duplication between spawn_widget and ghost preview.
    pub fn default_size(&self) -> egui::Vec2 {
        use egui::vec2;
        match self {
            WidgetKind::MenuButton => vec2(180.0, 28.0),
            WidgetKind::Label => vec2(140.0, 24.0),
            WidgetKind::Button => vec2(160.0, 32.0),
            WidgetKind::ImageTextButton => vec2(200.0, 36.0),
            WidgetKind::Checkbox => vec2(160.0, 28.0),
            WidgetKind::TextEdit => vec2(220.0, 36.0),
            WidgetKind::Slider => vec2(220.0, 24.0),
            WidgetKind::ProgressBar => vec2(220.0, 20.0),
            WidgetKind::RadioGroup => vec2(200.0, 80.0),
            WidgetKind::Link => vec2(160.0, 20.0),
            WidgetKind::Hyperlink => vec2(200.0, 20.0),
            WidgetKind::SelectableLabel => vec2(180.0, 24.0),
            WidgetKind::ComboBox => vec2(220.0, 28.0),
            WidgetKind::Separator => vec2(220.0, 8.0),
            WidgetKind::CollapsingHeader => vec2(260.0, 80.0),
            WidgetKind::DatePicker => vec2(200.0, 28.0),
            WidgetKind::AngleSelector => vec2(220.0, 28.0),
            WidgetKind::Password => vec2(220.0, 36.0),
            WidgetKind::Tree => vec2(260.0, 200.0),
            WidgetKind::TextArea => vec2(280.0, 120.0),
            WidgetKind::DragValue => vec2(180.0, 24.0),
            WidgetKind::Spinner => vec2(32.0, 32.0),
            WidgetKind::ColorPicker => vec2(200.0, 28.0),
            WidgetKind::Code => vec2(300.0, 150.0),
            WidgetKind::Heading => vec2(200.0, 32.0),
            WidgetKind::Small => vec2(120.0, 20.0),
            WidgetKind::Monospace => vec2(140.0, 20.0),
            WidgetKind::Image => vec2(150.0, 150.0),
            WidgetKind::Placeholder => vec2(200.0, 100.0),
            WidgetKind::Group => vec2(250.0, 150.0),
            WidgetKind::ScrollBox => vec2(200.0, 150.0),
            WidgetKind::TabBar => vec2(300.0, 32.0),
            WidgetKind::Columns => vec2(300.0, 120.0),
            WidgetKind::Window => vec2(280.0, 180.0),
            WidgetKind::Container => vec2(200.0, 150.0),
        }
    }

    /// Returns the default properties for a widget of this kind.
    pub fn default_props(&self) -> WidgetProps {
        match self {
            WidgetKind::MenuButton => {
                let mut p = WidgetProps {
                    text: "Menu".into(),
                    ..Default::default()
                };
                p.items = vec!["First".into(), "Second".into(), "Third".into()];
                p.selected = 0;
                p
            }
            WidgetKind::Label => WidgetProps {
                text: "Label".into(),
                ..Default::default()
            },
            WidgetKind::Button => WidgetProps {
                text: "Button".into(),
                ..Default::default()
            },
            WidgetKind::ImageTextButton => WidgetProps {
                text: "Button".into(),
                icon: "🖼️".into(),
                ..Default::default()
            },
            WidgetKind::Checkbox => WidgetProps {
                text: "Checkbox".into(),
                ..Default::default()
            },
            WidgetKind::TextEdit => WidgetProps {
                text: "Type here".into(),
                ..Default::default()
            },
            WidgetKind::Slider => WidgetProps {
                text: "Value".into(),
                min: 0.0,
                max: 100.0,
                value: 42.0,
                checked: false,
                ..Default::default()
            },
            WidgetKind::ProgressBar => WidgetProps {
                text: "".into(),
                value: 0.25,
                min: 0.0,
                max: 1.0,
                checked: false,
                ..Default::default()
            },
            WidgetKind::RadioGroup => {
                let mut p = WidgetProps {
                    text: "Radio Group".into(),
                    ..Default::default()
                };
                p.items = vec!["Option A".into(), "Option B".into(), "Option C".into()];
                p.selected = 0;
                p
            }
            WidgetKind::Link => WidgetProps {
                text: "Link text".into(),
                ..Default::default()
            },
            WidgetKind::Hyperlink => WidgetProps {
                text: "Open website".into(),
                url: "https://example.com".into(),
                ..Default::default()
            },
            WidgetKind::SelectableLabel => WidgetProps {
                text: "Selectable".into(),
                checked: false,
                ..Default::default()
            },
            WidgetKind::ComboBox => {
                let mut p = WidgetProps {
                    text: "Choose one".into(),
                    ..Default::default()
                };
                p.items = vec!["Red".into(), "Green".into(), "Blue".into()];
                p.selected = 0;
                p
            }
            WidgetKind::Separator => WidgetProps::default(),
            WidgetKind::CollapsingHeader => WidgetProps {
                text: "Section".into(),
                checked: true, // default open
                ..Default::default()
            },
            WidgetKind::DatePicker => WidgetProps {
                text: "Pick a date".into(),
                year: 2025,
                month: 1,
                day: 1,
                ..Default::default()
            },
            WidgetKind::AngleSelector => WidgetProps {
                text: "Angle (deg)".into(),
                min: 0.0,
                max: 360.0,
                value: 45.0,
                ..Default::default()
            },
            WidgetKind::Password => WidgetProps {
                text: "password".into(),
                ..Default::default()
            },
            WidgetKind::Tree => {
                let mut p = WidgetProps {
                    text: "Tree".into(),
                    ..Default::default()
                };
                // Indentation (two spaces = one level) to define hierarchy
                p.items = vec![
                    "Animals".into(),
                    "  Mammals".into(),
                    "    Dogs".into(),
                    "    Cats".into(),
                    "  Birds".into(),
                    "Plants".into(),
                    "  Trees".into(),
                    "  Flowers".into(),
                ];
                p
            }
            WidgetKind::TextArea => WidgetProps {
                text: "Multi-line\ntext here".into(),
                ..Default::default()
            },
            WidgetKind::DragValue => WidgetProps {
                text: "Value".into(),
                value: 42.0,
                min: 0.0,
                max: 100.0,
                ..Default::default()
            },
            WidgetKind::Spinner => WidgetProps::default(),
            WidgetKind::ColorPicker => WidgetProps {
                text: "Color".into(),
                color: [100, 149, 237, 255],
                ..Default::default()
            },
            WidgetKind::Code => WidgetProps {
                text: "fn main() {\n    println!(\"Hello\");\n}".into(),
                ..Default::default()
            },
            WidgetKind::Heading => WidgetProps {
                text: "Heading".into(),
                ..Default::default()
            },
            WidgetKind::Small => WidgetProps {
                text: "Small text".into(),
                ..Default::default()
            },
            WidgetKind::Monospace => WidgetProps {
                text: "code_value".into(),
                ..Default::default()
            },
            WidgetKind::Image => WidgetProps {
                text: "image.png".into(),
                url: "file://image.png".into(),
                ..Default::default()
            },
            WidgetKind::Placeholder => WidgetProps {
                text: "Placeholder".into(),
                color: [128, 128, 128, 128],
                ..Default::default()
            },
            WidgetKind::Group => WidgetProps {
                text: "Group".into(),
                ..Default::default()
            },
            WidgetKind::ScrollBox => WidgetProps {
                text: "Scroll content here...".into(),
                ..Default::default()
            },
            WidgetKind::TabBar => WidgetProps {
                items: vec!["Tab 1".into(), "Tab 2".into(), "Tab 3".into()],
                selected: 0,
                ..Default::default()
            },
            WidgetKind::Columns => WidgetProps {
                text: "Column content".into(),
                columns: 2,
                ..Default::default()
            },
            WidgetKind::Window => WidgetProps {
                text: "Window Title".into(),
                ..Default::default()
            },
            WidgetKind::Container => WidgetProps {
                text: String::new(),
                layout_mode: LayoutMode::Column,
                layout_gap: 8.0,
                layout_padding: [0.0; 4],
                auto_size_y: false,
                ..Default::default()
            },
        }
    }
}

/// Trigger event that initiates a widget action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ActionTrigger {
    OnClick,
    OnHover,
    OnChanged,
    OnDoubleClick,
}

impl ActionTrigger {
    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::OnClick => "On Click",
            Self::OnHover => "On Hover",
            Self::OnChanged => "On Changed",
            Self::OnDoubleClick => "On Double Click",
        }
    }

    pub(crate) const ALL: &'static [ActionTrigger] = &[
        Self::OnClick,
        Self::OnHover,
        Self::OnChanged,
        Self::OnDoubleClick,
    ];
}

/// Effect performed when an action trigger fires.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ActionEffect {
    ShowWidget(WidgetId),
    HideWidget(WidgetId),
    ToggleWidget(WidgetId),
    SetText {
        target: WidgetId,
        text: String,
    },
    SwitchTab {
        target: WidgetId,
        tab_index: usize,
    },
    OpenModal(WidgetId),
    CloseModal(WidgetId),
    CustomRustCode(String),
}

#[allow(dead_code)]
impl ActionEffect {
    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::ShowWidget(_) => "Show Widget",
            Self::HideWidget(_) => "Hide Widget",
            Self::ToggleWidget(_) => "Toggle Visibility",
            Self::SetText { .. } => "Set Text",
            Self::SwitchTab { .. } => "Switch Tab",
            Self::OpenModal(_) => "Open Window/Modal",
            Self::CloseModal(_) => "Close Window/Modal",
            Self::CustomRustCode(_) => "Custom Rust Code",
        }
    }

    pub(crate) fn target_widget(&self) -> Option<WidgetId> {
        match self {
            Self::ShowWidget(id)
            | Self::HideWidget(id)
            | Self::ToggleWidget(id)
            | Self::SetText { target: id, .. }
            | Self::SwitchTab { target: id, .. }
            | Self::OpenModal(id)
            | Self::CloseModal(id) => Some(*id),
            Self::CustomRustCode(_) => None,
        }
    }
}

/// An interactive action binding a trigger to an effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WidgetAction {
    pub(crate) trigger: ActionTrigger,
    pub(crate) effect: ActionEffect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WidgetProps {
    pub(crate) text: String,  // label/button/textedit placeholder
    pub(crate) checked: bool, // checkbox
    pub(crate) value: f32,    // slider/progress/dragvalue
    pub(crate) min: f32,
    pub(crate) max: f32,
    // lists (for radio/combobox)
    pub(crate) items: Vec<String>,
    pub(crate) selected: usize,
    // hyperlinks
    pub(crate) url: String,
    // date (stored as y/m/d to avoid chrono serde feature requirements)
    pub(crate) year: i32,
    pub(crate) month: u32,
    pub(crate) day: u32,
    pub(crate) icon: String,
    // color (rgba 0-255)
    pub(crate) color: [u8; 4],
    // optional tooltip text shown on hover in the generated app
    pub(crate) tooltip: String,
    // layout direction (for Group)
    pub(crate) horizontal: bool,
    // enabled state
    pub(crate) enabled: bool,
    // column count (for Columns widget)
    pub(crate) columns: usize,

    // --- Display control fields (all serde-defaulted for backward compat) ---
    /// Friendly name shown in the Layers panel.
    /// Empty string → falls back to `kind.display_name()`.
    #[serde(default)]
    pub(crate) name: String,

    /// If `false`, the widget is completely excluded from the canvas and
    /// from generated code. Use this to deactivate a widget during
    /// prototyping without deleting it.
    #[serde(default = "widget_prop_default_true")]
    pub(crate) active: bool,

    /// Initial visibility state when the prototype runs.
    /// `false` = starts hidden; actions can show/hide it at runtime.
    /// In design mode the widget is always shown (with a dashed outline),
    /// so you can still position and configure it.
    #[serde(default = "widget_prop_default_true")]
    pub(crate) initially_visible: bool,

    /// Visual opacity in the range `[0.0, 1.0]`.
    /// Applied both in the canvas preview and in the generated code
    /// via `ui.scope(|ui| { ui.set_opacity(x); … })`.
    #[serde(default = "widget_prop_default_opacity")]
    pub(crate) opacity: f32,

    /// Interactive actions defined on this widget.
    #[serde(default)]
    pub(crate) actions: Vec<WidgetAction>,

    // ── Layout (container props) ───────────────────────────────────────────
    /// How this container distributes its children. CSS: `display`/`flex-direction`.
    /// Ignored on non-container widgets.
    #[serde(default)]
    pub(crate) layout_mode: LayoutMode,

    /// Gap between children in auto-layout modes (px). CSS: `gap`.
    #[serde(default = "default_layout_gap")]
    pub(crate) layout_gap: f32,

    /// Number of columns for `Grid` mode. CSS: `grid-template-columns: repeat(N, 1fr)`.
    #[serde(default = "default_layout_cols")]
    pub(crate) layout_cols: usize,

    /// Cross-axis alignment. CSS: `align-items`.
    #[serde(default)]
    pub(crate) layout_align: Align,

    /// Main-axis justification. CSS: `justify-content`.
    #[serde(default)]
    pub(crate) layout_justify: Justify,

    /// Inner padding [top, right, bottom, left] in px. CSS: `padding`.
    #[serde(default)]
    pub(crate) layout_padding: [f32; 4],

    // ── Size policy (child props) ──────────────────────────────────────────
    /// Width policy when inside an auto-layout parent. CSS: `width` / `flex-basis`.
    #[serde(default)]
    pub(crate) width_policy: SizePolicy,

    /// Height policy when inside an auto-layout parent. CSS: `height`.
    #[serde(default)]
    pub(crate) height_policy: SizePolicy,

    /// Per-child cross-axis alignment override. CSS: `align-self`.
    /// `None` means inherit from parent container's `layout_align`.
    #[serde(default)]
    pub(crate) align_self: Option<Align>,

    // ── TabBar child configuration ─────────────────────────────────────────
    /// For widgets that are children of a TabBar container:
    /// indicates which tab page index this widget belongs to.
    /// `None` = visible on all tabs.
    #[serde(default)]
    pub(crate) tab_page: Option<usize>,

    // ── Auto-sizing ────────────────────────────────────────────────────────
    /// Auto-adjust container height to wrap all its children + padding (`fit-content`).
    #[serde(default = "widget_prop_default_true")]
    pub(crate) auto_size_y: bool,

    /// Saved height when a collapsible container (like CollapsingHeader) is expanded.
    /// Preserved when the widget collapses so it can be restored on expansion.
    #[serde(default)]
    pub(crate) expanded_height: Option<f32>,

    // ── Responsive configuration ───────────────────────────────────────────
    /// Conditional visibility based on screen orientation/breakpoint.
    #[serde(default)]
    pub(crate) responsive_vis: ResponsiveVisibility,

    /// Adaptive layout rule for containers.
    #[serde(default)]
    pub(crate) responsive_layout: ResponsiveLayout,

    /// Whether to render the visual tab header buttons for a TabBar container.
    /// When `false`, the tab bar header is hidden, acting as a clean ViewStack / Screen Switcher.
    #[serde(default = "widget_prop_default_true")]
    pub(crate) show_tabs: bool,

    // ── Visual Appearance & Styling (Figma-grade) ──────────────────────────
    /// Custom background fill color [R, G, B, A].
    #[serde(default)]
    pub(crate) bg_color: Option<[u8; 4]>,

    /// Custom text / foreground color [R, G, B, A].
    #[serde(default)]
    pub(crate) text_color: Option<[u8; 4]>,

    /// Custom font size in logical points.
    #[serde(default)]
    pub(crate) font_size: Option<f32>,

    /// Bold text styling.
    #[serde(default)]
    pub(crate) font_bold: bool,

    /// Custom border stroke color [R, G, B, A].
    #[serde(default)]
    pub(crate) border_color: Option<[u8; 4]>,

    /// Custom border stroke width in px.
    #[serde(default)]
    pub(crate) border_width: Option<f32>,

    /// Custom corner radius in px.
    #[serde(default)]
    pub(crate) corner_radius: Option<f32>,
}

fn widget_prop_default_true() -> bool {
    true
}

fn widget_prop_default_opacity() -> f32 {
    1.0
}

impl Default for WidgetProps {
    fn default() -> Self {
        Self {
            text: "Label".into(),
            checked: false,
            value: 0.5,
            min: 0.0,
            max: 1.0,
            items: vec![],
            selected: 0,
            url: "https://example.com".into(),
            year: 2024,
            month: 1,
            day: 1,
            icon: "🖼️".into(),
            color: [100, 149, 237, 255], // cornflower blue
            tooltip: String::new(),
            horizontal: false,
            enabled: true,
            columns: 2,
            name: String::new(),
            active: true,
            initially_visible: true,
            opacity: 1.0,
            actions: Vec::new(),
            // layout (container)
            layout_mode: LayoutMode::Free,
            layout_gap: 8.0,
            layout_cols: 2,
            layout_align: Align::Start,
            layout_justify: Justify::Start,
            layout_padding: [0.0; 4],
            // size policy (child)
            width_policy: SizePolicy::Fixed,
            height_policy: SizePolicy::Fixed,
            align_self: None,
            tab_page: None,
            auto_size_y: true,
            expanded_height: None,
            responsive_vis: ResponsiveVisibility::Always,
            responsive_layout: ResponsiveLayout::None,
            show_tabs: true,
            bg_color: None,
            text_color: None,
            font_size: None,
            font_bold: false,
            border_color: None,
            border_width: None,
            corner_radius: None,
        }
    }
}

pub(crate) fn snap_pos_with_grid(p: Pos2, grid: f32) -> Pos2 {
    pos2((p.x / grid).round() * grid, (p.y / grid).round() * grid)
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── Responsive system ─────────────────────────────────────────────────────────

/// Conditional visibility rules based on screen orientation / breakpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum ResponsiveVisibility {
    /// Always visible regardless of screen format.
    #[default]
    Always,
    /// Visible only when height > width (portrait / vertical mobile).
    PortraitOnly,
    /// Visible only when width >= height (landscape / desktop / horizontal mobile).
    LandscapeOnly,
    /// Visible only when width <= 600px (mobile breakpoint).
    MobileOnly,
    /// Visible only when width > 600px (desktop/tablet breakpoint).
    DesktopOnly,
}

#[allow(dead_code)]
impl ResponsiveVisibility {
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::Always        => "Always",
            Self::PortraitOnly  => "Portrait Only (Vertical)",
            Self::LandscapeOnly => "Landscape Only (Horizontal)",
            Self::MobileOnly    => "Mobile Only (<= 600px)",
            Self::DesktopOnly   => "Desktop Only (> 600px)",
        }
    }

    pub(crate) const fn all() -> &'static [ResponsiveVisibility] {
        &[
            Self::Always,
            Self::PortraitOnly,
            Self::LandscapeOnly,
            Self::MobileOnly,
            Self::DesktopOnly,
        ]
    }

    /// Evaluates if this visibility rule is satisfied given the canvas/window size.
    pub(crate) fn is_visible(self, size: egui::Vec2) -> bool {
        match self {
            Self::Always => true,
            Self::PortraitOnly => size.y > size.x,
            Self::LandscapeOnly => size.x >= size.y,
            Self::MobileOnly => size.x <= 600.0,
            Self::DesktopOnly => size.x > 600.0,
        }
    }
}

/// Adaptive layout behaviour for containers depending on screen orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum ResponsiveLayout {
    /// Do not adapt layout mode automatically.
    #[default]
    None,
    /// Acts as `Row` in Landscape, but switches automatically to `Column` in Portrait.
    RowToColumnOnPortrait,
}

#[allow(dead_code)]
impl ResponsiveLayout {
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::None => "None (keep assigned mode)",
            Self::RowToColumnOnPortrait => "Row in Landscape -> Column in Portrait",
        }
    }

    pub(crate) const fn all() -> &'static [ResponsiveLayout] {
        &[Self::None, Self::RowToColumnOnPortrait]
    }

    /// Determines the effective layout mode given the base mode and canvas size.
    pub(crate) fn resolve_mode(self, base: LayoutMode, canvas_size: egui::Vec2) -> LayoutMode {
        match self {
            Self::None => base,
            Self::RowToColumnOnPortrait => {
                if canvas_size.y > canvas_size.x {
                    // Vertical (portrait): stack vertically
                    LayoutMode::Column
                } else {
                    // Horizontal (landscape): arrange horizontally
                    LayoutMode::Row
                }
            }
        }
    }
}

// ── Layout system ─────────────────────────────────────────────────────────────

/// How a container distributes its children along the main axis.
/// Equivalent to CSS `display: flex/grid` + `flex-direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum LayoutMode {
    /// Absolute (x, y) positioning — default, current behavior.
    #[default]
    Free,
    /// Children left → right (CSS: `flex-direction: row`).
    Row,
    /// Children top → bottom (CSS: `flex-direction: column`).
    Column,
    /// Children left → right with wrapping (CSS: `flex-wrap: wrap`).
    WrapRow,
    /// Children in an N-column grid (CSS: `display: grid`).
    Grid,
}

impl LayoutMode {
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::Free    => "Free (absolute)",
            Self::Row     => "Row",
            Self::Column  => "Column",
            Self::WrapRow => "Wrap Row",
            Self::Grid    => "Grid",
        }
    }
    pub(crate) const fn all() -> &'static [LayoutMode] {
        &[LayoutMode::Free, LayoutMode::Row, LayoutMode::Column, LayoutMode::WrapRow, LayoutMode::Grid]
    }
}

/// Cross-axis alignment. CSS `align-items` / `align-self`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum Align {
    /// CSS: `flex-start`
    #[default]
    Start,
    /// CSS: `center`
    Center,
    /// CSS: `flex-end`
    End,
    /// Fills the cross-axis. CSS: `stretch`
    Stretch,
}

impl Align {
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::Start   => "Start",
            Self::Center  => "Center",
            Self::End     => "End",
            Self::Stretch => "Stretch",
        }
    }
    pub(crate) const fn all() -> &'static [Align] {
        &[Align::Start, Align::Center, Align::End, Align::Stretch]
    }
}

/// Main-axis justification. CSS `justify-content`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum Justify {
    /// CSS: `flex-start`
    #[default]
    Start,
    /// CSS: `center`
    Center,
    /// CSS: `flex-end`
    End,
    /// Equal gaps between items, none at edges. CSS: `space-between`
    SpaceBetween,
    /// Equal gaps around items. CSS: `space-around`
    SpaceAround,
    /// Equal gaps between items and edges. CSS: `space-evenly`
    SpaceEvenly,
}

impl Justify {
    pub(crate) const fn display_name(self) -> &'static str {
        match self {
            Self::Start       => "Start",
            Self::Center      => "Center",
            Self::End         => "End",
            Self::SpaceBetween => "Space Between",
            Self::SpaceAround  => "Space Around",
            Self::SpaceEvenly  => "Space Evenly",
        }
    }
    pub(crate) const fn all() -> &'static [Justify] {
        &[
            Justify::Start, Justify::Center, Justify::End,
            Justify::SpaceBetween, Justify::SpaceAround, Justify::SpaceEvenly,
        ]
    }
}

/// How a widget's size is computed along one axis when inside an auto-layout container.
/// Ignored in `Free` mode (absolute positioning).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "t", content = "v")]
pub(crate) enum SizePolicy {
    /// Use the widget's `size.x` / `size.y` as-is (pixel). Default.
    #[default]
    Fixed,
    /// Percentage of the parent container's inner dimension (0.0–100.0).
    /// e.g. `Percent(50.0)` = 50% of parent width/height.
    Percent(f32),
    /// Expand to fill remaining space after Fixed/Percent siblings.
    /// Equivalent to CSS `flex: 1`. Multiple Fill children share equally.
    Fill,
}

#[allow(dead_code)]
impl SizePolicy {
    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::Fixed       => "Fixed (px)",
            Self::Percent(_)  => "Percent (%)",
            Self::Fill        => "Fill",
        }
    }

    /// Resolve to a pixel value given the available dimension.
    pub(crate) fn resolve(&self, fixed_px: f32, available: f32) -> Option<f32> {
        match self {
            Self::Fixed      => Some(fixed_px),
            Self::Percent(p) => Some(available * p / 100.0),
            Self::Fill       => None, // handled separately in the layout pass
        }
    }
}

fn default_layout_gap()  -> f32   { 8.0 }
fn default_layout_cols() -> usize { 2 }

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn test_snap_pos_with_grid() {
        // Test snapping to grid of 10
        assert_eq!(snap_pos_with_grid(pos2(5.0, 5.0), 10.0), pos2(10.0, 10.0));
        assert_eq!(snap_pos_with_grid(pos2(4.9, 4.9), 10.0), pos2(0.0, 0.0));
        assert_eq!(snap_pos_with_grid(pos2(15.0, 25.0), 10.0), pos2(20.0, 30.0));

        // Test snapping to grid of 1 (no-op for integers)
        assert_eq!(snap_pos_with_grid(pos2(5.4, 3.6), 1.0), pos2(5.0, 4.0));

        // Test snapping to grid of 8
        assert_eq!(snap_pos_with_grid(pos2(12.0, 20.0), 8.0), pos2(16.0, 24.0));
    }

    #[test]
    fn test_escape() {
        // Test basic strings
        assert_eq!(escape("hello"), "hello");

        // Test backslash escaping
        assert_eq!(escape("path\\to\\file"), "path\\\\to\\\\file");

        // Test quote escaping
        assert_eq!(escape("say \"hello\""), "say \\\"hello\\\"");

        // Test combined
        assert_eq!(escape("c:\\path\\\"file\""), "c:\\\\path\\\\\\\"file\\\"");
    }

    #[test]
    fn test_widget_kind_default_size() {
        // All widget kinds should return positive dimensions
        let kinds = [
            WidgetKind::Label,
            WidgetKind::Button,
            WidgetKind::Checkbox,
            WidgetKind::TextEdit,
            WidgetKind::Slider,
            WidgetKind::ProgressBar,
            WidgetKind::RadioGroup,
            WidgetKind::ComboBox,
            WidgetKind::Separator,
            WidgetKind::Spinner,
            WidgetKind::ColorPicker,
            WidgetKind::Group,
            WidgetKind::Window,
        ];

        for kind in kinds {
            let size = kind.default_size();
            assert!(size.x > 0.0, "{:?} should have positive width", kind);
            assert!(size.y > 0.0, "{:?} should have positive height", kind);
        }
    }

    #[test]
    fn test_widget_kind_default_props() {
        // Test that default props are reasonable
        let label_props = WidgetKind::Label.default_props();
        assert_eq!(label_props.text, "Label");

        let button_props = WidgetKind::Button.default_props();
        assert_eq!(button_props.text, "Button");

        let slider_props = WidgetKind::Slider.default_props();
        assert!(slider_props.min < slider_props.max);
        assert!(slider_props.value >= slider_props.min);
        assert!(slider_props.value <= slider_props.max);

        let combobox_props = WidgetKind::ComboBox.default_props();
        assert!(!combobox_props.items.is_empty());
        assert!(combobox_props.selected < combobox_props.items.len());
    }

    #[test]
    fn test_widget_props_default() {
        let props = WidgetProps::default();
        assert!(!props.text.is_empty());
        assert!(props.min <= props.max);
        assert_eq!(props.columns, 2);
        assert!(props.enabled);
        // New display-control fields
        assert!(props.name.is_empty());
        assert!(props.active);
        assert!(props.initially_visible);
        assert!((props.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_widget_props_display_control_defaults() {
        // Verify the three display-control fields match spec
        let props = WidgetProps::default();
        assert!(props.active, "active should default to true");
        assert!(
            props.initially_visible,
            "initially_visible should default to true"
        );
        assert_eq!(props.opacity, 1.0, "opacity should default to 1.0");
    }

    #[test]
    fn test_widget_props_serde_backward_compat() {
        // JSON that predates the new fields should deserialize with correct defaults.
        let legacy_json = r#"{
            "text": "Click me",
            "checked": false,
            "value": 0.5,
            "min": 0.0,
            "max": 1.0,
            "items": [],
            "selected": 0,
            "url": "https://example.com",
            "year": 2024,
            "month": 1,
            "day": 1,
            "icon": "🖼️",
            "color": [100, 149, 237, 255],
            "tooltip": "",
            "horizontal": false,
            "enabled": true,
            "columns": 2
        }"#;

        let props: WidgetProps =
            serde_json::from_str(legacy_json).expect("should deserialize legacy JSON");

        assert_eq!(props.text, "Click me");
        assert!(props.name.is_empty(), "name should default to empty string");
        assert!(props.active, "active should default to true");
        assert!(
            props.initially_visible,
            "initially_visible should default to true"
        );
        assert_eq!(props.opacity, 1.0, "opacity should default to 1.0");
    }

    #[test]
    fn test_widget_props_serde_round_trip() {
        let mut props = WidgetProps::default();
        props.name = "My Button".into();
        props.active = false;
        props.initially_visible = false;
        props.opacity = 0.5;

        let json = serde_json::to_string(&props).expect("serialize");
        let decoded: WidgetProps = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.name, "My Button");
        assert!(!decoded.active);
        assert!(!decoded.initially_visible);
        assert!((decoded.opacity - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_dock_area_default() {
        let area = DockArea::default();
        assert_eq!(area, DockArea::Free);
    }

    #[test]
    fn test_widget_id_display() {
        let id = WidgetId::new(42);
        assert_eq!(format!("{}", id), "42");
    }

    #[test]
    fn test_widget_id_z_order() {
        let id = WidgetId::new(100);
        assert_eq!(id.as_z(), 100);
    }

    #[test]
    fn test_widget_category_all() {
        // All categories should be present
        let categories = WidgetCategory::all();
        assert_eq!(categories.len(), 5);
        assert!(categories.contains(&WidgetCategory::Basic));
        assert!(categories.contains(&WidgetCategory::Input));
        assert!(categories.contains(&WidgetCategory::Display));
        assert!(categories.contains(&WidgetCategory::Containers));
        assert!(categories.contains(&WidgetCategory::Advanced));
    }

    #[test]
    fn test_widget_category_display_order_and_labels() {
        let labels: Vec<_> = WidgetCategory::all()
            .iter()
            .map(WidgetCategory::display_name)
            .collect();

        assert_eq!(
            labels,
            vec!["Basic", "Input", "Display", "Containers", "Advanced"]
        );
    }

    #[test]
    fn test_widget_category_default_open_policy() {
        for category in WidgetCategory::all() {
            assert_eq!(
                category.default_open(),
                !matches!(category, WidgetCategory::Advanced),
                "{category:?} default-open policy changed"
            );
        }
    }

    #[test]
    fn test_widget_kind_categories() {
        // Test a few widgets are in correct categories
        assert_eq!(WidgetKind::Label.category(), WidgetCategory::Basic);
        assert_eq!(WidgetKind::Button.category(), WidgetCategory::Basic);
        assert_eq!(WidgetKind::TextEdit.category(), WidgetCategory::Input);
        assert_eq!(WidgetKind::Slider.category(), WidgetCategory::Input);
        assert_eq!(WidgetKind::ProgressBar.category(), WidgetCategory::Display);
        assert_eq!(WidgetKind::Group.category(), WidgetCategory::Containers);
        assert_eq!(WidgetKind::Tree.category(), WidgetCategory::Advanced);
    }

    #[test]
    fn test_widget_kind_all_have_categories() {
        // All widgets should have a valid category
        for kind in WidgetKind::all() {
            let _category = kind.category(); // Should not panic
            let _name = kind.display_name(); // Should not panic
        }
    }

    #[test]
    fn test_widget_kind_display_names() {
        assert_eq!(WidgetKind::Label.display_name(), "Label");
        assert_eq!(
            WidgetKind::ImageTextButton.display_name(),
            "Image + Text Button"
        );
        assert_eq!(
            WidgetKind::CollapsingHeader.display_name(),
            "Collapsing Header"
        );
    }

    #[test]
    fn test_widgets_in_category() {
        let basic_widgets = WidgetKind::widgets_in_category(WidgetCategory::Basic);
        assert!(basic_widgets.contains(&WidgetKind::Label));
        assert!(basic_widgets.contains(&WidgetKind::Button));
        assert!(!basic_widgets.contains(&WidgetKind::TextEdit)); // Input, not Basic

        let input_widgets = WidgetKind::widgets_in_category(WidgetCategory::Input);
        assert!(input_widgets.contains(&WidgetKind::TextEdit));
        assert!(input_widgets.contains(&WidgetKind::Slider));
    }

    #[test]
    fn test_widget_parent_serde_backward_compat() {
        let legacy_widget_json = r#"{
            "id": 1,
            "kind": {"t": "Button"},
            "pos": [10.0, 20.0],
            "size": [80.0, 24.0],
            "z": 1,
            "area": "Free",
            "props": {
                "text": "OldButton",
                "checked": false,
                "value": 0.5,
                "min": 0.0,
                "max": 1.0,
                "items": [],
                "selected": 0,
                "url": "https://example.com",
                "year": 2024,
                "month": 1,
                "day": 1,
                "icon": "🖼️",
                "color": [100, 149, 237, 255],
                "tooltip": "",
                "horizontal": false,
                "enabled": true,
                "columns": 2
            }
        }"#;

        let w: Widget = serde_json::from_str(legacy_widget_json)
            .expect("legacy widget json without parent should deserialize");
        assert_eq!(w.parent, None, "parent should default to None");
    }

    #[test]
    fn test_project_hierarchy_helpers() {
        let mut proj = crate::project::Project::default();
        let w_group = Widget {
            id: WidgetId::new(1),
            kind: WidgetKind::Group,
            pos: pos2(0.0, 0.0),
            size: egui::vec2(200.0, 200.0),
            z: 1,
            area: DockArea::Center,
            props: WidgetProps::default(),
            parent: None,
        };
        let w_btn = Widget {
            id: WidgetId::new(2),
            kind: WidgetKind::Button,
            pos: pos2(10.0, 10.0),
            size: egui::vec2(80.0, 24.0),
            z: 2,
            area: DockArea::Center,
            props: WidgetProps::default(),
            parent: Some(WidgetId::new(1)),
        };
        let w_sub = Widget {
            id: WidgetId::new(3),
            kind: WidgetKind::Label,
            pos: pos2(5.0, 5.0),
            size: egui::vec2(60.0, 18.0),
            z: 3,
            area: DockArea::Center,
            props: WidgetProps::default(),
            parent: Some(WidgetId::new(2)),
        };
        proj.widgets.push(w_group);
        proj.widgets.push(w_btn);
        proj.widgets.push(w_sub);

        let children = proj.children_of(WidgetId::new(1));
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, WidgetId::new(2));

        assert!(proj.is_descendant_of(WidgetId::new(3), WidgetId::new(1)));
        assert!(proj.is_descendant_of(WidgetId::new(2), WidgetId::new(1)));
        assert!(!proj.is_descendant_of(WidgetId::new(1), WidgetId::new(2)));

        let desc = proj.get_descendants(WidgetId::new(1));
        assert_eq!(desc.len(), 2);
        assert!(desc.contains(&WidgetId::new(2)));
        assert!(desc.contains(&WidgetId::new(3)));
    }

    #[test]
    fn test_widget_action_serde_round_trip() {
        let action = WidgetAction {
            trigger: ActionTrigger::OnClick,
            effect: ActionEffect::ToggleWidget(WidgetId::new(42)),
        };

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: WidgetAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
        assert_eq!(deserialized.trigger.display_name(), "On Click");
        assert_eq!(deserialized.effect.display_name(), "Toggle Visibility");
        assert_eq!(deserialized.effect.target_widget(), Some(WidgetId::new(42)));
    }

    #[test]
    fn test_widget_props_actions_serde_backward_compat() {
        // Old JSON without actions field
        let json = r#"{
            "text": "ClickMe",
            "checked": false,
            "value": 0.0,
            "min": 0.0,
            "max": 100.0,
            "items": [],
            "selected": 0,
            "url": "",
            "year": 2025,
            "month": 1,
            "day": 1,
            "icon": "",
            "color": [255, 255, 255, 255],
            "tooltip": "",
            "horizontal": false,
            "enabled": true,
            "columns": 1
        }"#;

        let props: WidgetProps = serde_json::from_str(json).unwrap();
        assert!(props.actions.is_empty());
        assert_eq!(props.layout_mode, LayoutMode::Free);
        assert_eq!(props.width_policy, SizePolicy::Fixed);
        assert_eq!(props.height_policy, SizePolicy::Fixed);
    }

    #[test]
    fn test_layout_types_serde_round_trip() {
        let props = WidgetProps {
            layout_mode: LayoutMode::Row,
            layout_gap: 12.0,
            layout_cols: 3,
            layout_align: Align::Center,
            layout_justify: Justify::SpaceBetween,
            layout_padding: [4.0, 8.0, 4.0, 8.0],
            width_policy: SizePolicy::Percent(50.0),
            height_policy: SizePolicy::Fill,
            align_self: Some(Align::Stretch),
            ..Default::default()
        };

        let json = serde_json::to_string(&props).unwrap();
        let deserialized: WidgetProps = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.layout_mode, LayoutMode::Row);
        assert_eq!(deserialized.layout_gap, 12.0);
        assert_eq!(deserialized.layout_cols, 3);
        assert_eq!(deserialized.layout_align, Align::Center);
        assert_eq!(deserialized.layout_justify, Justify::SpaceBetween);
        assert_eq!(deserialized.layout_padding, [4.0, 8.0, 4.0, 8.0]);
        assert_eq!(deserialized.width_policy, SizePolicy::Percent(50.0));
        assert_eq!(deserialized.height_policy, SizePolicy::Fill);
        assert_eq!(deserialized.align_self, Some(Align::Stretch));
    }

    #[test]
    fn test_responsive_types_serde_and_logic() {
        let props = WidgetProps {
            tab_page: Some(2),
            auto_size_y: false,
            responsive_vis: ResponsiveVisibility::PortraitOnly,
            responsive_layout: ResponsiveLayout::RowToColumnOnPortrait,
            ..Default::default()
        };

        let json = serde_json::to_string(&props).unwrap();
        let deserialized: WidgetProps = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tab_page, Some(2));
        assert!(!deserialized.auto_size_y);
        assert_eq!(deserialized.responsive_vis, ResponsiveVisibility::PortraitOnly);
        assert_eq!(deserialized.responsive_layout, ResponsiveLayout::RowToColumnOnPortrait);

        // Test visibility logic
        let portrait = egui::vec2(390.0, 844.0);
        let landscape = egui::vec2(844.0, 390.0);
        assert!(ResponsiveVisibility::PortraitOnly.is_visible(portrait));
        assert!(!ResponsiveVisibility::PortraitOnly.is_visible(landscape));
        assert!(!ResponsiveVisibility::LandscapeOnly.is_visible(portrait));
        assert!(ResponsiveVisibility::LandscapeOnly.is_visible(landscape));

        // Test responsive layout logic
        assert_eq!(
            ResponsiveLayout::RowToColumnOnPortrait.resolve_mode(LayoutMode::Row, portrait),
            LayoutMode::Column
        );
        assert_eq!(
            ResponsiveLayout::RowToColumnOnPortrait.resolve_mode(LayoutMode::Row, landscape),
            LayoutMode::Row
        );
    }

    #[test]
    fn test_container_widget_kind() {
        let kind = WidgetKind::Container;
        assert_eq!(kind.category(), WidgetCategory::Containers);
        assert!(kind.is_container());
        assert_eq!(kind.display_name(), "Container");
        assert_eq!(kind.default_size(), egui::vec2(200.0, 150.0));
        let props = kind.default_props();
        assert_eq!(props.layout_mode, LayoutMode::Column);
        assert_eq!(props.layout_gap, 8.0);
        assert!(!props.auto_size_y);
    }

    #[test]
    fn test_visual_styling_props_serde_round_trip() {
        let props = WidgetProps {
            bg_color: Some([24, 28, 36, 255]),
            text_color: Some([240, 240, 255, 255]),
            font_size: Some(18.0),
            font_bold: true,
            border_color: Some([70, 80, 100, 255]),
            border_width: Some(2.0),
            corner_radius: Some(8.0),
            ..Default::default()
        };

        let json = serde_json::to_string(&props).unwrap();
        let deserialized: WidgetProps = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bg_color, Some([24, 28, 36, 255]));
        assert_eq!(deserialized.text_color, Some([240, 240, 255, 255]));
        assert_eq!(deserialized.font_size, Some(18.0));
        assert!(deserialized.font_bold);
        assert_eq!(deserialized.border_color, Some([70, 80, 100, 255]));
        assert_eq!(deserialized.border_width, Some(2.0));
        assert_eq!(deserialized.corner_radius, Some(8.0));
    }
}
