//! Built-in example projects, accessible via File -> Examples.
//!
//! Each function returns a ready-to-use `Project` with widgets configured to
//! demonstrate a common mobile / desktop UI pattern.

use egui::{pos2, vec2};

use crate::{
    project::{Project, ScreenPreset},
    widget::{
        ActionEffect, ActionTrigger, DockArea, LayoutMode, ResponsiveLayout, ResponsiveVisibility,
        SizePolicy, Widget, WidgetAction, WidgetId, WidgetKind,
    },
};

// ─── Helper to build a Widget ─────────────────────────────────────────────────

fn w(id: u64, kind: WidgetKind, pos: egui::Pos2, size: egui::Vec2) -> Widget {
    let props = kind.default_props();
    Widget {
        id: WidgetId::new(id),
        kind,
        pos,
        size,
        z: id as i32,
        area: DockArea::Center,
        props,
        parent: None,
    }
}

fn child(id: u64, kind: WidgetKind, pos: egui::Pos2, size: egui::Vec2, parent: u64) -> Widget {
    let props = kind.default_props();
    Widget {
        id: WidgetId::new(id),
        kind,
        pos,
        size,
        z: id as i32,
        area: DockArea::Center,
        props,
        parent: Some(WidgetId::new(parent)),
    }
}

fn action(trigger: ActionTrigger, effect: ActionEffect) -> WidgetAction {
    WidgetAction { trigger, effect }
}

// ── 01 – Mobile Navigation & Multi-View ───────────────────────────────────────

