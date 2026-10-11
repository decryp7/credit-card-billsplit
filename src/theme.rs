//! Look and feel matching https://decryptology.net: a near-black navy night, warm cream text,
//! gold accents and hairlines, and Cormorant Garamond for headings.

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Mesh, Pos2, Rect,
    RichText, Shadow, Stroke, TextStyle, Theme, ThemePreference, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(0x07, 0x08, 0x0d);
pub const NAVY: Color32 = Color32::from_rgb(0x08, 0x0b, 0x18);
pub const SURFACE: Color32 = Color32::from_rgb(0x0c, 0x0e, 0x16);
pub const RAISED: Color32 = Color32::from_rgb(0x14, 0x17, 0x24);

pub const INK: Color32 = Color32::from_rgb(0xf3, 0xe7, 0xcf);
pub const INK_BRIGHT: Color32 = Color32::from_rgb(0xf6, 0xea, 0xd0);
pub const BODY: Color32 = Color32::from_rgb(0xd8, 0xcb, 0xb2);
pub const MUTED: Color32 = Color32::from_rgb(0xb9, 0xa9, 0x88);
pub const FAINT: Color32 = Color32::from_rgb(0x8f, 0x84, 0x70);

pub const GOLD: Color32 = Color32::from_rgb(0xe2, 0xb3, 0x5c);
pub const GOLD_LIGHT: Color32 = Color32::from_rgb(0xf3, 0xdc, 0xa6);
pub const GOLD_SOFT: Color32 = Color32::from_rgb(0xe2, 0xc9, 0x8f);
pub const ON_GOLD: Color32 = Color32::from_rgb(0x1a, 0x12, 0x06);

pub const CREDIT: Color32 = Color32::from_rgb(0x8f, 0xd1, 0x9e);
pub const WARN: Color32 = Color32::from_rgb(0xf0, 0xa3, 0x5c);
pub const ERROR: Color32 = Color32::from_rgb(0xe5, 0x7f, 0x73);

/// Inter SemiBold, for text that needs a little more weight than the body.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Font family for headings and large figures.
pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}

/// Italic variant of [`serif`], for asides such as the file name.
pub fn serif_italic() -> FontFamily {
    FontFamily::Name("serif-italic".into())
}

/// `color` with the given opacity (0–255), like the `#e2b35c33` borders on the site.
pub fn alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

/// Installs the fonts and the dark gold style. Call once at startup.
pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "cormorant-semibold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/CormorantGaramond-SemiBold.ttf")).into(),
    );
    fonts.font_data.insert(
        "cormorant-italic".into(),
        FontData::from_static(include_bytes!("../assets/fonts/CormorantGaramond-MediumItalic.ttf")).into(),
    );
    // Inter is much easier to read on a dark background than egui's default Ubuntu Light.
    fonts.font_data.insert(
        "inter".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")).into(),
    );
    // Fall back to the default fonts for symbols and emoji these fonts don't have.
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "inter".into());
    for (family, font) in [
        (semibold(), "inter-semibold"),
        (serif(), "cormorant-semibold"),
        (serif_italic(), "cormorant-italic"),
    ] {
        let mut list = vec![font.to_string()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(family, list);
    }
    ctx.set_fonts(fonts);

    // The site is dark only, so the app is too.
    ctx.set_theme(ThemePreference::Dark);
    ctx.set_style_of(Theme::Dark, style());
}

fn style() -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (TextStyle::Heading, FontId::new(30.0, serif())),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Monospace, FontId::monospace(13.5)),
        ]
        .into(),
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };

    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::new(8.0, 8.0);
    spacing.button_padding = Vec2::new(14.0, 5.0);
    spacing.interact_size.y = 28.0;
    spacing.menu_margin = Margin::same(8);

    let v = &mut style.visuals;
    v.panel_fill = NAVY;
    v.window_fill = SURFACE;
    v.window_stroke = Stroke::new(1.0, alpha(GOLD, 0x40));
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow { offset: [0, 12], blur: 32, spread: 0, color: Color32::from_black_alpha(150) };
    v.popup_shadow = Shadow { offset: [0, 8], blur: 20, spread: 0, color: Color32::from_black_alpha(140) };
    v.extreme_bg_color = BG;
    v.text_edit_bg_color = Some(BG);
    v.faint_bg_color = Color32::from_rgb(0x0d, 0x10, 0x1d);
    v.code_bg_color = RAISED;
    v.hyperlink_color = GOLD;
    v.warn_fg_color = WARN;
    v.error_fg_color = ERROR;
    v.weak_text_color = Some(MUTED);
    v.selection.bg_fill = alpha(GOLD, 0x55);
    v.selection.stroke = Stroke::new(1.0, GOLD);
    v.text_cursor.stroke = Stroke::new(2.0, GOLD);

    let radius = CornerRadius::same(14);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, alpha(Color32::WHITE, 0x14));
    w.noninteractive.fg_stroke = Stroke::new(1.0, BODY);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = RAISED;
    w.inactive.weak_bg_fill = RAISED;
    w.inactive.bg_stroke = Stroke::new(1.0, alpha(Color32::WHITE, 0x26));
    w.inactive.fg_stroke = Stroke::new(1.0, INK);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = Color32::from_rgb(0x1b, 0x1e, 0x2c);
    w.hovered.weak_bg_fill = Color32::from_rgb(0x1b, 0x1e, 0x2c);
    w.hovered.bg_stroke = Stroke::new(1.0, GOLD);
    w.hovered.fg_stroke = Stroke::new(1.5, GOLD_LIGHT);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = alpha(GOLD, 0x33);
    w.active.weak_bg_fill = alpha(GOLD, 0x33);
    w.active.bg_stroke = Stroke::new(1.0, GOLD);
    w.active.fg_stroke = Stroke::new(2.0, INK_BRIGHT);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open = w.hovered;

    style
}

