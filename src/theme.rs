use egui::Color32;

#[allow(dead_code)]
pub struct Theme {
    pub sidebar_bg: Color32,
    pub content_bg: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub card_bg: Color32,
    pub border: Color32,
}

pub const THEME: Theme = Theme {
    sidebar_bg: Color32::from_rgb(22, 22, 42),
    content_bg: Color32::from_rgb(16, 16, 32),
    accent: Color32::from_rgb(108, 92, 231),
    success: Color32::from_rgb(60, 180, 90),
    error: Color32::from_rgb(210, 60, 60),
    warning: Color32::from_rgb(220, 150, 30),
    text_primary: Color32::from_rgb(230, 230, 240),
    text_secondary: Color32::from_rgb(180, 180, 200),
    text_muted: Color32::from_rgb(120, 120, 140),
    card_bg: Color32::from_rgb(26, 26, 46),
    border: Color32::from_rgb(40, 40, 60),
};