/// Classic mobile app with responsive adaptive navigation:
/// - In Portrait (vertical 390x844): Bottom navigation bar in Row layout.
/// - In Landscape (horizontal 844x390): Left navigation rail in Column layout.
/// - Multi-view container (TabBar as ViewStack) with Home, Detail, and Profile screens.
pub(crate) fn mobile_navigation_example() -> Project {
    let mut project = Project {
        screen_preset: ScreenPreset::MobilePortrait,
        canvas_size: vec2(390.0, 844.0),
        root_layout_mode: LayoutMode::Column,
        root_layout_gap: 0.0,
        root_layout_padding: [0.0; 4],
        root_responsive_layout: ResponsiveLayout::RowToColumnOnPortrait,
        ..Default::default()
    };

    // ── Root layout widgets ────────────────────────────────────────────────────
    // 1) Left Navigation Rail (visible only in Landscape)
    let mut left_rail = w(4, WidgetKind::Container, pos2(0.0, 0.0), vec2(80.0, 390.0));
    left_rail.props.name = "LeftNavRail".into();
    left_rail.area = DockArea::Left;
    left_rail.props.responsive_vis = ResponsiveVisibility::LandscapeOnly;
    left_rail.props.width_policy = SizePolicy::Fixed;
    left_rail.props.height_policy = SizePolicy::Fill;
    left_rail.props.layout_mode = LayoutMode::Column;
    left_rail.props.layout_gap = 12.0;
    left_rail.props.layout_padding = [16.0, 8.0, 16.0, 8.0];
    left_rail.props.bg_color = Some([24, 24, 30, 255]);
    left_rail.props.border_color = Some([45, 45, 60, 255]);
    left_rail.props.border_width = Some(1.0);

    // 2) Top App Bar (visible in both orientations)
    let mut top_bar = w(1, WidgetKind::Container, pos2(0.0, 0.0), vec2(390.0, 56.0));
    top_bar.props.name = "TopBar".into();
    top_bar.area = DockArea::Top;
    top_bar.props.horizontal = true;
    top_bar.props.layout_mode = LayoutMode::Row;
    top_bar.props.layout_gap = 8.0;
    top_bar.props.layout_padding = [12.0, 16.0, 12.0, 16.0];
    top_bar.props.width_policy = SizePolicy::Fill;
    top_bar.props.height_policy = SizePolicy::Fixed;
    top_bar.props.bg_color = Some([28, 28, 36, 255]);

    // 3) View Container (TabBar as ViewStack — no tab buttons)
    let mut views = w(2, WidgetKind::TabBar, pos2(0.0, 56.0), vec2(390.0, 732.0));
    views.props.name = "Views".into();
    views.area = DockArea::Center;
    views.props.show_tabs = false;
    views.props.items = vec!["Home".into(), "Detail".into(), "Profile".into()];
    views.props.selected = 0;
    views.props.width_policy = SizePolicy::Fill;
    views.props.height_policy = SizePolicy::Fill;

    // 4) Bottom Navigation Bar (visible only in Portrait)
    let mut bottom = w(3, WidgetKind::Container, pos2(0.0, 788.0), vec2(390.0, 56.0));
    bottom.props.name = "BottomNav".into();
    bottom.area = DockArea::Bottom;
    bottom.props.responsive_vis = ResponsiveVisibility::PortraitOnly;
    bottom.props.horizontal = true;
    bottom.props.layout_mode = LayoutMode::Row;
    bottom.props.layout_gap = 0.0;
    bottom.props.layout_padding = [6.0, 8.0, 6.0, 8.0];
    bottom.props.layout_justify = crate::widget::Justify::SpaceEvenly;
    bottom.props.width_policy = SizePolicy::Fill;
    bottom.props.height_policy = SizePolicy::Fixed;
    bottom.props.bg_color = Some([24, 24, 30, 255]);
    bottom.props.border_color = Some([45, 45, 60, 255]);
    bottom.props.border_width = Some(1.0);

    project.widgets.push(left_rail);
    project.widgets.push(top_bar);
    project.widgets.push(views);
    project.widgets.push(bottom);

    // ── Top bar children ──────────────────────────────────────────────────────
    let mut app_title = child(10, WidgetKind::Heading, pos2(0.0, 0.0), vec2(200.0, 28.0), 1);
    app_title.props.text = "My App".into();
    app_title.props.name = "AppTitle".into();
    app_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(app_title);

    // ── View 0: Home ──────────────────────────────────────────────────────────
    let mut home_group = child(20, WidgetKind::Container, pos2(0.0, 0.0), vec2(374.0, 700.0), 2);
    home_group.props.name = "HomeView".into();
    home_group.props.layout_mode = LayoutMode::Column;
    home_group.props.layout_gap = 16.0;
    home_group.props.layout_padding = [16.0, 16.0, 16.0, 16.0];
    home_group.props.width_policy = SizePolicy::Fill;
    home_group.props.height_policy = SizePolicy::Fill;
    home_group.props.tab_page = Some(0);
    project.widgets.push(home_group);

    let mut home_title = child(21, WidgetKind::Heading, pos2(0.0, 0.0), vec2(358.0, 32.0), 20);
    home_title.props.text = "Home".into();
    home_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(home_title);

    let mut card1 = child(22, WidgetKind::Container, pos2(0.0, 0.0), vec2(358.0, 140.0), 20);
    card1.props.name = "FeaturedCard".into();
    card1.props.bg_color = Some([32, 60, 110, 255]);
    card1.props.corner_radius = Some(8.0);
    card1.props.layout_mode = LayoutMode::Column;
    card1.props.layout_padding = [16.0, 16.0, 16.0, 16.0];
    card1.props.width_policy = SizePolicy::Fill;
    project.widgets.push(card1);

    let mut card1_lbl = child(25, WidgetKind::Heading, pos2(0.0, 0.0), vec2(326.0, 28.0), 22);
    card1_lbl.props.text = "Featured Content".into();
    project.widgets.push(card1_lbl);

    let mut card2 = child(23, WidgetKind::Container, pos2(0.0, 0.0), vec2(358.0, 80.0), 20);
    card2.props.name = "ArticleCard".into();
    card2.props.bg_color = Some([28, 28, 36, 255]);
    card2.props.border_color = Some([50, 50, 65, 255]);
    card2.props.border_width = Some(1.0);
    card2.props.corner_radius = Some(6.0);
    card2.props.layout_mode = LayoutMode::Column;
    card2.props.layout_padding = [12.0, 12.0, 12.0, 12.0];
    card2.props.width_policy = SizePolicy::Fill;
    project.widgets.push(card2);

    let mut card2_lbl = child(26, WidgetKind::Label, pos2(0.0, 0.0), vec2(334.0, 24.0), 23);
    card2_lbl.props.text = "Article Item (tap See Details below)".into();
    project.widgets.push(card2_lbl);

    // Button: go to Detail view
    let mut see_details = child(24, WidgetKind::Button, pos2(0.0, 0.0), vec2(160.0, 40.0), 20);
    see_details.props.text = "See Details".into();
    see_details.props.bg_color = Some([40, 90, 180, 255]);
    see_details.props.corner_radius = Some(6.0);
    see_details.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: 1 },
    )];
    project.widgets.push(see_details);

    // ── View 1: Detail ────────────────────────────────────────────────────────
    let mut detail_group = child(30, WidgetKind::Container, pos2(0.0, 0.0), vec2(374.0, 700.0), 2);
    detail_group.props.name = "DetailView".into();
    detail_group.props.layout_mode = LayoutMode::Column;
    detail_group.props.layout_gap = 16.0;
    detail_group.props.layout_padding = [16.0, 16.0, 16.0, 16.0];
    detail_group.props.width_policy = SizePolicy::Fill;
    detail_group.props.height_policy = SizePolicy::Fill;
    detail_group.props.tab_page = Some(1);
    project.widgets.push(detail_group);

    let mut detail_title = child(31, WidgetKind::Heading, pos2(0.0, 0.0), vec2(358.0, 32.0), 30);
    detail_title.props.text = "Item Details".into();
    detail_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(detail_title);

    let mut detail_img = child(32, WidgetKind::Placeholder, pos2(0.0, 0.0), vec2(358.0, 200.0), 30);
    detail_img.props.text = "Hero Image".into();
    detail_img.props.color = [40, 100, 180, 160];
    detail_img.props.width_policy = SizePolicy::Fill;
    project.widgets.push(detail_img);

    let mut detail_desc = child(33, WidgetKind::Label, pos2(0.0, 0.0), vec2(358.0, 64.0), 30);
    detail_desc.props.text = "Full item description goes here. Responsive navigation automatically adapts between portrait and landscape.".into();
    detail_desc.props.width_policy = SizePolicy::Fill;
    project.widgets.push(detail_desc);

    // Back button
    let mut back_btn = child(34, WidgetKind::Button, pos2(0.0, 0.0), vec2(160.0, 40.0), 30);
    back_btn.props.text = "Back to Home".into();
    back_btn.props.bg_color = Some([50, 50, 65, 255]);
    back_btn.props.corner_radius = Some(6.0);
    back_btn.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: 0 },
    )];
    project.widgets.push(back_btn);

    // ── View 2: Profile ───────────────────────────────────────────────────────
    let mut profile_group = child(40, WidgetKind::Container, pos2(0.0, 0.0), vec2(374.0, 700.0), 2);
    profile_group.props.name = "ProfileView".into();
    profile_group.props.layout_mode = LayoutMode::Column;
    profile_group.props.layout_gap = 12.0;
    profile_group.props.layout_padding = [16.0, 16.0, 16.0, 16.0];
    profile_group.props.width_policy = SizePolicy::Fill;
    profile_group.props.height_policy = SizePolicy::Fill;
    profile_group.props.tab_page = Some(2);
    project.widgets.push(profile_group);

    let mut profile_title = child(41, WidgetKind::Heading, pos2(0.0, 0.0), vec2(358.0, 32.0), 40);
    profile_title.props.text = "Profile".into();
    profile_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(profile_title);

    let mut avatar = child(42, WidgetKind::Placeholder, pos2(0.0, 0.0), vec2(80.0, 80.0), 40);
    avatar.props.text = "Avatar".into();
    avatar.props.color = [100, 100, 200, 200];
    project.widgets.push(avatar);

    let mut username = child(43, WidgetKind::Label, pos2(0.0, 0.0), vec2(358.0, 28.0), 40);
    username.props.text = "John Doe".into();
    username.props.width_policy = SizePolicy::Fill;
    project.widgets.push(username);

    let mut dark_mode = child(44, WidgetKind::Checkbox, pos2(0.0, 0.0), vec2(200.0, 28.0), 40);
    dark_mode.props.text = "Dark mode".into();
    dark_mode.props.checked = false;
    project.widgets.push(dark_mode);

    let mut notifs = child(45, WidgetKind::Checkbox, pos2(0.0, 0.0), vec2(200.0, 28.0), 40);
    notifs.props.text = "Notifications".into();
    notifs.props.checked = true;
    project.widgets.push(notifs);

    // ── Bottom nav buttons (in a Row for Portrait) ───────────────────────────
    for (id, label, tab_idx) in [(50u64, "Home", 0usize), (51, "Details", 1), (52, "Profile", 2)] {
        let mut btn = child(id, WidgetKind::Button, pos2(0.0, 0.0), vec2(110.0, 44.0), 3);
        btn.props.text = label.into();
        btn.props.height_policy = SizePolicy::Fill;
        btn.props.width_policy = SizePolicy::Fill;
        btn.props.actions = vec![action(
            ActionTrigger::OnClick,
            ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: tab_idx },
        )];
        project.widgets.push(btn);
    }

    // ── Left nav rail buttons (in a Column for Landscape) ─────────────────────
    for (id, label, tab_idx) in [(60u64, "Home", 0usize), (61, "Details", 1), (62, "Profile", 2)] {
        let mut btn = child(id, WidgetKind::Button, pos2(0.0, 0.0), vec2(64.0, 44.0), 4);
        btn.props.text = label.into();
        btn.props.width_policy = SizePolicy::Fill;
        btn.props.actions = vec![action(
            ActionTrigger::OnClick,
            ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: tab_idx },
        )];
        project.widgets.push(btn);
    }

    project
}