// ----------------------------------------------------------------------------
// Building blocks

/// Small, letter-spaced, uppercase caption in gold, like the site's `.eyebrow`.
pub fn eyebrow(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).size(10.5).extra_letter_spacing(2.6).color(GOLD)
}

/// Like [`eyebrow`] but in the muted tone, for secondary captions.
pub fn caption(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).size(10.5).extra_letter_spacing(2.0).color(MUTED)
}

/// Serif text in the given size, for headings and figures.
pub fn display(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, serif())).color(INK_BRIGHT)
}

/// The gold call-to-action pill.
pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).family(semibold()).color(ON_GOLD))
        .fill(GOLD)
        .stroke(Stroke::new(1.0, GOLD))
        .corner_radius(CornerRadius::same(255))
}

/// A softly lit panel with a faint gold border, like the site's highlight tiles.
pub fn tile_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(alpha(GOLD, 0x0a))
        .stroke(Stroke::new(1.0, alpha(GOLD, 0x33)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin { left: 16, right: 18, top: 12, bottom: 10 })
}

/// A quiet panel with a faint white border, for larger blocks of content.
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(alpha(Color32::WHITE, 0x06))
        .stroke(Stroke::new(1.0, alpha(Color32::WHITE, 0x14)))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(12))
}

/// A horizontal gold hairline that fades out at both ends.
pub fn paint_rule(painter: &egui::Painter, left: f32, right: f32, y: f32, color: Color32) {
    let clear = alpha(color, 0);
    let width = right - left;
    let stops = [(0.0, clear), (0.3, color), (0.7, color), (1.0, clear)];
    let mut mesh = Mesh::default();
    for pair in stops.windows(2) {
        let (a, ca) = pair[0];
        let (b, cb) = pair[1];
        let base = mesh.vertices.len() as u32;
        let (x0, x1) = (left + a * width, left + b * width);
        mesh.colored_vertex(Pos2::new(x0, y - 0.5), ca);
        mesh.colored_vertex(Pos2::new(x1, y - 0.5), cb);
        mesh.colored_vertex(Pos2::new(x1, y + 0.5), cb);
        mesh.colored_vertex(Pos2::new(x0, y + 0.5), ca);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(mesh);
}

/// The site's `.flourish`: two fading gold lines meeting at a small diamond.
pub fn flourish(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(200.0, 18.0), egui::Sense::hover());
    let c = rect.center();
    let painter = ui.painter();
    paint_fading_line(painter, c.x - 84.0, c.x - 12.0, c.y, false);
    paint_fading_line(painter, c.x + 12.0, c.x + 84.0, c.y, true);
    let r = 5.0;
    painter.add(egui::Shape::convex_polygon(
        vec![Pos2::new(c.x, c.y - r), Pos2::new(c.x + r, c.y), Pos2::new(c.x, c.y + r), Pos2::new(c.x - r, c.y)],
        Color32::TRANSPARENT,
        Stroke::new(1.2, GOLD),
    ));
    painter.circle_filled(c, 1.5, GOLD);
}

/// A gold line fading in from one end; `reverse` makes it fade out instead.
fn paint_fading_line(painter: &egui::Painter, left: f32, right: f32, y: f32, reverse: bool) {
    let (cl, cr) = if reverse { (GOLD, alpha(GOLD, 0)) } else { (alpha(GOLD, 0), GOLD) };
    let mut mesh = Mesh::default();
    mesh.colored_vertex(Pos2::new(left, y - 0.5), cl);
    mesh.colored_vertex(Pos2::new(right, y - 0.5), cr);
    mesh.colored_vertex(Pos2::new(right, y + 0.5), cr);
    mesh.colored_vertex(Pos2::new(left, y + 0.5), cl);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

/// A soft gold glow at the top of `rect`, like the radial wash behind the site's career section.
pub fn paint_glow(painter: &egui::Painter, rect: Rect) {
    let center = Pos2::new(rect.center().x, rect.top());
    let steps = 24;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, alpha(GOLD, 0x12));
    for i in 0..=steps {
        let t = std::f32::consts::PI * i as f32 / steps as f32;
        let p = Pos2::new(center.x + rect.width() * 0.45 * t.cos(), center.y + 260.0 * t.sin());
        mesh.colored_vertex(p, alpha(GOLD, 0));
    }
    for i in 1..=steps as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(mesh);
}