// ── 02 – Hamburger Drawer (Side Menu) ─────────────────────────────────────────

/// Mobile/desktop app with a hamburger button that opens a side navigation drawer.
pub(crate) fn hamburger_drawer_example() -> Project {
    let mut project = Project {
        screen_preset: ScreenPreset::MobilePortrait,
        canvas_size: vec2(390.0, 844.0),
        root_layout_mode: LayoutMode::Free,
        root_layout_padding: [0.0; 4],
        ..Default::default()
    };

    // ── Header bar ────────────────────────────────────────────────────────────
    let mut header = w(1, WidgetKind::Container, pos2(0.0, 0.0), vec2(390.0, 56.0));
    header.props.name = "Header".into();
    header.props.horizontal = true;
    header.props.layout_mode = LayoutMode::Row;
    header.props.layout_gap = 8.0;
    header.props.layout_padding = [8.0, 16.0, 8.0, 8.0];
    header.props.bg_color = Some([28, 28, 36, 255]);
    project.widgets.push(header);

    // Hamburger button
    let mut hamburger = child(10, WidgetKind::Button, pos2(0.0, 0.0), vec2(44.0, 40.0), 1);
    hamburger.props.text = "=".into();
    hamburger.props.name = "HamburgerBtn".into();
    hamburger.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::ToggleWidget(WidgetId::new(3)),
    )];
    project.widgets.push(hamburger);

    let mut title = child(11, WidgetKind::Heading, pos2(0.0, 0.0), vec2(200.0, 28.0), 1);
    title.props.text = "My App".into();
    title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(title);

    // ── Main content area ─────────────────────────────────────────────────────
    let mut content = w(2, WidgetKind::Container, pos2(0.0, 56.0), vec2(390.0, 788.0));
    content.props.name = "MainContent".into();
    content.props.layout_mode = LayoutMode::Column;
    content.props.layout_gap = 16.0;
    content.props.layout_padding = [24.0, 24.0, 24.0, 24.0];
    project.widgets.push(content);

    let mut page_title = child(20, WidgetKind::Heading, pos2(0.0, 0.0), vec2(342.0, 32.0), 2);
    page_title.props.text = "Dashboard".into();
    page_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(page_title);

    let mut card = child(21, WidgetKind::Placeholder, pos2(0.0, 0.0), vec2(342.0, 200.0), 2);
    card.props.text = "Main Content Area".into();
    card.props.color = [60, 80, 140, 160];
    card.props.width_policy = SizePolicy::Fill;
    project.widgets.push(card);

    let mut desc = child(22, WidgetKind::Label, pos2(0.0, 0.0), vec2(342.0, 48.0), 2);
    desc.props.text = "Press the hamburger button ( = ) in the top bar to open the side menu.".into();
    desc.props.width_policy = SizePolicy::Fill;
    project.widgets.push(desc);

    // ── Drawer (initially hidden, full screen height, solid surface) ───────────
    let mut drawer = w(3, WidgetKind::Container, pos2(0.0, 0.0), vec2(280.0, 844.0));
    drawer.props.name = "SideDrawer".into();
    drawer.props.layout_mode = LayoutMode::Column;
    drawer.props.layout_gap = 8.0;
    drawer.props.layout_padding = [16.0, 16.0, 16.0, 16.0];
    drawer.props.auto_size_y = false;
    drawer.props.bg_color = Some([24, 24, 30, 255]);
    drawer.props.border_color = Some([50, 50, 65, 255]);
    drawer.props.border_width = Some(1.5);
    drawer.props.initially_visible = false;
    project.widgets.push(drawer);

    let mut drawer_title = child(30, WidgetKind::Heading, pos2(0.0, 0.0), vec2(248.0, 32.0), 3);
    drawer_title.props.text = "Menu".into();
    drawer_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(drawer_title);

    let mut close_btn = child(31, WidgetKind::Button, pos2(0.0, 0.0), vec2(80.0, 36.0), 3);
    close_btn.props.text = "Close X".into();
    close_btn.props.name = "CloseDrawer".into();
    close_btn.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::HideWidget(WidgetId::new(3)),
    )];
    project.widgets.push(close_btn);

    let mut sep = child(32, WidgetKind::Separator, pos2(0.0, 0.0), vec2(248.0, 8.0), 3);
    sep.props.width_policy = SizePolicy::Fill;
    project.widgets.push(sep);

    for (id, label) in [(33u64, "Home"), (34, "Explore"), (35, "Notifications"), (36, "Settings")] {
        let mut item = child(id, WidgetKind::Button, pos2(0.0, 0.0), vec2(248.0, 44.0), 3);
        item.props.text = label.into();
        item.props.width_policy = SizePolicy::Fill;
        // Close drawer when a menu item is clicked
        item.props.actions = vec![action(
            ActionTrigger::OnClick,
            ActionEffect::HideWidget(WidgetId::new(3)),
        )];
        project.widgets.push(item);
    }

    project
}

// ── 03 – Adaptive Responsive Screen (Auto-Rotate) ─────────────────────────────

/// Demonstrates RowToColumnOnPortrait: portrait shows a single column,
/// landscape rotates to a side-by-side two-column layout automatically.
pub(crate) fn responsive_adaptive_example() -> Project {
    let mut project = Project {
        screen_preset: ScreenPreset::MobilePortrait,
        canvas_size: vec2(390.0, 844.0),
        root_layout_mode: LayoutMode::Column,
        root_layout_gap: 16.0,
        root_layout_padding: [16.0, 16.0, 16.0, 16.0],
        root_responsive_layout: ResponsiveLayout::RowToColumnOnPortrait,
        ..Default::default()
    };

    // Hero banner
    let mut hero = w(1, WidgetKind::Placeholder, pos2(0.0, 0.0), vec2(358.0, 160.0));
    hero.props.text = "Hero Banner".into();
    hero.props.color = [30, 80, 180, 200];
    hero.props.name = "Hero".into();
    hero.props.width_policy = SizePolicy::Fill;
    project.widgets.push(hero);

    // Sidebar (visible in landscape as a column; in portrait appears above content)
    let mut sidebar = w(2, WidgetKind::Group, pos2(0.0, 0.0), vec2(200.0, 400.0));
    sidebar.props.text = "Sidebar".into();
    sidebar.props.name = "Sidebar".into();
    sidebar.props.layout_mode = LayoutMode::Column;
    sidebar.props.layout_gap = 8.0;
    sidebar.props.layout_padding = [8.0, 8.0, 8.0, 8.0];
    sidebar.props.height_policy = SizePolicy::Fill;
    project.widgets.push(sidebar);

    for (id, label) in [(10u64, "Section A"), (11, "Section B"), (12, "Section C")] {
        let mut item = child(id, WidgetKind::Button, pos2(0.0, 0.0), vec2(184.0, 40.0), 2);
        item.props.text = label.into();
        item.props.width_policy = SizePolicy::Fill;
        project.widgets.push(item);
    }

    // Main content
    let mut main = w(3, WidgetKind::Group, pos2(0.0, 0.0), vec2(358.0, 400.0));
    main.props.text = "Main Content".into();
    main.props.name = "MainContent".into();
    main.props.layout_mode = LayoutMode::Column;
    main.props.layout_gap = 12.0;
    main.props.layout_padding = [12.0, 12.0, 12.0, 12.0];
    main.props.width_policy = SizePolicy::Fill;
    main.props.height_policy = SizePolicy::Fill;
    project.widgets.push(main);

    let mut headline = child(20, WidgetKind::Heading, pos2(0.0, 0.0), vec2(334.0, 32.0), 3);
    headline.props.text = "Article Title".into();
    headline.props.width_policy = SizePolicy::Fill;
    project.widgets.push(headline);

    let mut body = child(21, WidgetKind::Label, pos2(0.0, 0.0), vec2(334.0, 80.0), 3);
    body.props.text = "In portrait, all sections stack vertically. Rotate the screen (use the Rotate button in the Inspector) to see the sidebar and content side by side.".into();
    body.props.width_policy = SizePolicy::Fill;
    project.widgets.push(body);

    let mut progress = child(22, WidgetKind::ProgressBar, pos2(0.0, 0.0), vec2(334.0, 20.0), 3);
    progress.props.value = 0.65;
    progress.props.width_policy = SizePolicy::Fill;
    project.widgets.push(progress);

    project
}

// ── 04 – Multi-Step Form Wizard ────────────────────────────────────────────────

/// A 3-step form wizard with Next and Back navigation.
pub(crate) fn multi_step_wizard_example() -> Project {
    let mut project = Project {
        screen_preset: ScreenPreset::MobilePortrait,
        canvas_size: vec2(390.0, 844.0),
        root_layout_mode: LayoutMode::Column,
        root_layout_gap: 0.0,
        root_layout_padding: [0.0; 4],
        ..Default::default()
    };

    // Progress bar header
    let mut header = w(1, WidgetKind::Group, pos2(0.0, 0.0), vec2(390.0, 60.0));
    header.props.text = String::new();
    header.props.name = "WizardHeader".into();
    header.props.layout_mode = LayoutMode::Column;
    header.props.layout_gap = 4.0;
    header.props.layout_padding = [8.0, 16.0, 8.0, 16.0];
    header.props.width_policy = SizePolicy::Fill;
    header.props.height_policy = SizePolicy::Fixed;
    project.widgets.push(header);

    let mut step_label = child(10, WidgetKind::Label, pos2(0.0, 0.0), vec2(358.0, 20.0), 1);
    step_label.props.text = "Step 1 of 3".into();
    step_label.props.width_policy = SizePolicy::Fill;
    project.widgets.push(step_label);

    let mut progress_bar = child(11, WidgetKind::ProgressBar, pos2(0.0, 0.0), vec2(358.0, 12.0), 1);
    progress_bar.props.value = 0.33;
    progress_bar.props.width_policy = SizePolicy::Fill;
    project.widgets.push(progress_bar);

    // ViewStack for the 3 steps
    let mut steps = w(2, WidgetKind::TabBar, pos2(0.0, 60.0), vec2(390.0, 724.0));
    steps.props.name = "WizardSteps".into();
    steps.props.show_tabs = false;
    steps.props.items = vec!["Personal Info".into(), "Configuration".into(), "Summary".into()];
    steps.props.selected = 0;
    steps.props.width_policy = SizePolicy::Fill;
    steps.props.height_policy = SizePolicy::Fill;
    project.widgets.push(steps);

    // ── Step 0: Personal Info ─────────────────────────────────────────────────
    let mut step0 = child(20, WidgetKind::Group, pos2(0.0, 0.0), vec2(374.0, 680.0), 2);
    step0.props.text = String::new();
    step0.props.name = "Step1_PersonalInfo".into();
    step0.props.layout_mode = LayoutMode::Column;
    step0.props.layout_gap = 16.0;
    step0.props.layout_padding = [24.0, 24.0, 24.0, 24.0];
    step0.props.width_policy = SizePolicy::Fill;
    step0.props.height_policy = SizePolicy::Fill;
    step0.props.tab_page = Some(0);
    project.widgets.push(step0);

    let mut s0_title = child(21, WidgetKind::Heading, pos2(0.0, 0.0), vec2(326.0, 32.0), 20);
    s0_title.props.text = "Personal Information".into();
    s0_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s0_title);

    let mut name_field = child(22, WidgetKind::TextEdit, pos2(0.0, 0.0), vec2(326.0, 36.0), 20);
    name_field.props.text = "Full name".into();
    name_field.props.width_policy = SizePolicy::Fill;
    project.widgets.push(name_field);

    let mut email_field = child(23, WidgetKind::TextEdit, pos2(0.0, 0.0), vec2(326.0, 36.0), 20);
    email_field.props.text = "Email address".into();
    email_field.props.width_policy = SizePolicy::Fill;
    project.widgets.push(email_field);

    let mut phone_field = child(24, WidgetKind::TextEdit, pos2(0.0, 0.0), vec2(326.0, 36.0), 20);
    phone_field.props.text = "Phone number".into();
    phone_field.props.width_policy = SizePolicy::Fill;
    project.widgets.push(phone_field);

    let mut s0_next = child(25, WidgetKind::Button, pos2(0.0, 0.0), vec2(160.0, 44.0), 20);
    s0_next.props.text = "Next Step".into();
    s0_next.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: 1 },
    )];
    project.widgets.push(s0_next);

    // ── Step 1: Configuration ─────────────────────────────────────────────────
    let mut step1 = child(30, WidgetKind::Group, pos2(0.0, 0.0), vec2(374.0, 680.0), 2);
    step1.props.text = String::new();
    step1.props.name = "Step2_Config".into();
    step1.props.layout_mode = LayoutMode::Column;
    step1.props.layout_gap = 16.0;
    step1.props.layout_padding = [24.0, 24.0, 24.0, 24.0];
    step1.props.width_policy = SizePolicy::Fill;
    step1.props.height_policy = SizePolicy::Fill;
    step1.props.tab_page = Some(1);
    project.widgets.push(step1);

    let mut s1_title = child(31, WidgetKind::Heading, pos2(0.0, 0.0), vec2(326.0, 32.0), 30);
    s1_title.props.text = "Configuration".into();
    s1_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s1_title);

    let mut plan_combo = child(32, WidgetKind::ComboBox, pos2(0.0, 0.0), vec2(326.0, 28.0), 30);
    plan_combo.props.text = "Select plan".into();
    plan_combo.props.items = vec!["Basic".into(), "Professional".into(), "Enterprise".into()];
    plan_combo.props.width_policy = SizePolicy::Fill;
    project.widgets.push(plan_combo);

    let mut notif_cb = child(33, WidgetKind::Checkbox, pos2(0.0, 0.0), vec2(280.0, 28.0), 30);
    notif_cb.props.text = "Enable email notifications".into();
    notif_cb.props.checked = true;
    project.widgets.push(notif_cb);

    let mut s1_volume = child(34, WidgetKind::Slider, pos2(0.0, 0.0), vec2(326.0, 28.0), 30);
    s1_volume.props.text = "Storage (GB)".into();
    s1_volume.props.min = 1.0;
    s1_volume.props.max = 100.0;
    s1_volume.props.value = 20.0;
    s1_volume.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s1_volume);

    let mut s1_row = child(35, WidgetKind::Group, pos2(0.0, 0.0), vec2(326.0, 50.0), 30);
    s1_row.props.text = String::new();
    s1_row.props.layout_mode = LayoutMode::Row;
    s1_row.props.layout_gap = 12.0;
    s1_row.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s1_row);

    let mut s1_back = child(36, WidgetKind::Button, pos2(0.0, 0.0), vec2(140.0, 44.0), 35);
    s1_back.props.text = "Back".into();
    s1_back.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: 0 },
    )];
    project.widgets.push(s1_back);

    let mut s1_next = child(37, WidgetKind::Button, pos2(0.0, 0.0), vec2(160.0, 44.0), 35);
    s1_next.props.text = "Next Step".into();
    s1_next.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: 2 },
    )];
    project.widgets.push(s1_next);

    // ── Step 2: Summary ───────────────────────────────────────────────────────
    let mut step2 = child(40, WidgetKind::Group, pos2(0.0, 0.0), vec2(374.0, 680.0), 2);
    step2.props.text = String::new();
    step2.props.name = "Step3_Summary".into();
    step2.props.layout_mode = LayoutMode::Column;
    step2.props.layout_gap = 16.0;
    step2.props.layout_padding = [24.0, 24.0, 24.0, 24.0];
    step2.props.width_policy = SizePolicy::Fill;
    step2.props.height_policy = SizePolicy::Fill;
    step2.props.tab_page = Some(2);
    project.widgets.push(step2);

    let mut s2_title = child(41, WidgetKind::Heading, pos2(0.0, 0.0), vec2(326.0, 32.0), 40);
    s2_title.props.text = "Summary".into();
    s2_title.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s2_title);

    let mut s2_desc = child(42, WidgetKind::Label, pos2(0.0, 0.0), vec2(326.0, 60.0), 40);
    s2_desc.props.text = "Please review your information before submitting. All fields are ready.".into();
    s2_desc.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s2_desc);

    let mut s2_card = child(43, WidgetKind::Placeholder, pos2(0.0, 0.0), vec2(326.0, 120.0), 40);
    s2_card.props.text = "Review Summary".into();
    s2_card.props.color = [50, 150, 80, 160];
    s2_card.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s2_card);

    let mut s2_row = child(44, WidgetKind::Group, pos2(0.0, 0.0), vec2(326.0, 50.0), 40);
    s2_row.props.text = String::new();
    s2_row.props.layout_mode = LayoutMode::Row;
    s2_row.props.layout_gap = 12.0;
    s2_row.props.width_policy = SizePolicy::Fill;
    project.widgets.push(s2_row);

    let mut s2_back = child(45, WidgetKind::Button, pos2(0.0, 0.0), vec2(140.0, 44.0), 44);
    s2_back.props.text = "Back".into();
    s2_back.props.actions = vec![action(
        ActionTrigger::OnClick,
        ActionEffect::SwitchTab { target: WidgetId::new(2), tab_index: 1 },
    )];
    project.widgets.push(s2_back);

    let mut s2_submit = child(46, WidgetKind::Button, pos2(0.0, 0.0), vec2(160.0, 44.0), 44);
    s2_submit.props.text = "Submit".into();
    project.widgets.push(s2_submit);

    project
}
