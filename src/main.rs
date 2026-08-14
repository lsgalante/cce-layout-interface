mod scroll_region;
use scroll_region::ScrollRegion;
use wayland_client::QueueHandle;
use cce_ui::cosmic_text::FontSystem;
use serde::{Serialize, Deserialize};
use cce_ui::engine::{Application, EngineState, LogicalPosition, LogicalSize, WindowSettings, LineCap};
use cce_ui::widget::{
    MouseButton, ElementState, MouseScrollDelta, KeyEvent, WidgetHost as UiElement,
    TextBox, Slider, TextLabel, Paginator, Button, Dropdown, Toggle, ColorSelector,
    Label, Spinbox, Key, FontSelector, PageSelector, MenuController
};
use cce_ui::layout::{RenderTarget, Section, UiFrame};

pub struct PageContent {
    pub rects: Vec<([f32; 4], f32, f32, f32, f32)>,
    pub texts: Vec<(String, f32, f32, f32, [f32; 4], Option<String>, Option<[f32; 4]>)>,
}

impl PageContent {
    pub fn new() -> Self {
        Self {
            rects: Vec::new(),
            texts: Vec::new(),
        }
    }
}

impl RenderTarget for PageContent {
    fn rect(&mut self, color: [f32; 4], x: f32, y: f32, w: f32, h: f32) {
        self.rects.push((color, x, y, w, h));
    }

    fn text(&mut self, content: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
        self.texts.push((content.to_string(), size, x, y, color, None, None));
    }

    fn text_with_font(&mut self, content: &str, x: f32, y: f32, size: f32, color: [f32; 4], font: &str) {
        self.texts.push((content.to_string(), size, x, y, color, Some(font.to_string()), None));
    }

    fn text_with_bounds(&mut self, content: &str, x: f32, y: f32, size: f32, color: [f32; 4], bounds: Option<[f32; 4]>) {
        self.texts.push((content.to_string(), size, x, y, color, None, bounds));
    }

    fn text_with_font_and_bounds(&mut self, content: &str, x: f32, y: f32, size: f32, color: [f32; 4], font: &str, bounds: Option<[f32; 4]>) {
        self.texts.push((content.to_string(), size, x, y, color, Some(font.to_string()), bounds));
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TextAlignH {
    Left,
    Center,
    Right,
}

impl Default for TextAlignH {
    fn default() -> Self {
        TextAlignH::Left
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TextAlignV {
    Top,
    Middle,
    Bottom,
}

impl Default for TextAlignV {
    fn default() -> Self {
        TextAlignV::Top
    }
}

fn point_to_line_segment_distance(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.001 {
        let dx_p = px - x1;
        let dy_p = py - y1;
        return (dx_p * dx_p + dy_p * dy_p).sqrt();
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    let dx_p = px - proj_x;
    let dy_p = py - proj_y;
    (dx_p * dx_p + dy_p * dy_p).sqrt()
}

#[derive(Debug, Clone)]
enum AppMessage {
    Exit,
    NewDocument,
    Open,
    Save,
    SaveAs,
    AddText,
    AddRectangle,
    AddBanner,
    AddVector,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum ShapeType {
    Rectangle,
    Banner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Element {
    Text {
        text: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        font_size: f32,
        color: [f32; 4],
        font_family: String,
        #[serde(default, rename = "align")]
        align_h: TextAlignH,
        #[serde(default)]
        align_v: TextAlignV,
        #[serde(default = "default_multiline")]
        multiline: bool,
    },
    Shape {
        shape_type: ShapeType,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
    Vector {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke_width: f32,
        color: [f32; 4],
        #[serde(default = "default_line_cap")]
        line_cap: LineCap,
    },
}

fn default_line_cap() -> LineCap {
    LineCap::Arrow
}

fn default_grid_color() -> [f32; 4] {
    [0.20, 0.35, 0.60, 0.8]
}

fn default_grid_size() -> f32 {
    1.5
}

fn default_multiline() -> bool {
    true
}

fn default_margin_color() -> [f32; 4] {
    [0.20, 0.50, 0.85, 0.35]
}

fn default_margin_thickness() -> f32 {
    1.0
}

fn default_grid_enabled() -> bool {
    true
}

fn default_grid_units() -> usize {
    0
}

fn default_zoom() -> f32 {
    0.33333334
}

fn default_rulers_enabled() -> bool {
    false
}

fn default_ruler_units() -> usize {
    0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LayoutDocument {
    page_w: f32,
    page_h: f32,
    page_color: [f32; 4],
    margin_enabled: bool,
    margin_x: f32,
    margin_y: f32,
    word_processor_enabled: bool,
    wp_text: String,
    elements: Vec<Element>,
    #[serde(default = "default_grid_color")]
    grid_color: [f32; 4],
    #[serde(default = "default_grid_size")]
    grid_size: f32,
    #[serde(default = "default_margin_color")]
    margin_color: [f32; 4],
    #[serde(default = "default_margin_thickness")]
    margin_thickness: f32,
    #[serde(default = "default_grid_enabled")]
    grid_enabled: bool,
    #[serde(default = "default_grid_units")]
    grid_units: usize,
    #[serde(default = "default_zoom")]
    zoom: f32,
    #[serde(default = "default_rulers_enabled")]
    rulers_enabled: bool,
    #[serde(default = "default_ruler_units")]
    ruler_units: usize,
}

struct PagePreset {
    name: &'static str,
    w: f32,
    h: f32,
}

const PRESETS: &[PagePreset] = &[
    PagePreset { name: "Letter (8.5\" x 11\")", w: 510.0, h: 660.0 },
    PagePreset { name: "Legal (8.5\" x 14\")", w: 510.0, h: 840.0 },
    PagePreset { name: "Tabloid (11\" x 17\")", w: 660.0, h: 1020.0 },
    PagePreset { name: "Executive (7.25\" x 10.5\")", w: 435.0, h: 630.0 },
    PagePreset { name: "A3 (297 x 420 mm)", w: 701.0, h: 992.0 },
    PagePreset { name: "A4 (210 x 297 mm)", w: 496.0, h: 701.0 },
    PagePreset { name: "A5 (148 x 210 mm)", w: 350.0, h: 496.0 },
    PagePreset { name: "A6 (105 x 148 mm)", w: 248.0, h: 350.0 },
    PagePreset { name: "B5 (JIS) (182 x 257 mm)", w: 430.0, h: 607.0 },
    PagePreset { name: "Business Card (3.5\" x 2\")", w: 210.0, h: 120.0 },
    PagePreset { name: "Presentation (16:9)", w: 800.0, h: 450.0 },
    PagePreset { name: "Square Poster (12\" x 12\")", w: 720.0, h: 720.0 },
    PagePreset { name: "Custom", w: 510.0, h: 660.0 },
];

/// App shortcuts, resolved once at startup from input.kdl
/// (`cce-layout-interface` domain → `cce-ui` domain).
struct LayoutKeys {
    save_document: String,
    delete_element: String,
}

impl LayoutKeys {
    fn load() -> Self {
        Self {
            save_document: cce_ui::input::app_chord("save_document", "ctrl+s"),
            delete_element: cce_ui::input::app_chord("delete_element", "delete"),
        }
    }
}

struct LayoutApp {
    keys: LayoutKeys,
    btn_new_doc: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    btn_open: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    recent_files: Vec<std::path::PathBuf>,
    recent_files_list: ScrollRegion,
    recent_files_buttons: Vec<cce_ui::widget::Adapted<cce_ui::widget::Button>>,
    btn_exit: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    btn_save: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    btn_save_as: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    paginator: cce_ui::widget::Adapted<Paginator>,
    layer_buttons: Vec<Box<cce_ui::widget::Adapted<cce_ui::widget::Button>>>,
    elements: Vec<Element>,
    selected_idx: Option<usize>,
    dragging: Option<(usize, f32, f32)>, // Index, offset_x, offset_y
    grid_enabled: bool,
    width: u32,
    current_file_path: Option<std::path::PathBuf>,
    last_saved_document: LayoutDocument,
    height: u32,
    scale_factor: f64,
    // (text, size, x, y, color_u8, font, bounds, layout) — the frame's text as prim data.
    text_prims: Vec<(String, f32, f32, f32, [u8; 3], Option<String>, Option<[f32; 4]>, Option<cce_ui::scene::paint::TextLayout>)>,
    font_system: FontSystem,
    needs_rebuild: bool,

    // Page 0: Layout properties controls
    dropdown_presets: cce_ui::widget::Adapted<Dropdown>,
    slider_page_x: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    slider_page_y: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    page_color_selector: cce_ui::widget::Adapted<ColorSelector>,
    page_color: [f32; 4],
    toggle_margin: cce_ui::widget::Adapted<Toggle>,
    dropdown_margin_units: cce_ui::widget::Adapted<Dropdown>,
    slider_margin_x: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    slider_margin_y: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    margin_enabled: bool,
    margin_x: f32,
    margin_y: f32,
    toggle_word_processor: cce_ui::widget::Adapted<Toggle>,
    word_processor_enabled: bool,
    wp_text_box: cce_ui::widget::Adapted<TextBox>,
    sidebar_x: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    sidebar_y: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    sidebar_w: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    sidebar_h: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    sidebar_text: cce_ui::widget::Adapted<TextBox>,
    sidebar_size: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    font_selector: cce_ui::widget::Adapted<FontSelector>,
    dropdown_text_align_h: cce_ui::widget::Adapted<Dropdown>,
    dropdown_text_align_v: cce_ui::widget::Adapted<Dropdown>,
    toggle_multiline: cce_ui::widget::Adapted<Toggle>,
    dropdown_align_h: cce_ui::widget::Adapted<Dropdown>,
    dropdown_align_v: cce_ui::widget::Adapted<Dropdown>,
    dropdown_line_cap: cce_ui::widget::Adapted<Dropdown>,
    wp_base_font_size: f32,

    slider_r: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    slider_g: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    slider_b: cce_ui::widget::Adapted<cce_ui::widget::Slider>,
    slider_zoom: cce_ui::widget::Adapted<cce_ui::widget::Slider>,

    // Page 1: Canvas settings controls
    toggle_grid: cce_ui::widget::Adapted<Toggle>,
    grid_color_selector: cce_ui::widget::Adapted<ColorSelector>,
    grid_color: [f32; 4],
    grid_size: f32,
    spinbox_grid_size: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    dropdown_grid_units: cce_ui::widget::Adapted<Dropdown>,
    margin_color_selector: cce_ui::widget::Adapted<ColorSelector>,
    margin_color: [f32; 4],
    spinbox_margin_thickness: cce_ui::widget::Adapted<cce_ui::widget::Spinbox>,
    margin_thickness: f32,
    btn_add_text: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    btn_add_rect: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    btn_add_banner: cce_ui::widget::Adapted<cce_ui::widget::Button>,
    btn_add_vector: cce_ui::widget::Adapted<cce_ui::widget::Button>,

    toggle_rulers: cce_ui::widget::Adapted<Toggle>,
    dropdown_units: cce_ui::widget::Adapted<Dropdown>,

    label_sel_status: cce_ui::widget::Adapted<cce_ui::widget::Label>,
    label_sel_desc1: cce_ui::widget::Adapted<cce_ui::widget::Label>,
    label_sel_desc2: cce_ui::widget::Adapted<cce_ui::widget::Label>,
    label_grid_snap: cce_ui::widget::Adapted<Toggle>,
    label_total_elements: cce_ui::widget::Adapted<cce_ui::widget::Label>,

    last_selected: Option<usize>,
    page_w: f32,
    page_h: f32,
    pan_x: f32,
    pan_y: f32,
    sidebar_quads: Vec<(f32, f32, f32, f32, [f32; 4])>,
    ui_context: cce_ui::context::UiContext,
}

fn get_monitor_ppi() -> f32 {
    let fallback_ppi = 96.0;
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return fallback_ppi;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let status_path = path.join("status");
            let edid_path = path.join("edid");
            let modes_path = path.join("modes");
            if status_path.exists() && edid_path.exists() && modes_path.exists() {
                if let Ok(status) = std::fs::read_to_string(&status_path) {
                    if status.trim() == "connected" {
                        if let (Ok(edid_bytes), Ok(modes_str)) = (std::fs::read(&edid_path), std::fs::read_to_string(&modes_path)) {
                            if edid_bytes.len() >= 23 {
                                let w_cm = edid_bytes[21] as f32;
                                if w_cm > 0.0 {
                                    // Parse resolution from the first mode line (e.g., "3840x2400")
                                    if let Some(first_mode) = modes_str.lines().next() {
                                        if let Some(w_str) = first_mode.split('x').next() {
                                            if let Ok(w_px) = w_str.parse::<f32>() {
                                                let ppi = (w_px * 2.54) / w_cm;
                                                if ppi > 30.0 && ppi < 600.0 {
                                                    return ppi;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fallback_ppi
}

#[allow(dead_code)]
fn get_system_fonts() -> Vec<String> {
    let mut fonts = Vec::new();
    if let Ok(output) = std::process::Command::new("fc-list")
        .arg(":")
        .arg("family")
        .output()
    {
        if let Ok(stdout) = String::from_utf8(output.stdout) {
            for line in stdout.lines() {
                for part in line.split(',') {
                    let cleaned = part.replace('\\', "").trim().to_string();
                    if !cleaned.is_empty() {
                        fonts.push(cleaned);
                    }
                }
            }
        }
    }
    fonts.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    fonts.dedup();
    if fonts.is_empty() {
        fonts = vec![
            "Monospace".to_string(),
            "Sans-Serif".to_string(),
            "Serif".to_string(),
        ];
    }
    fonts
}

impl LayoutApp {
    fn load_recent_files() -> Vec<std::path::PathBuf> {
        cce_ui::config::load_recent_files()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect()
    }

    fn save_recent_files(files: &[std::path::PathBuf]) {
        let string_files: Vec<String> = files.iter().map(|p| p.to_string_lossy().to_string()).collect();
        cce_ui::config::save_recent_files(&string_files);
    }

    fn add_recent_file(&mut self, path: std::path::PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
        Self::save_recent_files(&self.recent_files);
        self.rebuild_recent_buttons();
    }

    fn rebuild_recent_buttons(&mut self) {
        // Widget ids are globally monotonic and never reused, so the buttons pushed below
        // register under NEW ids; the dropped ones would stay in the registry pointing at
        // freed memory, and the engine derefs the whole registry on every left press
        // (`close_popovers_missed_by_press`). Drop their registrations first.
        let stale: Vec<_> = self.recent_files_buttons.iter().map(|b| b.id()).collect();
        for id in stale {
            self.ui_context.unregister_widget(id);
        }
        self.recent_files_buttons.clear();
        for file in &self.recent_files {
            let label = file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.to_string_lossy().to_string());
            let btn = Button::new_list_row(0.0, 0.0, 0.0, 0.0).with_label(&label);
            self.recent_files_buttons.push(btn);
        }
    }

    fn save_document(&self, path: &std::path::Path) -> std::io::Result<()> {
        let doc = self.get_current_document();
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &doc)?;
        Ok(())
    }

    fn perform_save_as(&mut self) {
        let path_opt = std::process::Command::new("/home/lsgalante/.local/bin/cce-files")
            .arg("--save")
            .output()
            .or_else(|_| {
                std::process::Command::new("cce-files")
                    .arg("--save")
                    .output()
            })
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let trimmed = stdout.trim();
                    if !trimmed.is_empty() {
                        Some(std::path::PathBuf::from(trimmed))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

        if let Some(path) = path_opt {
            if let Err(e) = self.save_document(&path) {
                log::error!("Failed to save document: {}", e);
            } else {
                log::info!("Successfully saved layout to {:?}", path);
                self.current_file_path = Some(path.clone());
                self.last_saved_document = self.get_current_document();
                self.add_recent_file(path);
            }
        }
    }

    fn load_document(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        let file = std::fs::File::open(path)?;
        let doc: LayoutDocument = serde_json::from_reader(file)?;

        self.page_w = doc.page_w;
        self.page_h = doc.page_h;
        self.page_color = doc.page_color;
        self.margin_enabled = doc.margin_enabled;
        self.margin_x = doc.margin_x;
        self.margin_y = doc.margin_y;
        self.word_processor_enabled = doc.word_processor_enabled;
        self.wp_text_box.text = doc.wp_text.clone();
        self.wp_text_box.edit_buffer = doc.wp_text;
        self.elements = doc.elements;

        self.toggle_margin.set_toggled(self.margin_enabled);
        self.toggle_word_processor.set_toggled(self.word_processor_enabled);

        let r = (self.page_color[0] * 255.0).clamp(0.0, 255.0) as u8;
        let g = (self.page_color[1] * 255.0).clamp(0.0, 255.0) as u8;
        let b = (self.page_color[2] * 255.0).clamp(0.0, 255.0) as u8;
        self.page_color_selector.color = [r, g, b];

        self.grid_color = doc.grid_color;
        self.grid_size = doc.grid_size;
        self.margin_color = doc.margin_color;

        let gr = (self.grid_color[0] * 255.0).clamp(0.0, 255.0) as u8;
        let gg = (self.grid_color[1] * 255.0).clamp(0.0, 255.0) as u8;
        let gb = (self.grid_color[2] * 255.0).clamp(0.0, 255.0) as u8;
        self.grid_color_selector.color = [gr, gg, gb];
        self.spinbox_grid_size.value = (self.grid_size * 10.0).round() as i32;

        let mr = (self.margin_color[0] * 255.0).clamp(0.0, 255.0) as u8;
        let mg = (self.margin_color[1] * 255.0).clamp(0.0, 255.0) as u8;
        let mb = (self.margin_color[2] * 255.0).clamp(0.0, 255.0) as u8;
        self.margin_color_selector.color = [mr, mg, mb];

        self.margin_thickness = doc.margin_thickness;
        self.spinbox_margin_thickness.value = self.margin_thickness.round() as i32;

        self.grid_enabled = doc.grid_enabled;
        self.toggle_grid.set_toggled(doc.grid_enabled);
        self.label_grid_snap.set_toggled(doc.grid_enabled);
        self.dropdown_grid_units.selected = doc.grid_units;
        self.slider_zoom.set_value(doc.zoom);
        self.toggle_rulers.set_toggled(doc.rulers_enabled);
        self.dropdown_units.selected = doc.ruler_units;

        let mut preset_idx = PRESETS.len() - 1; // Custom
        for (i, preset) in PRESETS.iter().enumerate() {
            if (preset.w - self.page_w).abs() < 1.0 && (preset.h - self.page_h).abs() < 1.0 {
                preset_idx = i;
                break;
            }
        }
        self.dropdown_presets.selected = preset_idx;

        self.sync_page_units();
        self.rebuild_layers_tab_widgets();
        self.sync_sidebar_fields();
        self.needs_rebuild = true;

        Ok(())
    }

    fn perform_open(&mut self) {
        let path_opt = std::process::Command::new("/home/lsgalante/.local/bin/cce-files")
            .arg("--select")
            .output()
            .or_else(|_| {
                std::process::Command::new("cce-files")
                    .arg("--select")
                    .output()
            })
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let trimmed = stdout.trim();
                    if !trimmed.is_empty() {
                        Some(std::path::PathBuf::from(trimmed))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

        if let Some(path) = path_opt {
            self.selected_idx = None;
            self.dragging = None;
            self.pan_x = 0.0;
            self.pan_y = 0.0;
            if let Err(e) = self.load_document(&path) {
                log::error!("Failed to load document: {}", e);
            } else {
                log::info!("Successfully loaded layout from {:?}", path);
                self.current_file_path = Some(path.clone());
                self.last_saved_document = self.get_current_document();
                self.add_recent_file(path);
            }
        }
    }

    fn sync_page_units(&mut self) {
        let unit_idx = self.dropdown_margin_units.selected;
        let px_per_unit = match unit_idx {
            0 => 1.0,           // Pixels
            1 => 60.0,          // Inches
            2 => 23.622047,     // Centimeters
            3 => 2.3622047,     // Millimeters
            _ => 1.0,
        };
        let max_val = 100.0 / px_per_unit;
        self.slider_margin_x.set_range(0.0, max_val);
        self.slider_margin_x.set_value(self.margin_x / 100.0);

        self.slider_margin_y.set_range(0.0, max_val);
        self.slider_margin_y.set_value(self.margin_y / 100.0);

        self.slider_page_x.set_range(100.0 / px_per_unit, 1500.0 / px_per_unit);
        self.slider_page_x.set_value((self.page_w - 100.0) / (1500.0 - 100.0));

        self.slider_page_y.set_range(100.0 / px_per_unit, 1500.0 / px_per_unit);
        self.slider_page_y.set_value((self.page_h - 100.0) / (1500.0 - 100.0));

        let unit_suffix = match unit_idx {
            0 => "px",
            1 => "in",
            2 => "cm",
            3 => "mm",
            _ => "px",
        };
        self.slider_margin_x.set_label(&format!("Margin Width (X) ({})", unit_suffix));
        self.slider_margin_y.set_label(&format!("Margin Height (Y) ({})", unit_suffix));
        self.slider_page_x.set_label(&format!("Page Width (X) ({})", unit_suffix));
        self.slider_page_y.set_label(&format!("Page Height (Y) ({})", unit_suffix));
    }

    fn render_zoom(&self) -> f32 {
        let ppi = get_monitor_ppi();
        let scale_factor = self.scale_factor as f32;
        let zoom_multiplier = ppi / (60.0 * scale_factor);
        self.slider_zoom.get_scaled_value() * zoom_multiplier
    }

    fn rebuild_text_items(&mut self) {
        let font_system = &mut self.font_system;
        let selected_page = self.paginator.selected_page();
        let word_processor_enabled = self.word_processor_enabled;
        let _ = cce_ui::scale::scale_factor();

        match selected_page {
            0 => {
                self.btn_new_doc.prepare_text(font_system);
                self.btn_open.prepare_text(font_system);
                for btn in &mut self.recent_files_buttons {
                    btn.prepare_text(font_system);
                }
                self.btn_save.prepare_text(font_system);
                self.btn_save_as.prepare_text(font_system);
                self.btn_exit.prepare_text(font_system);
            }
            1 => {
                self.dropdown_margin_units.prepare_text(font_system);
                self.dropdown_presets.prepare_text(font_system);
                self.slider_page_x.prepare_text(font_system);
                self.slider_page_y.prepare_text(font_system);
                self.page_color_selector.prepare_text(font_system);
                self.toggle_margin.prepare_text(font_system);
                self.toggle_word_processor.prepare_text(font_system);
                self.slider_margin_x.prepare_text(font_system);
                self.slider_margin_y.prepare_text(font_system);
            }
            2 => {
                if word_processor_enabled {
                    self.font_selector.prepare_text(font_system);
                    self.sidebar_size.prepare_text(font_system);
                    self.slider_r.prepare_text(font_system);
                    self.slider_g.prepare_text(font_system);
                    self.slider_b.prepare_text(font_system);
                } else if self.selected_idx.is_some() {
                    self.dropdown_align_h.prepare_text(font_system);
                    self.dropdown_align_v.prepare_text(font_system);

                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        self.sidebar_x.prepare_text(font_system);
                        self.sidebar_y.prepare_text(font_system);
                        self.sidebar_w.prepare_text(font_system);
                        self.sidebar_h.prepare_text(font_system);
                        self.sidebar_text.prepare_text(font_system);
                        self.font_selector.prepare_text(font_system);
                        self.toggle_multiline.prepare_text(font_system);
                        self.dropdown_text_align_h.prepare_text(font_system);
                        self.dropdown_text_align_v.prepare_text(font_system);
                        self.sidebar_size.prepare_text(font_system);
                        self.slider_r.prepare_text(font_system);
                        self.slider_g.prepare_text(font_system);
                        self.slider_b.prepare_text(font_system);
                    } else if is_vector {
                        self.sidebar_x.prepare_text(font_system);
                        self.sidebar_y.prepare_text(font_system);
                        self.sidebar_w.prepare_text(font_system);
                        self.sidebar_h.prepare_text(font_system);
                        self.sidebar_size.prepare_text(font_system);
                        self.dropdown_line_cap.prepare_text(font_system);
                        self.slider_r.prepare_text(font_system);
                        self.slider_g.prepare_text(font_system);
                        self.slider_b.prepare_text(font_system);
                    } else {
                        self.sidebar_x.prepare_text(font_system);
                        self.sidebar_y.prepare_text(font_system);
                        self.sidebar_w.prepare_text(font_system);
                        self.sidebar_h.prepare_text(font_system);
                        self.slider_r.prepare_text(font_system);
                        self.slider_g.prepare_text(font_system);
                        self.slider_b.prepare_text(font_system);
                    }
                }
            }
            3 => {
                self.btn_add_text.prepare_text(font_system);
                self.btn_add_rect.prepare_text(font_system);
                self.btn_add_banner.prepare_text(font_system);
                self.btn_add_vector.prepare_text(font_system);
            }
            4 => {
                self.toggle_grid.prepare_text(font_system);
                self.grid_color_selector.prepare_text(font_system);
                self.spinbox_grid_size.prepare_text(font_system);
                self.label_grid_snap.prepare_text(font_system);
                self.slider_zoom.prepare_text(font_system);
                self.toggle_rulers.prepare_text(font_system);
                self.dropdown_units.prepare_text(font_system);
                self.margin_color_selector.prepare_text(font_system);
                self.spinbox_margin_thickness.prepare_text(font_system);
                self.dropdown_grid_units.prepare_text(font_system);
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    btn.prepare_text(font_system);
                }
            }
            _ => {}
        }

        self.wp_text_box.prepare_text(font_system);

        self.sidebar_quads.clear();
        self.text_prims.clear();

        // 1. Populate sidebar page views into PageContent
        let mut pc = PageContent::new();
        let ui_frame = UiFrame::start(0.0);

        let sidebar_w = self.paginator.sidebar_w();
        let cx = sidebar_w;
        let cy = 16.0;
        let cw = 280.0 - sidebar_w;

        match self.paginator.selected_page() {
            0 => {
                let mut sec = Section::new(&mut pc, cx, cy, cw, "File Operations");
                let col_w = cw - 40.0;
                sec.widget(&mut pc, &mut self.btn_new_doc, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_open, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);

                sec.text(&mut pc, "Recent Files", 12.0, 0.0, 11.0, [0.83, 0.83, 0.83, 1.0]);
                sec.spacing(16.0);

                let list_h = 100.0;
                let list_x = sec.ax(12.0);
                let list_y = sec.ay();
                self.recent_files_list.set_rect(list_x, list_y, col_w, list_h);
                self.recent_files_list.push_prims(&mut pc);
                sec.spacing(list_h);

                self.recent_files_list.update_bounds(self.recent_files.len(), list_y, list_h);

                let btn_h = 22.0;
                let inner_x = list_x + 4.0;
                let inner_w = col_w - 16.0;

                for (idx, btn) in self.recent_files_buttons.iter_mut().enumerate() {
                    if let Some(draw_y) = self.recent_files_list.get_item_draw_y(idx, 0.0) {
                        cce_ui::layout::render_widget(&mut pc, btn, inner_x, draw_y, inner_w, btn_h, &mut self.ui_context);
                    } else {
                        btn.set_rect(-9999.0, -9999.0, 0.0, 0.0);
                    }
                }

                if self.recent_files.is_empty() {
                    pc.text("No recent files", list_x + 12.0, list_y + 16.0, 11.0, [0.55, 0.55, 0.60, 1.0]);
                }

                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_save, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_save_as, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_exit, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.finish(&mut pc);
            }
            1 => {
                let mut sec = Section::new(&mut pc, cx, cy, cw, "Document Size");
                let col_w = cw - 40.0;
                sec.widget(&mut pc, &mut self.dropdown_presets, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.slider_page_x, 12.0, col_w, 18.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.slider_page_y, 12.0, col_w, 18.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.page_color_selector, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                let next_y = sec.finish(&mut pc);

                let mut sec2 = Section::new(&mut pc, cx, next_y, cw, "Margins & Mode");
                sec2.widget(&mut pc, &mut self.toggle_margin, 12.0, col_w, 26.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.widget(&mut pc, &mut self.dropdown_margin_units, 12.0, col_w, 26.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.widget(&mut pc, &mut self.slider_margin_x, 12.0, col_w, 18.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.widget(&mut pc, &mut self.slider_margin_y, 12.0, col_w, 18.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.widget(&mut pc, &mut self.toggle_word_processor, 12.0, col_w, 26.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.finish(&mut pc);
            }
            2 => {
                let col_w = cw - 40.0;
                if self.word_processor_enabled {
                    let mut sec = Section::new(&mut pc, cx, cy, cw, "Word Processor");
                    sec.widget(&mut pc, &mut self.font_selector, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    sec.widget(&mut pc, &mut self.sidebar_size, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    sec.widget(&mut pc, &mut self.slider_r, 12.0, col_w, 18.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    sec.widget(&mut pc, &mut self.slider_g, 12.0, col_w, 18.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    sec.widget(&mut pc, &mut self.slider_b, 12.0, col_w, 18.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    sec.finish(&mut pc);
                } else if let Some(idx) = self.selected_idx {
                    let (is_text, is_vector) = match &self.elements[idx] {
                        Element::Text { .. } => (true, false),
                        Element::Vector { .. } => (false, true),
                        _ => (false, false),
                    };

                    let mut sec = Section::new(&mut pc, cx, cy, cw, "Geometry");
                    sec.widget(&mut pc, &mut self.sidebar_x, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec.spacing(16.0);
                    sec.widget(&mut pc, &mut self.sidebar_y, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec.spacing(16.0);
                    sec.widget(&mut pc, &mut self.sidebar_w, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec.spacing(16.0);
                    sec.widget(&mut pc, &mut self.sidebar_h, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    let next_y = sec.finish(&mut pc);

                    let mut sec_align = Section::new(&mut pc, cx, next_y, cw, "Alignment");
                    sec_align.widget(&mut pc, &mut self.dropdown_align_h, 12.0, col_w, 26.0, &mut self.ui_context);
                    sec_align.spacing(12.0);
                    sec_align.widget(&mut pc, &mut self.dropdown_align_v, 12.0, col_w, 26.0, &mut self.ui_context);
                    let next_y = sec_align.finish(&mut pc);

                    if is_text {
                        let mut sec2 = Section::new(&mut pc, cx, next_y, cw, "Text Properties");
                        sec2.widget(&mut pc, &mut self.sidebar_text, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.toggle_multiline, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.font_selector, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.sidebar_size, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_r, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_g, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_b, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        let next_y_align = sec2.finish(&mut pc);

                        let mut sec_text_align = Section::new(&mut pc, cx, next_y_align, cw, "Text Alignment");
                        sec_text_align.widget(&mut pc, &mut self.dropdown_text_align_h, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec_text_align.spacing(12.0);
                        sec_text_align.widget(&mut pc, &mut self.dropdown_text_align_v, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec_text_align.finish(&mut pc);
                    } else if is_vector {
                        let mut sec2 = Section::new(&mut pc, cx, next_y, cw, "Line Properties");
                        sec2.widget(&mut pc, &mut self.sidebar_size, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.dropdown_line_cap, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_r, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_g, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_b, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.finish(&mut pc);
                    } else {
                        let mut sec2 = Section::new(&mut pc, cx, next_y, cw, "Fill Color");
                        sec2.widget(&mut pc, &mut self.slider_r, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_g, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.widget(&mut pc, &mut self.slider_b, 12.0, col_w, 18.0, &mut self.ui_context);
                        sec2.spacing(12.0);
                        sec2.finish(&mut pc);
                    }
                } else {
                    let mut sec = Section::new(&mut pc, cx, cy, cw, "Properties");
                    sec.widget(&mut pc, &mut self.label_sel_status, 12.0, col_w, 18.0, &mut self.ui_context);
                    sec.spacing(8.0);
                    sec.widget(&mut pc, &mut self.label_sel_desc1, 12.0, col_w, 14.0, &mut self.ui_context);
                    sec.spacing(6.0);
                    sec.widget(&mut pc, &mut self.label_sel_desc2, 12.0, col_w, 14.0, &mut self.ui_context);
                    sec.spacing(12.0);
                    sec.finish(&mut pc);
                }
            }
            3 => {
                let mut sec = Section::new(&mut pc, cx, cy, cw, "Add Elements");
                let col_w = cw - 40.0;
                sec.widget(&mut pc, &mut self.btn_add_text, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_add_rect, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_add_banner, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.btn_add_vector, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.label_total_elements, 12.0, col_w, 18.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.finish(&mut pc);
            }
            4 => {
                let mut sec = Section::new(&mut pc, cx, cy, cw, "Grid");
                let col_w = cw - 40.0;
                sec.widget(&mut pc, &mut self.toggle_grid, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.grid_color_selector, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.spinbox_grid_size, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.label_grid_snap, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                sec.widget(&mut pc, &mut self.dropdown_grid_units, 12.0, col_w, 26.0, &mut self.ui_context);
                sec.spacing(12.0);
                let next_y = sec.finish(&mut pc);

                let mut sec_zoom = Section::new(&mut pc, cx, next_y, cw, "Zoom");
                sec_zoom.widget(&mut pc, &mut self.slider_zoom, 12.0, col_w, 18.0, &mut self.ui_context);
                sec_zoom.spacing(12.0);
                let next_y = sec_zoom.finish(&mut pc);

                let mut sec_margins = Section::new(&mut pc, cx, next_y, cw, "Margins");
                sec_margins.widget(&mut pc, &mut self.margin_color_selector, 12.0, col_w, 26.0, &mut self.ui_context);
                sec_margins.spacing(12.0);
                sec_margins.widget(&mut pc, &mut self.spinbox_margin_thickness, 12.0, col_w, 26.0, &mut self.ui_context);
                sec_margins.spacing(12.0);
                let next_y = sec_margins.finish(&mut pc);

                let mut sec2 = Section::new(&mut pc, cx, next_y, cw, "Rulers");
                sec2.widget(&mut pc, &mut self.toggle_rulers, 12.0, col_w, 26.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.widget(&mut pc, &mut self.dropdown_units, 12.0, col_w, 26.0, &mut self.ui_context);
                sec2.spacing(12.0);
                sec2.finish(&mut pc);
            }
            5 => {
                let mut sec = Section::new(&mut pc, cx, cy, cw, "Layers List");
                let col_w = cw - 40.0;
                if self.layer_buttons.is_empty() {
                    let mut label_no_layers = Label::new("No elements found").with_color([0x83, 0x83, 0x8a]);
                    sec.widget(&mut pc, &mut label_no_layers, 12.0, col_w, 18.0, &mut self.ui_context);
                    sec.spacing(12.0);
                } else {
                    for btn in &mut self.layer_buttons {
                        sec.widget(&mut pc, &mut **btn, 12.0, col_w, 26.0, &mut self.ui_context);
                        sec.spacing(8.0);
                    }
                }
                sec.finish(&mut pc);
            }
            _ => {}
        }

        ui_frame.finish(&mut pc);

        // Store the sidebar quads
        for (c, x, y, w, h) in pc.rects {
            self.sidebar_quads.push((x, y, w, h, c));
        }

        // Sidebar widget text as prims (shaped by the engine at render).
        for (text, size, x, y, color, font, bounds) in pc.texts {
            let c = [
                (color[0] * 255.0).clamp(0.0, 255.0) as u8,
                (color[1] * 255.0).clamp(0.0, 255.0) as u8,
                (color[2] * 255.0).clamp(0.0, 255.0) as u8,
            ];
            self.text_prims.push((text, size, x, y, c, font, bounds, None));
        }

        if let Some(idx) = self.selected_idx {
            self.label_sel_status.set_text(&format!("Selected Element #{}", idx + 1));
            self.label_sel_status.set_color([0x3b, 0x82, 0xf6]);
            self.label_sel_desc1.set_text("");
            self.label_sel_desc2.set_text("");
        } else {
            self.label_sel_status.set_text("No Selection");
            self.label_sel_status.set_color([0x83, 0x83, 0x8a]);
            self.label_sel_desc1.set_text("Click canvas elements");
            self.label_sel_desc2.set_text("to edit properties.");
        }

        let snap_suffix = match self.dropdown_grid_units.selected {
            0 => "20px",
            1 => "0.25in",
            2 => "0.5cm",
            3 => "5mm",
            _ => "20px",
        };
        self.label_grid_snap.set_label(&format!(
            "Grid Snapping: {}",
            if self.grid_enabled {
                format!("ON ({})", snap_suffix)
            } else {
                "OFF".to_string()
            }
        ));
        self.label_total_elements.set_text(&format!("Total Elements: {}", self.elements.len()));

        let mut labels = Vec::new();

        let canvas_w = self.width as f32 - 280.0;
        let canvas_h = self.height as f32;
        let zoom = self.render_zoom();
        let page_w = self.page_w * zoom;
        let page_h = self.page_h * zoom;
        let page_x = 280.0 + (canvas_w - page_w) / 2.0 + self.pan_x;
        let page_y = (canvas_h - page_h) / 2.0 + self.pan_y;

        // 2. Paginator sidebar tabs: emitted in display_list via the paint walk.

        if self.toggle_rulers.toggled() {
            let unit_idx = self.dropdown_units.selected;
            let (px_per_unit, tick_step, label_step, precision) = match unit_idx {
                0 => (1.0, 50.0, 100.0, 0),       // Pixels
                1 => (60.0, 0.25, 1.0, 0),        // Inches
                2 => (23.622047, 0.5, 1.0, 0),    // Centimeters
                3 => (2.3622047, 5.0, 10.0, 0),   // Millimeters
                _ => (1.0, 50.0, 100.0, 0),
            };

            // X Ruler Labels
            let max_unit_w = self.page_w / px_per_unit;
            let mut val = 0.0;
            while val <= max_unit_w + 0.001 {
                let is_major = (val / label_step).round() * label_step;
                if (val - is_major).abs() < 0.001 {
                    labels.push(TextLabel {
                        text: format!("{:.precision$}", val, precision = precision),
                        x: page_x + val * px_per_unit * zoom + 2.0,
                        y: page_y - 16.0,
                        font_size: 8.0,
                        color: [0xaa, 0xaa, 0xbb],
                    });
                }
                val += tick_step;
            }

            // Y Ruler Labels
            let max_unit_h = self.page_h / px_per_unit;
            let mut val = 0.0;
            while val <= max_unit_h + 0.001 {
                let is_major = (val / label_step).round() * label_step;
                if (val - is_major).abs() < 0.001 {
                    labels.push(TextLabel {
                        text: format!("{:.precision$}", val, precision = precision),
                        x: page_x - 18.0,
                        y: page_y + val * px_per_unit * zoom + 2.0,
                        font_size: 8.0,
                        color: [0xaa, 0xaa, 0xbb],
                    });
                }
                val += tick_step;
            }
        }



        // 5. Canvas Element Labels (drawn relative to the paper sheet)
        if self.word_processor_enabled {
            // Word-processor text box: emitted in display_list via the paint walk.
        } else {
            for (idx, element) in self.elements.iter().enumerate() {
                match element {
                    Element::Text { text, x, y, w, h, font_size, color, font_family, align_h, align_v, multiline } => {
                        let label_font_size = *font_size * zoom;
                        let label_color = [
                            (color[0] * 255.0).clamp(0.0, 255.0) as u8,
                            (color[1] * 255.0).clamp(0.0, 255.0) as u8,
                            (color[2] * 255.0).clamp(0.0, 255.0) as u8,
                        ];

                        // Boxed canvas text: the engine shapes it uncached with wrap +
                        // alignment (Phase 6aj / boxed-text prims) and applies the vertical
                        // offset from the shaped height.
                        let text_w = (*w * zoom - 16.0 * zoom).max(0.0);
                        let text_h = (*h * zoom - 12.0 * zoom).max(0.0);
                        let wrap = if *multiline { Some(text_w) } else { None };
                        let l_align_h = match align_h {
                            TextAlignH::Left => cce_ui::scene::paint::AlignH::Left,
                            TextAlignH::Center => cce_ui::scene::paint::AlignH::Center,
                            TextAlignH::Right => cce_ui::scene::paint::AlignH::Right,
                        };
                        let l_align_v = match align_v {
                            TextAlignV::Top => cce_ui::scene::paint::AlignV::Top,
                            TextAlignV::Middle => cce_ui::scene::paint::AlignV::Middle,
                            TextAlignV::Bottom => cce_ui::scene::paint::AlignV::Bottom,
                        };
                        let tl = cce_ui::scene::paint::TextLayout {
                            wrap_width: wrap,
                            box_height: text_h,
                            align_h: l_align_h,
                            align_v: l_align_v,
                        };
                        let label_x = page_x + *x * zoom + 8.0 * zoom;
                        let label_y = page_y + *y * zoom + 6.0 * zoom;
                        self.text_prims.push((text.clone(), label_font_size, label_x, label_y, label_color, Some(font_family.clone()), None, Some(tl)));
                    }
                    Element::Shape { shape_type, x, y, w: _, h: _, color: _ } => {
                        let type_str = match shape_type {
                            ShapeType::Rectangle => "Rectangle",
                            ShapeType::Banner => "Banner",
                        };
                        labels.push(TextLabel {
                            text: format!("{} #{}", type_str, idx + 1),
                            x: page_x + *x * zoom + 8.0 * zoom,
                            y: page_y + *y * zoom + 6.0 * zoom,
                            font_size: 11.0 * zoom,
                            color: [0xee, 0xee, 0xf5],
                        });
                    }
                    Element::Vector { x1, y1, .. } => {
                        labels.push(TextLabel {
                            text: format!("Line #{}", idx + 1),
                            x: page_x + *x1 * zoom + 8.0 * zoom,
                            y: page_y + *y1 * zoom + 6.0 * zoom,
                            font_size: 11.0 * zoom,
                            color: [0xee, 0xee, 0xf5],
                        });
                    }
                }
            }
        }

        for label in labels {
            self.text_prims.push((label.text, label.font_size, label.x, label.y, label.color, None, None, None));
        }
    }

    fn rebuild_layers_tab_widgets(&mut self) {
        // See the note in rebuild_recent_buttons(): monotonic ids mean the outgoing buttons
        // must be unregistered or the registry keeps dangling pointers into freed boxes.
        let stale: Vec<_> = self.layer_buttons.iter().map(|b| b.id()).collect();
        for id in stale {
            self.ui_context.unregister_widget(id);
        }
        self.layer_buttons.clear();
        for (idx, element) in self.elements.iter().enumerate() {
            let label = match element {
                Element::Text { text, .. } => {
                    let truncated: String = text.chars().take(20).collect();
                    format!("{}. Text: \"{}\"", idx + 1, truncated)
                }
                Element::Shape { shape_type, .. } => {
                    let type_str = match shape_type {
                        ShapeType::Rectangle => "Rectangle",
                        ShapeType::Banner => "Banner",
                    };
                    format!("{}. {}", idx + 1, type_str)
                }
                Element::Vector { x1, y1, x2, y2, .. } => {
                    format!("{}. Line: ({:.0},{:.0})->({:.0},{:.0})", idx + 1, x1, y1, x2, y2)
                }
            };

            let mut btn = Box::new(Button::new_list_row(0.0, 0.0, 224.0, 26.0).with_label(&label));
            btn.selected = Some(idx) == self.selected_idx;
            self.layer_buttons.push(btn);
        }
    }


    fn sync_sidebar_fields(&mut self) {
        if self.word_processor_enabled {
            if !self.sidebar_size.editing {
                self.sidebar_size.value = self.wp_base_font_size.round() as i32;
            }
            self.font_selector.font_family = self.wp_text_box.font_family.clone();

            if let Some(rgb) = self.wp_text_box.text_color {
                self.slider_r.set_value(rgb[0] as f32 / 255.0);
                self.slider_g.set_value(rgb[1] as f32 / 255.0);
                self.slider_b.set_value(rgb[2] as f32 / 255.0);
            } else {
                self.slider_r.set_value(0.1);
                self.slider_g.set_value(0.1);
                self.slider_b.set_value(0.14);
            }
        } else if let Some(idx) = self.selected_idx {
            if self.last_selected != Some(idx) {
                self.sidebar_text.unfocus();
                self.sidebar_x.unfocus();
                self.sidebar_y.unfocus();
                self.sidebar_w.unfocus();
                self.sidebar_h.unfocus();
                self.sidebar_size.unfocus();
                self.last_selected = Some(idx);
            }

            let element = &self.elements[idx];
            match element {
                Element::Text { text, x, y, w, h, font_size, color, font_family, align_h, align_v, multiline } => {
                    self.sidebar_x.base_mut().label = Some("X Position".to_string());
                    self.sidebar_y.base_mut().label = Some("Y Position".to_string());
                    self.sidebar_w.base_mut().label = Some("Width".to_string());
                    self.sidebar_h.base_mut().label = Some("Height".to_string());
                    self.sidebar_size.base_mut().label = Some("Text Size".to_string());

                    if !self.sidebar_text.editing { self.sidebar_text.text = text.clone(); }
                    if !self.sidebar_x.editing { self.sidebar_x.value = x.round() as i32; }
                    if !self.sidebar_y.editing { self.sidebar_y.value = y.round() as i32; }
                    if !self.sidebar_w.editing { self.sidebar_w.value = w.round() as i32; }
                    if !self.sidebar_h.editing { self.sidebar_h.value = h.round() as i32; }
                    if !self.sidebar_size.editing { self.sidebar_size.value = font_size.round() as i32; }

                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);

                    self.font_selector.font_family = font_family.clone();
                    self.toggle_multiline.set_toggled(*multiline);

                    self.dropdown_text_align_h.selected = match align_h {
                        TextAlignH::Left => 0,
                        TextAlignH::Center => 1,
                        TextAlignH::Right => 2,
                    };
                    self.dropdown_text_align_v.selected = match align_v {
                        TextAlignV::Top => 0,
                        TextAlignV::Middle => 1,
                        TextAlignV::Bottom => 2,
                    };
                }
                Element::Shape { shape_type: _, x, y, w, h, color } => {
                    self.sidebar_x.base_mut().label = Some("X Position".to_string());
                    self.sidebar_y.base_mut().label = Some("Y Position".to_string());
                    self.sidebar_w.base_mut().label = Some("Width".to_string());
                    self.sidebar_h.base_mut().label = Some("Height".to_string());

                    if !self.sidebar_text.editing { self.sidebar_text.text = "Shape Mode".to_string(); }
                    if !self.sidebar_x.editing { self.sidebar_x.value = x.round() as i32; }
                    if !self.sidebar_y.editing { self.sidebar_y.value = y.round() as i32; }
                    if !self.sidebar_w.editing { self.sidebar_w.value = w.round() as i32; }
                    if !self.sidebar_h.editing { self.sidebar_h.value = h.round() as i32; }

                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);
                }
                Element::Vector { x1, y1, x2, y2, stroke_width, color, line_cap } => {
                    self.sidebar_x.base_mut().label = Some("Start X".to_string());
                    self.sidebar_y.base_mut().label = Some("Start Y".to_string());
                    self.sidebar_w.base_mut().label = Some("End X".to_string());
                    self.sidebar_h.base_mut().label = Some("End Y".to_string());
                    self.sidebar_size.base_mut().label = Some("Thickness".to_string());

                    if !self.sidebar_text.editing { self.sidebar_text.text = "Line Mode".to_string(); }
                    if !self.sidebar_x.editing { self.sidebar_x.value = x1.round() as i32; }
                    if !self.sidebar_y.editing { self.sidebar_y.value = y1.round() as i32; }
                    if !self.sidebar_w.editing { self.sidebar_w.value = x2.round() as i32; }
                    if !self.sidebar_h.editing { self.sidebar_h.value = y2.round() as i32; }
                    if !self.sidebar_size.editing { self.sidebar_size.value = stroke_width.round() as i32; }
                    self.dropdown_line_cap.selected = match line_cap {
                        LineCap::Arrow => 0,
                        LineCap::Round => 1,
                        LineCap::Flat => 2,
                    };

                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);
                }
            }

            // Sync page alignment dropdowns based on current positions
            let (ex, ey, ew, eh) = match element {
                Element::Text { x, y, w, h, .. } => (*x, *y, *w, *h),
                Element::Shape { x, y, w, h, .. } => (*x, *y, *w, *h),
                Element::Vector { x1, y1, x2, y2, .. } => {
                    let x = x1.min(*x2);
                    let y = y1.min(*y2);
                    let w = (x1 - x2).abs();
                    let h = (y1 - y2).abs();
                    (x, y, w, h)
                }
            };

            let center_x = ((self.page_w - ew) / 2.0).round();
            let right_x = (self.page_w - ew).round();
            self.dropdown_align_h.selected = if (ex - 0.0).abs() < 0.1 {
                1 // Left
            } else if (ex - center_x).abs() < 0.1 {
                2 // Center
            } else if (ex - right_x).abs() < 0.1 {
                3 // Right
            } else {
                0 // "--" (No alignment)
            };

            let middle_y = ((self.page_h - eh) / 2.0).round();
            let bottom_y = (self.page_h - eh).round();
            self.dropdown_align_v.selected = if (ey - 0.0).abs() < 0.1 {
                1 // Top
            } else if (ey - middle_y).abs() < 0.1 {
                2 // Middle
            } else if (ey - bottom_y).abs() < 0.1 {
                3 // Bottom
            } else {
                0 // "--" (No alignment)
            };
        }
    }

    fn apply_sidebar_changes(&mut self) {
        if self.word_processor_enabled {
            let mut family_str = self.wp_text_box.font_family.clone();
            if self.font_selector.take_change() {
                family_str = self.font_selector.font_family.clone();
            }
            self.wp_text_box.font_family = family_str;

            let size_val = if self.sidebar_size.editing {
                self.sidebar_size.edit_buffer.parse::<f32>().unwrap_or(14.0)
            } else {
                self.sidebar_size.value as f32
            };
            self.wp_base_font_size = size_val;

            let r = (self.slider_r.inner().value() * 255.0).clamp(0.0, 255.0) as u8;
            let g = (self.slider_g.inner().value() * 255.0).clamp(0.0, 255.0) as u8;
            let b = (self.slider_b.inner().value() * 255.0).clamp(0.0, 255.0) as u8;
            self.wp_text_box.text_color = Some([r, g, b]);
        } else if let Some(idx) = self.selected_idx {
            let text_val = if self.sidebar_text.editing { self.sidebar_text.edit_buffer.clone() } else { self.sidebar_text.text.clone() };
            let x_val = if self.sidebar_x.editing {
                self.sidebar_x.edit_buffer.parse::<f32>().unwrap_or(0.0)
            } else {
                self.sidebar_x.value as f32
            };
            let y_val = if self.sidebar_y.editing {
                self.sidebar_y.edit_buffer.parse::<f32>().unwrap_or(0.0)
            } else {
                self.sidebar_y.value as f32
            };
            let w_val = if self.sidebar_w.editing {
                self.sidebar_w.edit_buffer.parse::<f32>().unwrap_or(100.0)
            } else {
                self.sidebar_w.value as f32
            };
            let h_val = if self.sidebar_h.editing {
                self.sidebar_h.edit_buffer.parse::<f32>().unwrap_or(50.0)
            } else {
                self.sidebar_h.value as f32
            };

            let is_vector = match &self.elements[idx] {
                Element::Vector { .. } => true,
                _ => false,
            };
            let (v_x2, v_y2) = if is_vector {
                let nx2 = if self.sidebar_w.editing {
                    self.sidebar_w.edit_buffer.parse::<f32>().unwrap_or(0.0)
                } else {
                    self.sidebar_w.value as f32
                };
                let ny2 = if self.sidebar_h.editing {
                    self.sidebar_h.edit_buffer.parse::<f32>().unwrap_or(0.0)
                } else {
                    self.sidebar_h.value as f32
                };
                (nx2, ny2)
            } else {
                (0.0, 0.0)
            };
            let w_for_align = if is_vector { (v_x2 - x_val).abs() } else { w_val };
            let h_for_align = if is_vector { (v_y2 - y_val).abs() } else { h_val };
            let size_val = if self.sidebar_size.editing {
                self.sidebar_size.edit_buffer.parse::<f32>().unwrap_or(16.0)
            } else {
                self.sidebar_size.value as f32
            };

            let r_val = self.slider_r.inner().value();
            let g_val = self.slider_g.inner().value();
            let b_val = self.slider_b.inner().value();

            let mut family_str = String::new();
            if let Some(Element::Text { font_family, .. }) = self.elements.get(idx) {
                family_str = font_family.clone();
            }
            if self.font_selector.take_change() {
                family_str = self.font_selector.font_family.clone();
            }

            let mut page_align_h = None;
            if self.dropdown_align_h.take_change() {
                page_align_h = Some(self.dropdown_align_h.selected);
            }
            let mut page_align_v = None;
            if self.dropdown_align_v.take_change() {
                page_align_v = Some(self.dropdown_align_v.selected);
            }

            let mut text_align_h = None;
            if self.dropdown_text_align_h.take_change() {
                text_align_h = Some(match self.dropdown_text_align_h.selected {
                    1 => TextAlignH::Center,
                    2 => TextAlignH::Right,
                    _ => TextAlignH::Left,
                });
            }
            let mut text_align_v = None;
            if self.dropdown_text_align_v.take_change() {
                text_align_v = Some(match self.dropdown_text_align_v.selected {
                    1 => TextAlignV::Middle,
                    2 => TextAlignV::Bottom,
                    _ => TextAlignV::Top,
                });
            }

            let mut final_x = x_val;
            let mut final_y = y_val;

            if let Some(sel_h) = page_align_h {
                if sel_h == 1 {
                    final_x = 0.0;
                } else if sel_h == 2 {
                    final_x = ((self.page_w - w_for_align) / 2.0).round();
                } else if sel_h == 3 {
                    final_x = (self.page_w - w_for_align).round();
                }
            }
            if let Some(sel_v) = page_align_v {
                if sel_v == 1 {
                    final_y = 0.0;
                } else if sel_v == 2 {
                    final_y = ((self.page_h - h_for_align) / 2.0).round();
                } else if sel_v == 3 {
                    final_y = (self.page_h - h_for_align).round();
                }
            }

            let mut final_x2 = v_x2;
            let mut final_y2 = v_y2;
            if is_vector {
                let dx = final_x - x_val;
                let dy = final_y - y_val;
                final_x2 += dx;
                final_y2 += dy;
            }

            match &mut self.elements[idx] {
                Element::Text { text, x, y, w, h, font_size, color, font_family, align_h, align_v, multiline } => {
                    *text = text_val;
                    *w = w_val;
                    *h = h_val;
                    *font_size = size_val;
                    *color = [r_val, g_val, b_val, 1.0];
                    if !family_str.is_empty() {
                        *font_family = family_str;
                    }
                    if let Some(ah) = text_align_h {
                        *align_h = ah;
                    }
                    if let Some(av) = text_align_v {
                        *align_v = av;
                    }
                    *multiline = self.toggle_multiline.toggled();
                    *x = final_x;
                    *y = final_y;
                }
                Element::Shape { shape_type: _, x, y, w, h, color } => {
                    *w = w_val;
                    *h = h_val;
                    *color = [r_val, g_val, b_val, 1.0];
                    *x = final_x;
                    *y = final_y;
                }
                Element::Vector { x1, y1, x2, y2, stroke_width, color, line_cap } => {
                    *x1 = final_x;
                    *y1 = final_y;
                    *x2 = final_x2;
                    *y2 = final_y2;
                    *stroke_width = size_val;
                    *color = [r_val, g_val, b_val, 1.0];
                    *line_cap = match self.dropdown_line_cap.selected {
                        0 => LineCap::Arrow,
                        1 => LineCap::Round,
                        2 => LineCap::Flat,
                        _ => LineCap::Arrow,
                    };
                }
            }
        }
        self.rebuild_layers_tab_widgets();
        self.sync_sidebar_fields();
    }



    pub fn get_current_document(&self) -> LayoutDocument {
        LayoutDocument {
            page_w: self.page_w,
            page_h: self.page_h,
            page_color: self.page_color,
            margin_enabled: self.margin_enabled,
            margin_x: self.margin_x,
            margin_y: self.margin_y,
            word_processor_enabled: self.word_processor_enabled,
            wp_text: self.wp_text_box.text.clone(),
            elements: self.elements.clone(),
            grid_color: self.grid_color,
            grid_size: self.spinbox_grid_size.value as f32 / 10.0,
            margin_color: self.margin_color,
            margin_thickness: self.margin_thickness,
            grid_enabled: self.grid_enabled,
            grid_units: self.dropdown_grid_units.selected,
            zoom: self.slider_zoom.inner().value(),
            rulers_enabled: self.toggle_rulers.toggled(),
            ruler_units: self.dropdown_units.selected,
        }
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.get_current_document() != self.last_saved_document
    }

    pub fn grid_spacing(&self) -> f32 {
        match self.dropdown_grid_units.selected {
            0 => 20.0,                    // Pixels
            1 => 0.25 * 60.0,             // Inches
            2 => 0.5 * 23.622047,         // Centimeters
            3 => 5.0 * 2.3622047,         // Millimeters
            _ => 20.0,
        }
    }

    fn active_page_widgets_unfocus(&mut self) {
        let selected_page = self.paginator.selected_page();
        match selected_page {
            0 => {
                self.btn_new_doc.unfocus();
                self.btn_open.unfocus();
                self.recent_files_list.focused = false;
                for btn in &mut self.recent_files_buttons {
                    btn.unfocus();
                }
                self.btn_save.unfocus();
                self.btn_save_as.unfocus();
                self.btn_exit.unfocus();
            }
            1 => {
                self.dropdown_margin_units.unfocus();
                self.dropdown_presets.unfocus();
                self.slider_page_x.unfocus();
                self.slider_page_y.unfocus();
                self.page_color_selector.unfocus();
                self.toggle_margin.unfocus();
                self.toggle_word_processor.unfocus();
                self.slider_margin_x.unfocus();
                self.slider_margin_y.unfocus();
            }
            2 => {
                self.font_selector.unfocus();
                self.sidebar_size.unfocus();
                self.slider_r.unfocus();
                self.slider_g.unfocus();
                self.slider_b.unfocus();
                self.sidebar_x.unfocus();
                self.sidebar_y.unfocus();
                self.sidebar_w.unfocus();
                self.sidebar_h.unfocus();
                self.sidebar_text.unfocus();
                self.toggle_multiline.unfocus();
                self.dropdown_text_align_h.unfocus();
                self.dropdown_text_align_v.unfocus();
                self.dropdown_align_h.unfocus();
                self.dropdown_align_v.unfocus();
                self.dropdown_line_cap.unfocus();
                self.label_sel_status.unfocus();
                self.label_sel_desc1.unfocus();
                self.label_sel_desc2.unfocus();
            }
            3 => {
                self.btn_add_text.unfocus();
                self.btn_add_rect.unfocus();
                self.btn_add_banner.unfocus();
                self.btn_add_vector.unfocus();
                self.label_total_elements.unfocus();
            }
            4 => {
                self.toggle_grid.unfocus();
                self.grid_color_selector.unfocus();
                self.spinbox_grid_size.unfocus();
                self.label_grid_snap.unfocus();
                self.slider_zoom.unfocus();
                self.toggle_rulers.unfocus();
                self.dropdown_units.unfocus();
                self.margin_color_selector.unfocus();
                self.spinbox_margin_thickness.unfocus();
                self.dropdown_grid_units.unfocus();
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    btn.unfocus();
                }
            }
            _ => {}
        }
    }

    fn active_page_widgets_focus_handle(&mut self, px: f32, py: f32) {
        let ctx = &self.ui_context;
        let selected_page = self.paginator.selected_page();
        let mut hit_idx = None;
        match selected_page {
            0 => {
                if self.btn_new_doc.hit_test(px, py, ctx) { hit_idx = Some(0); }
                else if self.btn_open.hit_test(px, py, ctx) { hit_idx = Some(1); }
                else if self.recent_files_list.hit(px, py) { hit_idx = Some(2); }
                else {
                    for (i, btn) in self.recent_files_buttons.iter().enumerate() {
                        if btn.hit_test(px, py, ctx) {
                            hit_idx = Some(3 + i);
                            break;
                        }
                    }
                    if hit_idx.is_none() {
                        let offset = 3 + self.recent_files_buttons.len();
                        if self.btn_save.hit_test(px, py, ctx) { hit_idx = Some(offset); }
                        else if self.btn_save_as.hit_test(px, py, ctx) { hit_idx = Some(offset + 1); }
                        else if self.btn_exit.hit_test(px, py, ctx) { hit_idx = Some(offset + 2); }
                    }
                }
            }
            1 => {
                if self.dropdown_margin_units.hit_test(px, py, ctx) { hit_idx = Some(0); }
                else if self.dropdown_presets.hit_test(px, py, ctx) { hit_idx = Some(1); }
                else if self.slider_page_x.hit_test(px, py, ctx) { hit_idx = Some(2); }
                else if self.slider_page_y.hit_test(px, py, ctx) { hit_idx = Some(3); }
                else if self.page_color_selector.hit_test(px, py, ctx) { hit_idx = Some(4); }
                else if self.toggle_margin.hit_test(px, py, ctx) { hit_idx = Some(5); }
                else if self.toggle_word_processor.hit_test(px, py, ctx) { hit_idx = Some(6); }
                else if self.slider_margin_x.hit_test(px, py, ctx) { hit_idx = Some(7); }
                else if self.slider_margin_y.hit_test(px, py, ctx) { hit_idx = Some(8); }
            }
            2 => {
                if self.word_processor_enabled {
                    if self.font_selector.hit_test(px, py, ctx) { hit_idx = Some(0); }
                    else if self.sidebar_size.hit_test(px, py, ctx) { hit_idx = Some(1); }
                    else if self.slider_r.hit_test(px, py, ctx) { hit_idx = Some(2); }
                    else if self.slider_g.hit_test(px, py, ctx) { hit_idx = Some(3); }
                    else if self.slider_b.hit_test(px, py, ctx) { hit_idx = Some(4); }
                } else if self.selected_idx.is_some() {
                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if self.sidebar_x.hit_test(px, py, ctx) { hit_idx = Some(0); }
                        else if self.sidebar_y.hit_test(px, py, ctx) { hit_idx = Some(1); }
                        else if self.sidebar_w.hit_test(px, py, ctx) { hit_idx = Some(2); }
                        else if self.sidebar_h.hit_test(px, py, ctx) { hit_idx = Some(3); }
                        else if self.sidebar_text.hit_test(px, py, ctx) { hit_idx = Some(4); }
                        else if self.font_selector.hit_test(px, py, ctx) { hit_idx = Some(5); }
                        else if self.toggle_multiline.hit_test(px, py, ctx) { hit_idx = Some(6); }
                        else if self.dropdown_text_align_h.hit_test(px, py, ctx) { hit_idx = Some(7); }
                        else if self.dropdown_text_align_v.hit_test(px, py, ctx) { hit_idx = Some(8); }
                        else if self.sidebar_size.hit_test(px, py, ctx) { hit_idx = Some(9); }
                        else if self.slider_r.hit_test(px, py, ctx) { hit_idx = Some(10); }
                        else if self.slider_g.hit_test(px, py, ctx) { hit_idx = Some(11); }
                        else if self.slider_b.hit_test(px, py, ctx) { hit_idx = Some(12); }
                    } else if is_vector {
                        if self.sidebar_x.hit_test(px, py, ctx) { hit_idx = Some(0); }
                        else if self.sidebar_y.hit_test(px, py, ctx) { hit_idx = Some(1); }
                        else if self.sidebar_w.hit_test(px, py, ctx) { hit_idx = Some(2); }
                        else if self.sidebar_h.hit_test(px, py, ctx) { hit_idx = Some(3); }
                        else if self.sidebar_size.hit_test(px, py, ctx) { hit_idx = Some(4); }
                        else if self.dropdown_line_cap.hit_test(px, py, ctx) { hit_idx = Some(5); }
                        else if self.slider_r.hit_test(px, py, ctx) { hit_idx = Some(6); }
                        else if self.slider_g.hit_test(px, py, ctx) { hit_idx = Some(7); }
                        else if self.slider_b.hit_test(px, py, ctx) { hit_idx = Some(8); }
                    } else {
                        if self.sidebar_x.hit_test(px, py, ctx) { hit_idx = Some(0); }
                        else if self.sidebar_y.hit_test(px, py, ctx) { hit_idx = Some(1); }
                        else if self.sidebar_w.hit_test(px, py, ctx) { hit_idx = Some(2); }
                        else if self.sidebar_h.hit_test(px, py, ctx) { hit_idx = Some(3); }
                        else if self.slider_r.hit_test(px, py, ctx) { hit_idx = Some(4); }
                        else if self.slider_g.hit_test(px, py, ctx) { hit_idx = Some(5); }
                        else if self.slider_b.hit_test(px, py, ctx) { hit_idx = Some(6); }
                    }
                } else {
                    if self.label_sel_status.hit_test(px, py, ctx) { hit_idx = Some(0); }
                    else if self.label_sel_desc1.hit_test(px, py, ctx) { hit_idx = Some(1); }
                    else if self.label_sel_desc2.hit_test(px, py, ctx) { hit_idx = Some(2); }
                }
            }
            3 => {
                if self.btn_add_text.hit_test(px, py, ctx) { hit_idx = Some(0); }
                else if self.btn_add_rect.hit_test(px, py, ctx) { hit_idx = Some(1); }
                else if self.btn_add_banner.hit_test(px, py, ctx) { hit_idx = Some(2); }
                else if self.btn_add_vector.hit_test(px, py, ctx) { hit_idx = Some(3); }
                else if self.label_total_elements.hit_test(px, py, ctx) { hit_idx = Some(4); }
            }
            4 => {
                if self.toggle_grid.hit_test(px, py, ctx) { hit_idx = Some(0); }
                else if self.grid_color_selector.hit_test(px, py, ctx) { hit_idx = Some(1); }
                else if self.spinbox_grid_size.hit_test(px, py, ctx) { hit_idx = Some(2); }
                else if self.label_grid_snap.hit_test(px, py, ctx) { hit_idx = Some(3); }
                else if self.slider_zoom.hit_test(px, py, ctx) { hit_idx = Some(4); }
                else if self.toggle_rulers.hit_test(px, py, ctx) { hit_idx = Some(5); }
                else if self.dropdown_units.hit_test(px, py, ctx) { hit_idx = Some(6); }
                else if self.margin_color_selector.hit_test(px, py, ctx) { hit_idx = Some(7); }
                else if self.spinbox_margin_thickness.hit_test(px, py, ctx) { hit_idx = Some(8); }
                else if self.dropdown_grid_units.hit_test(px, py, ctx) { hit_idx = Some(9); }
            }
            5 => {
                for (i, btn) in self.layer_buttons.iter().enumerate() {
                    if btn.hit_test(px, py, ctx) {
                        hit_idx = Some(i);
                        break;
                    }
                }
            }
            _ => {}
        }

        match selected_page {
            0 => {
                if hit_idx == Some(0) { self.btn_new_doc.focus(); } else { self.btn_new_doc.unfocus(); }
                if hit_idx == Some(1) { self.btn_open.focus(); } else { self.btn_open.unfocus(); }
                self.recent_files_list.focused = hit_idx == Some(2);
                for (i, btn) in self.recent_files_buttons.iter_mut().enumerate() {
                    if hit_idx == Some(3 + i) { btn.focus(); } else { btn.unfocus(); }
                }
                let offset = 3 + self.recent_files_buttons.len();
                if hit_idx == Some(offset) { self.btn_save.focus(); } else { self.btn_save.unfocus(); }
                if hit_idx == Some(offset + 1) { self.btn_save_as.focus(); } else { self.btn_save_as.unfocus(); }
                if hit_idx == Some(offset + 2) { self.btn_exit.focus(); } else { self.btn_exit.unfocus(); }
            }
            1 => {
                if hit_idx == Some(0) { self.dropdown_margin_units.focus(); } else { self.dropdown_margin_units.unfocus(); }
                if hit_idx == Some(1) { self.dropdown_presets.focus(); } else { self.dropdown_presets.unfocus(); }
                if hit_idx == Some(2) { self.slider_page_x.focus(); } else { self.slider_page_x.unfocus(); }
                if hit_idx == Some(3) { self.slider_page_y.focus(); } else { self.slider_page_y.unfocus(); }
                if hit_idx == Some(4) { self.page_color_selector.focus(); } else { self.page_color_selector.unfocus(); }
                if hit_idx == Some(5) { self.toggle_margin.focus(); } else { self.toggle_margin.unfocus(); }
                if hit_idx == Some(6) { self.toggle_word_processor.focus(); } else { self.toggle_word_processor.unfocus(); }
                if hit_idx == Some(7) { self.slider_margin_x.focus(); } else { self.slider_margin_x.unfocus(); }
                if hit_idx == Some(8) { self.slider_margin_y.focus(); } else { self.slider_margin_y.unfocus(); }
            }
            2 => {
                if self.word_processor_enabled {
                    if hit_idx == Some(0) { self.font_selector.focus(); } else { self.font_selector.unfocus(); }
                    if hit_idx == Some(1) { self.sidebar_size.focus(); } else { self.sidebar_size.unfocus(); }
                    if hit_idx == Some(2) { self.slider_r.focus(); } else { self.slider_r.unfocus(); }
                    if hit_idx == Some(3) { self.slider_g.focus(); } else { self.slider_g.unfocus(); }
                    if hit_idx == Some(4) { self.slider_b.focus(); } else { self.slider_b.unfocus(); }
                } else if self.selected_idx.is_some() {
                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if hit_idx == Some(0) { self.sidebar_x.focus(); } else { self.sidebar_x.unfocus(); }
                        if hit_idx == Some(1) { self.sidebar_y.focus(); } else { self.sidebar_y.unfocus(); }
                        if hit_idx == Some(2) { self.sidebar_w.focus(); } else { self.sidebar_w.unfocus(); }
                        if hit_idx == Some(3) { self.sidebar_h.focus(); } else { self.sidebar_h.unfocus(); }
                        if hit_idx == Some(4) { self.sidebar_text.focus(); } else { self.sidebar_text.unfocus(); }
                        if hit_idx == Some(5) { self.font_selector.focus(); } else { self.font_selector.unfocus(); }
                        if hit_idx == Some(6) { self.toggle_multiline.focus(); } else { self.toggle_multiline.unfocus(); }
                        if hit_idx == Some(7) { self.dropdown_text_align_h.focus(); } else { self.dropdown_text_align_h.unfocus(); }
                        if hit_idx == Some(8) { self.dropdown_text_align_v.focus(); } else { self.dropdown_text_align_v.unfocus(); }
                        if hit_idx == Some(9) { self.sidebar_size.focus(); } else { self.sidebar_size.unfocus(); }
                        if hit_idx == Some(10) { self.slider_r.focus(); } else { self.slider_r.unfocus(); }
                        if hit_idx == Some(11) { self.slider_g.focus(); } else { self.slider_g.unfocus(); }
                        if hit_idx == Some(12) { self.slider_b.focus(); } else { self.slider_b.unfocus(); }
                    } else if is_vector {
                        if hit_idx == Some(0) { self.sidebar_x.focus(); } else { self.sidebar_x.unfocus(); }
                        if hit_idx == Some(1) { self.sidebar_y.focus(); } else { self.sidebar_y.unfocus(); }
                        if hit_idx == Some(2) { self.sidebar_w.focus(); } else { self.sidebar_w.unfocus(); }
                        if hit_idx == Some(3) { self.sidebar_h.focus(); } else { self.sidebar_h.unfocus(); }
                        if hit_idx == Some(4) { self.sidebar_size.focus(); } else { self.sidebar_size.unfocus(); }
                        if hit_idx == Some(5) { self.dropdown_line_cap.focus(); } else { self.dropdown_line_cap.unfocus(); }
                        if hit_idx == Some(6) { self.slider_r.focus(); } else { self.slider_r.unfocus(); }
                        if hit_idx == Some(7) { self.slider_g.focus(); } else { self.slider_g.unfocus(); }
                        if hit_idx == Some(8) { self.slider_b.focus(); } else { self.slider_b.unfocus(); }
                    } else {
                        if hit_idx == Some(0) { self.sidebar_x.focus(); } else { self.sidebar_x.unfocus(); }
                        if hit_idx == Some(1) { self.sidebar_y.focus(); } else { self.sidebar_y.unfocus(); }
                        if hit_idx == Some(2) { self.sidebar_w.focus(); } else { self.sidebar_w.unfocus(); }
                        if hit_idx == Some(3) { self.sidebar_h.focus(); } else { self.sidebar_h.unfocus(); }
                        if hit_idx == Some(4) { self.slider_r.focus(); } else { self.slider_r.unfocus(); }
                        if hit_idx == Some(5) { self.slider_g.focus(); } else { self.slider_g.unfocus(); }
                        if hit_idx == Some(6) { self.slider_b.focus(); } else { self.slider_b.unfocus(); }
                    }
                } else {
                    if hit_idx == Some(0) { self.label_sel_status.focus(); } else { self.label_sel_status.unfocus(); }
                    if hit_idx == Some(1) { self.label_sel_desc1.focus(); } else { self.label_sel_desc1.unfocus(); }
                    if hit_idx == Some(2) { self.label_sel_desc2.focus(); } else { self.label_sel_desc2.unfocus(); }
                }
            }
            3 => {
                if hit_idx == Some(0) { self.btn_add_text.focus(); } else { self.btn_add_text.unfocus(); }
                if hit_idx == Some(1) { self.btn_add_rect.focus(); } else { self.btn_add_rect.unfocus(); }
                if hit_idx == Some(2) { self.btn_add_banner.focus(); } else { self.btn_add_banner.unfocus(); }
                if hit_idx == Some(3) { self.btn_add_vector.focus(); } else { self.btn_add_vector.unfocus(); }
                if hit_idx == Some(4) { self.label_total_elements.focus(); } else { self.label_total_elements.unfocus(); }
            }
            4 => {
                if hit_idx == Some(0) { self.toggle_grid.focus(); } else { self.toggle_grid.unfocus(); }
                if hit_idx == Some(1) { self.grid_color_selector.focus(); } else { self.grid_color_selector.unfocus(); }
                if hit_idx == Some(2) { self.spinbox_grid_size.focus(); } else { self.spinbox_grid_size.unfocus(); }
                if hit_idx == Some(3) { self.label_grid_snap.focus(); } else { self.label_grid_snap.unfocus(); }
                if hit_idx == Some(4) { self.slider_zoom.focus(); } else { self.slider_zoom.unfocus(); }
                if hit_idx == Some(5) { self.toggle_rulers.focus(); } else { self.toggle_rulers.unfocus(); }
                if hit_idx == Some(6) { self.dropdown_units.focus(); } else { self.dropdown_units.unfocus(); }
                if hit_idx == Some(7) { self.margin_color_selector.focus(); } else { self.margin_color_selector.unfocus(); }
                if hit_idx == Some(8) { self.spinbox_margin_thickness.focus(); } else { self.spinbox_margin_thickness.unfocus(); }
                if hit_idx == Some(9) { self.dropdown_grid_units.focus(); } else { self.dropdown_grid_units.unfocus(); }
            }
            5 => {
                for (i, btn) in self.layer_buttons.iter_mut().enumerate() {
                    if hit_idx == Some(i) { btn.focus(); } else { btn.unfocus(); }
                }
            }
            _ => {}
        }
    }

    fn active_page_widgets_cursor_moved(&mut self, px: f32, py: f32) -> bool {
        let mut changed = false;
        // Routed dispatch (6bd shrink): one Event per widget root through the router.
        let mv = cce_ui::widget::Event::PointerMove { x: px, y: py, local_x: px, local_y: py };
        let ctx = &mut self.ui_context;
        let selected_page = self.paginator.selected_page();
        match selected_page {
            0 => {
                if ctx.propagate_event(&mv, self.btn_new_doc.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.btn_open.id()) { changed = true; }
                if self.recent_files_list.cursor_moved(px, py) { changed = true; }
                for btn in &mut self.recent_files_buttons {
                    if ctx.propagate_event(&mv, btn.id()) { changed = true; }
                }
                if ctx.propagate_event(&mv, self.btn_save.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.btn_save_as.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.btn_exit.id()) { changed = true; }
            }
            1 => {
                if ctx.propagate_event(&mv, self.dropdown_margin_units.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.dropdown_presets.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.slider_page_x.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.slider_page_y.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.page_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.toggle_margin.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.toggle_word_processor.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.slider_margin_x.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.slider_margin_y.id()) { changed = true; }
            }
            2 => {
                if self.word_processor_enabled {
                    if ctx.propagate_event(&mv, self.font_selector.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.sidebar_size.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.slider_r.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.slider_g.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.slider_b.id()) { changed = true; }
                } else if self.selected_idx.is_some() {
                    if ctx.propagate_event(&mv, self.dropdown_align_h.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.dropdown_align_v.id()) { changed = true; }

                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if ctx.propagate_event(&mv, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_text.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.font_selector.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.toggle_multiline.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.dropdown_text_align_h.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.dropdown_text_align_v.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_size.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_b.id()) { changed = true; }
                    } else if is_vector {
                        if ctx.propagate_event(&mv, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_size.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.dropdown_line_cap.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_b.id()) { changed = true; }
                    } else {
                        if ctx.propagate_event(&mv, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&mv, self.slider_b.id()) { changed = true; }
                    }
                } else {
                    if ctx.propagate_event(&mv, self.label_sel_status.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.label_sel_desc1.id()) { changed = true; }
                    if ctx.propagate_event(&mv, self.label_sel_desc2.id()) { changed = true; }
                }
            }
            3 => {
                if ctx.propagate_event(&mv, self.btn_add_text.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.btn_add_rect.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.btn_add_banner.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.btn_add_vector.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.label_total_elements.id()) { changed = true; }
            }
            4 => {
                if ctx.propagate_event(&mv, self.toggle_grid.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.grid_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.spinbox_grid_size.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.label_grid_snap.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.slider_zoom.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.toggle_rulers.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.dropdown_units.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.margin_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.spinbox_margin_thickness.id()) { changed = true; }
                if ctx.propagate_event(&mv, self.dropdown_grid_units.id()) { changed = true; }
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    if ctx.propagate_event(&mv, btn.id()) { changed = true; }
                }
            }
            _ => {}
        }
        changed
    }

    fn active_page_widgets_mouse_input(&mut self, button: MouseButton, state: ElementState, px: f32, py: f32) -> bool {
        let mut changed = false;
        let ev = cce_ui::widget::Event::MouseButton { button, state, x: px, y: py, local_x: px, local_y: py };
        let ctx = &mut self.ui_context;
        let selected_page = self.paginator.selected_page();
        match selected_page {
            0 => {
                if ctx.propagate_event(&ev, self.btn_new_doc.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.btn_open.id()) { changed = true; }
                if button == MouseButton::Left {
                    let handled = match state {
                        ElementState::Pressed => self.recent_files_list.press(px, py),
                        ElementState::Released => self.recent_files_list.release(),
                    };
                    if handled { changed = true; }
                }
                for btn in &mut self.recent_files_buttons {
                    if ctx.propagate_event(&ev, btn.id()) { changed = true; }
                }
                if ctx.propagate_event(&ev, self.btn_save.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.btn_save_as.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.btn_exit.id()) { changed = true; }
            }
            1 => {
                if ctx.propagate_event(&ev, self.dropdown_margin_units.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.dropdown_presets.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.slider_page_x.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.slider_page_y.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.page_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.toggle_margin.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.toggle_word_processor.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.slider_margin_x.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.slider_margin_y.id()) { changed = true; }
            }
            2 => {
                if self.word_processor_enabled {
                    if ctx.propagate_event(&ev, self.font_selector.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.sidebar_size.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.slider_r.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.slider_g.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.slider_b.id()) { changed = true; }
                } else if self.selected_idx.is_some() {
                    if ctx.propagate_event(&ev, self.dropdown_align_h.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.dropdown_align_v.id()) { changed = true; }

                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if ctx.propagate_event(&ev, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_text.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.font_selector.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.toggle_multiline.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.dropdown_text_align_h.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.dropdown_text_align_v.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_size.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_b.id()) { changed = true; }
                    } else if is_vector {
                        if ctx.propagate_event(&ev, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_size.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.dropdown_line_cap.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_b.id()) { changed = true; }
                    } else {
                        if ctx.propagate_event(&ev, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&ev, self.slider_b.id()) { changed = true; }
                    }
                } else {
                    if ctx.propagate_event(&ev, self.label_sel_status.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.label_sel_desc1.id()) { changed = true; }
                    if ctx.propagate_event(&ev, self.label_sel_desc2.id()) { changed = true; }
                }
            }
            3 => {
                if ctx.propagate_event(&ev, self.btn_add_text.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.btn_add_rect.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.btn_add_banner.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.btn_add_vector.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.label_total_elements.id()) { changed = true; }
            }
            4 => {
                if ctx.propagate_event(&ev, self.toggle_grid.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.grid_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.spinbox_grid_size.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.label_grid_snap.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.slider_zoom.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.toggle_rulers.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.dropdown_units.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.margin_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.spinbox_margin_thickness.id()) { changed = true; }
                if ctx.propagate_event(&ev, self.dropdown_grid_units.id()) { changed = true; }
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    if ctx.propagate_event(&ev, btn.id()) { changed = true; }
                }
            }
            _ => {}
        }
        changed
    }

    fn active_page_widgets_mouse_wheel(&mut self, delta: &MouseScrollDelta, px: f32, py: f32) -> bool {
        let mut changed = false;
        let wev = cce_ui::widget::Event::MouseWheel { delta: *delta, x: px, y: py, local_x: px, local_y: py };
        let ctx = &mut self.ui_context;
        let selected_page = self.paginator.selected_page();
        match selected_page {
            0 => {
                if ctx.propagate_event(&wev, self.btn_new_doc.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.btn_open.id()) { changed = true; }
                if self.recent_files_list.wheel(delta, px, py) { changed = true; }
                for btn in &mut self.recent_files_buttons {
                    if ctx.propagate_event(&wev, btn.id()) { changed = true; }
                }
                if ctx.propagate_event(&wev, self.btn_save.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.btn_save_as.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.btn_exit.id()) { changed = true; }
            }
            1 => {
                if ctx.propagate_event(&wev, self.dropdown_margin_units.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.dropdown_presets.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.slider_page_x.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.slider_page_y.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.page_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.toggle_margin.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.toggle_word_processor.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.slider_margin_x.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.slider_margin_y.id()) { changed = true; }
            }
            2 => {
                if self.word_processor_enabled {
                    if ctx.propagate_event(&wev, self.font_selector.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.sidebar_size.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.slider_r.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.slider_g.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.slider_b.id()) { changed = true; }
                } else if self.selected_idx.is_some() {
                    if ctx.propagate_event(&wev, self.dropdown_align_h.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.dropdown_align_v.id()) { changed = true; }

                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if ctx.propagate_event(&wev, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_text.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.font_selector.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.toggle_multiline.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.dropdown_text_align_h.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.dropdown_text_align_v.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_size.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_b.id()) { changed = true; }
                    } else if is_vector {
                        if ctx.propagate_event(&wev, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_size.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.dropdown_line_cap.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_b.id()) { changed = true; }
                    } else {
                        if ctx.propagate_event(&wev, self.sidebar_x.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_y.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_w.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.sidebar_h.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_r.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_g.id()) { changed = true; }
                        if ctx.propagate_event(&wev, self.slider_b.id()) { changed = true; }
                    }
                } else {
                    if ctx.propagate_event(&wev, self.label_sel_status.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.label_sel_desc1.id()) { changed = true; }
                    if ctx.propagate_event(&wev, self.label_sel_desc2.id()) { changed = true; }
                }
            }
            3 => {
                if ctx.propagate_event(&wev, self.btn_add_text.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.btn_add_rect.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.btn_add_banner.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.btn_add_vector.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.label_total_elements.id()) { changed = true; }
            }
            4 => {
                if ctx.propagate_event(&wev, self.toggle_grid.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.grid_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.spinbox_grid_size.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.label_grid_snap.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.slider_zoom.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.toggle_rulers.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.dropdown_units.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.margin_color_selector.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.spinbox_margin_thickness.id()) { changed = true; }
                if ctx.propagate_event(&wev, self.dropdown_grid_units.id()) { changed = true; }
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    if ctx.propagate_event(&wev, btn.id()) { changed = true; }
                }
            }
            _ => {}
        }
        changed
    }

    fn active_page_widgets_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        let mut changed = false;
        let kev = cce_ui::widget::Event::KeyInput(event.clone());
        let ctx = &mut self.ui_context;
        let selected_page = self.paginator.selected_page();
        match selected_page {
            0 => {
                if self.btn_new_doc.focused(ctx) { if ctx.propagate_event(&kev, self.btn_new_doc.id()) { changed = true; } }
                if self.btn_open.focused(ctx) { if ctx.propagate_event(&kev, self.btn_open.id()) { changed = true; } }
                if self.recent_files_list.focused { if self.recent_files_list.keyboard(event) { changed = true; } }
                for btn in &mut self.recent_files_buttons {
                    if btn.focused(ctx) { if ctx.propagate_event(&kev, btn.id()) { changed = true; } }
                }
                if self.btn_save.focused(ctx) { if ctx.propagate_event(&kev, self.btn_save.id()) { changed = true; } }
                if self.btn_save_as.focused(ctx) { if ctx.propagate_event(&kev, self.btn_save_as.id()) { changed = true; } }
                if self.btn_exit.focused(ctx) { if ctx.propagate_event(&kev, self.btn_exit.id()) { changed = true; } }
            }
            1 => {
                if self.dropdown_margin_units.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_margin_units.id()) { changed = true; } }
                if self.dropdown_presets.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_presets.id()) { changed = true; } }
                if self.slider_page_x.focused(ctx) { if ctx.propagate_event(&kev, self.slider_page_x.id()) { changed = true; } }
                if self.slider_page_y.focused(ctx) { if ctx.propagate_event(&kev, self.slider_page_y.id()) { changed = true; } }
                if self.page_color_selector.focused(ctx) { if ctx.propagate_event(&kev, self.page_color_selector.id()) { changed = true; } }
                if self.toggle_margin.focused(ctx) { if ctx.propagate_event(&kev, self.toggle_margin.id()) { changed = true; } }
                if self.toggle_word_processor.focused(ctx) { if ctx.propagate_event(&kev, self.toggle_word_processor.id()) { changed = true; } }
                if self.slider_margin_x.focused(ctx) { if ctx.propagate_event(&kev, self.slider_margin_x.id()) { changed = true; } }
                if self.slider_margin_y.focused(ctx) { if ctx.propagate_event(&kev, self.slider_margin_y.id()) { changed = true; } }
            }
            2 => {
                if self.word_processor_enabled {
                    if self.font_selector.focused(ctx) { if ctx.propagate_event(&kev, self.font_selector.id()) { changed = true; } }
                    if self.sidebar_size.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_size.id()) { changed = true; } }
                    if self.slider_r.focused(ctx) { if ctx.propagate_event(&kev, self.slider_r.id()) { changed = true; } }
                    if self.slider_g.focused(ctx) { if ctx.propagate_event(&kev, self.slider_g.id()) { changed = true; } }
                    if self.slider_b.focused(ctx) { if ctx.propagate_event(&kev, self.slider_b.id()) { changed = true; } }
                } else if self.selected_idx.is_some() {
                    if self.dropdown_align_h.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_align_h.id()) { changed = true; } }
                    if self.dropdown_align_v.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_align_v.id()) { changed = true; } }

                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if self.sidebar_x.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_x.id()) { changed = true; } }
                        if self.sidebar_y.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_y.id()) { changed = true; } }
                        if self.sidebar_w.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_w.id()) { changed = true; } }
                        if self.sidebar_h.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_h.id()) { changed = true; } }
                        if self.sidebar_text.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_text.id()) { changed = true; } }
                        if self.font_selector.focused(ctx) { if ctx.propagate_event(&kev, self.font_selector.id()) { changed = true; } }
                        if self.toggle_multiline.focused(ctx) { if ctx.propagate_event(&kev, self.toggle_multiline.id()) { changed = true; } }
                        if self.dropdown_text_align_h.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_text_align_h.id()) { changed = true; } }
                        if self.dropdown_text_align_v.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_text_align_v.id()) { changed = true; } }
                        if self.sidebar_size.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_size.id()) { changed = true; } }
                        if self.slider_r.focused(ctx) { if ctx.propagate_event(&kev, self.slider_r.id()) { changed = true; } }
                        if self.slider_g.focused(ctx) { if ctx.propagate_event(&kev, self.slider_g.id()) { changed = true; } }
                        if self.slider_b.focused(ctx) { if ctx.propagate_event(&kev, self.slider_b.id()) { changed = true; } }
                    } else if is_vector {
                        if self.sidebar_x.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_x.id()) { changed = true; } }
                        if self.sidebar_y.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_y.id()) { changed = true; } }
                        if self.sidebar_w.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_w.id()) { changed = true; } }
                        if self.sidebar_h.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_h.id()) { changed = true; } }
                        if self.sidebar_size.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_size.id()) { changed = true; } }
                        if self.dropdown_line_cap.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_line_cap.id()) { changed = true; } }
                        if self.slider_r.focused(ctx) { if ctx.propagate_event(&kev, self.slider_r.id()) { changed = true; } }
                        if self.slider_g.focused(ctx) { if ctx.propagate_event(&kev, self.slider_g.id()) { changed = true; } }
                        if self.slider_b.focused(ctx) { if ctx.propagate_event(&kev, self.slider_b.id()) { changed = true; } }
                    } else {
                        if self.sidebar_x.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_x.id()) { changed = true; } }
                        if self.sidebar_y.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_y.id()) { changed = true; } }
                        if self.sidebar_w.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_w.id()) { changed = true; } }
                        if self.sidebar_h.focused(ctx) { if ctx.propagate_event(&kev, self.sidebar_h.id()) { changed = true; } }
                        if self.slider_r.focused(ctx) { if ctx.propagate_event(&kev, self.slider_r.id()) { changed = true; } }
                        if self.slider_g.focused(ctx) { if ctx.propagate_event(&kev, self.slider_g.id()) { changed = true; } }
                        if self.slider_b.focused(ctx) { if ctx.propagate_event(&kev, self.slider_b.id()) { changed = true; } }
                    }
                } else {
                    if self.label_sel_status.focused(ctx) { if ctx.propagate_event(&kev, self.label_sel_status.id()) { changed = true; } }
                    if self.label_sel_desc1.focused(ctx) { if ctx.propagate_event(&kev, self.label_sel_desc1.id()) { changed = true; } }
                    if self.label_sel_desc2.focused(ctx) { if ctx.propagate_event(&kev, self.label_sel_desc2.id()) { changed = true; } }
                }
            }
            3 => {
                if self.btn_add_text.focused(ctx) { if ctx.propagate_event(&kev, self.btn_add_text.id()) { changed = true; } }
                if self.btn_add_rect.focused(ctx) { if ctx.propagate_event(&kev, self.btn_add_rect.id()) { changed = true; } }
                if self.btn_add_banner.focused(ctx) { if ctx.propagate_event(&kev, self.btn_add_banner.id()) { changed = true; } }
                if self.btn_add_vector.focused(ctx) { if ctx.propagate_event(&kev, self.btn_add_vector.id()) { changed = true; } }
                if self.label_total_elements.focused(ctx) { if ctx.propagate_event(&kev, self.label_total_elements.id()) { changed = true; } }
            }
            4 => {
                if self.toggle_grid.focused(ctx) { if ctx.propagate_event(&kev, self.toggle_grid.id()) { changed = true; } }
                if self.grid_color_selector.focused(ctx) { if ctx.propagate_event(&kev, self.grid_color_selector.id()) { changed = true; } }
                if self.spinbox_grid_size.focused(ctx) { if ctx.propagate_event(&kev, self.spinbox_grid_size.id()) { changed = true; } }
                if self.label_grid_snap.focused(ctx) { if ctx.propagate_event(&kev, self.label_grid_snap.id()) { changed = true; } }
                if self.slider_zoom.focused(ctx) { if ctx.propagate_event(&kev, self.slider_zoom.id()) { changed = true; } }
                if self.toggle_rulers.focused(ctx) { if ctx.propagate_event(&kev, self.toggle_rulers.id()) { changed = true; } }
                if self.dropdown_units.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_units.id()) { changed = true; } }
                if self.margin_color_selector.focused(ctx) { if ctx.propagate_event(&kev, self.margin_color_selector.id()) { changed = true; } }
                if self.spinbox_margin_thickness.focused(ctx) { if ctx.propagate_event(&kev, self.spinbox_margin_thickness.id()) { changed = true; } }
                if self.dropdown_grid_units.focused(ctx) { if ctx.propagate_event(&kev, self.dropdown_grid_units.id()) { changed = true; } }
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    if btn.focused(ctx) { if ctx.propagate_event(&kev, btn.id()) { changed = true; } }
                }
            }
            _ => {}
        }
        changed
    }

    fn active_page_widgets_tick(&mut self, dt: f32) -> bool {
        let mut changed = false;
        let ctx = &mut self.ui_context;
        let selected_page = self.paginator.selected_page();
        match selected_page {
            0 => {
                if self.btn_new_doc.tick(dt, ctx) { changed = true; }
                if self.btn_open.tick(dt, ctx) { changed = true; }
                for btn in &mut self.recent_files_buttons {
                    if btn.tick(dt, ctx) { changed = true; }
                }
                if self.btn_save.tick(dt, ctx) { changed = true; }
                if self.btn_save_as.tick(dt, ctx) { changed = true; }
                if self.btn_exit.tick(dt, ctx) { changed = true; }
            }
            1 => {
                if self.dropdown_margin_units.tick(dt, ctx) { changed = true; }
                if self.dropdown_presets.tick(dt, ctx) { changed = true; }
                if self.slider_page_x.tick(dt, ctx) { changed = true; }
                if self.slider_page_y.tick(dt, ctx) { changed = true; }
                if self.page_color_selector.tick(dt, ctx) { changed = true; }
                if self.toggle_margin.tick(dt, ctx) { changed = true; }
                if self.toggle_word_processor.tick(dt, ctx) { changed = true; }
                if self.slider_margin_x.tick(dt, ctx) { changed = true; }
                if self.slider_margin_y.tick(dt, ctx) { changed = true; }
            }
            2 => {
                if self.word_processor_enabled {
                    if self.font_selector.tick(dt, ctx) { changed = true; }
                    if self.sidebar_size.tick(dt, ctx) { changed = true; }
                    if self.slider_r.tick(dt, ctx) { changed = true; }
                    if self.slider_g.tick(dt, ctx) { changed = true; }
                    if self.slider_b.tick(dt, ctx) { changed = true; }
                } else if self.selected_idx.is_some() {
                    if self.dropdown_align_h.tick(dt, ctx) { changed = true; }
                    if self.dropdown_align_v.tick(dt, ctx) { changed = true; }

                    let (is_text, is_vector) = match self.selected_idx.map(|idx| &self.elements[idx]) {
                        Some(Element::Text { .. }) => (true, false),
                        Some(Element::Vector { .. }) => (false, true),
                        _ => (false, false),
                    };
                    if is_text {
                        if self.sidebar_x.tick(dt, ctx) { changed = true; }
                        if self.sidebar_y.tick(dt, ctx) { changed = true; }
                        if self.sidebar_w.tick(dt, ctx) { changed = true; }
                        if self.sidebar_h.tick(dt, ctx) { changed = true; }
                        if self.sidebar_text.tick(dt, ctx) { changed = true; }
                        if self.font_selector.tick(dt, ctx) { changed = true; }
                        if self.toggle_multiline.tick(dt, ctx) { changed = true; }
                        if self.dropdown_text_align_h.tick(dt, ctx) { changed = true; }
                        if self.dropdown_text_align_v.tick(dt, ctx) { changed = true; }
                        if self.sidebar_size.tick(dt, ctx) { changed = true; }
                        if self.slider_r.tick(dt, ctx) { changed = true; }
                        if self.slider_g.tick(dt, ctx) { changed = true; }
                        if self.slider_b.tick(dt, ctx) { changed = true; }
                    } else if is_vector {
                        if self.sidebar_x.tick(dt, ctx) { changed = true; }
                        if self.sidebar_y.tick(dt, ctx) { changed = true; }
                        if self.sidebar_w.tick(dt, ctx) { changed = true; }
                        if self.sidebar_h.tick(dt, ctx) { changed = true; }
                        if self.sidebar_size.tick(dt, ctx) { changed = true; }
                        if self.dropdown_line_cap.tick(dt, ctx) { changed = true; }
                        if self.slider_r.tick(dt, ctx) { changed = true; }
                        if self.slider_g.tick(dt, ctx) { changed = true; }
                        if self.slider_b.tick(dt, ctx) { changed = true; }
                    } else {
                        if self.sidebar_x.tick(dt, ctx) { changed = true; }
                        if self.sidebar_y.tick(dt, ctx) { changed = true; }
                        if self.sidebar_w.tick(dt, ctx) { changed = true; }
                        if self.sidebar_h.tick(dt, ctx) { changed = true; }
                        if self.slider_r.tick(dt, ctx) { changed = true; }
                        if self.slider_g.tick(dt, ctx) { changed = true; }
                        if self.slider_b.tick(dt, ctx) { changed = true; }
                    }
                } else {
                    if self.label_sel_status.tick(dt, ctx) { changed = true; }
                    if self.label_sel_desc1.tick(dt, ctx) { changed = true; }
                    if self.label_sel_desc2.tick(dt, ctx) { changed = true; }
                }
            }
            3 => {
                if self.btn_add_text.tick(dt, ctx) { changed = true; }
                if self.btn_add_rect.tick(dt, ctx) { changed = true; }
                if self.btn_add_banner.tick(dt, ctx) { changed = true; }
                if self.btn_add_vector.tick(dt, ctx) { changed = true; }
                if self.label_total_elements.tick(dt, ctx) { changed = true; }
            }
            4 => {
                if self.toggle_grid.tick(dt, ctx) { changed = true; }
                if self.grid_color_selector.tick(dt, ctx) { changed = true; }
                if self.spinbox_grid_size.tick(dt, ctx) { changed = true; }
                if self.label_grid_snap.tick(dt, ctx) { changed = true; }
                if self.slider_zoom.tick(dt, ctx) { changed = true; }
                if self.toggle_rulers.tick(dt, ctx) { changed = true; }
                if self.dropdown_units.tick(dt, ctx) { changed = true; }
                if self.margin_color_selector.tick(dt, ctx) { changed = true; }
                if self.spinbox_margin_thickness.tick(dt, ctx) { changed = true; }
                if self.dropdown_grid_units.tick(dt, ctx) { changed = true; }
            }
            5 => {
                for btn in &mut self.layer_buttons {
                    if btn.tick(dt, ctx) { changed = true; }
                }
            }
            _ => {}
        }
        changed
    }
}

/// Adapts the ported view() body's `quads.push`/`.extend` calls to the single paint path.
struct __LayoutQuadSink<'a> {
    pc: &'a mut cce_ui::scene::paint::PaintCtx,
}
impl<'a> __LayoutQuadSink<'a> {
    fn push(&mut self, q: (f32, f32, f32, f32, [f32; 4])) {
        self.pc.quad(cce_ui::scene::layout::Rect { x: q.0, y: q.1, width: q.2, height: q.3 }, q.4);
    }
    fn extend<I: IntoIterator<Item = (f32, f32, f32, f32, [f32; 4])>>(&mut self, it: I) {
        for q in it {
            self.push(q);
        }
    }
}

impl Application for LayoutApp {
    type Message = AppMessage;

    fn ui_context(&self) -> Option<&cce_ui::context::UiContext> {
        Some(&self.ui_context)
    }

    // The engine ticks the exposed context each loop — this is what drives the
    // dropdown expand/contract animation frames.
    fn ui_context_mut(&mut self) -> Option<&mut cce_ui::context::UiContext> {
        Some(&mut self.ui_context)
    }

    fn new(_qh: &QueueHandle<EngineState<Self>>, _sender: calloop::channel::Sender<Self::Message>) -> Self {
        cce_ui::scale::set_scale_factor(1.0);
        let btn_new_doc = Button::new(0.0, 0.0, 240.0, 26.0).with_label("New Document");
        let btn_open = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Open");
        let btn_save = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Save");
        let btn_save_as = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Save As");
        let btn_exit = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Exit Application");

        let paginator = Paginator::new(vec![
            "File".to_string(),
            "Page".to_string(),
            "Element".to_string(),
            "Canvas".to_string(),
            "Guides".to_string(),
            "Layers".to_string(),
        ]).with_title("LAYOUT");

        // Default paper sheet sizing (Letter)
        let page_w = 510.0;
        let page_h = 660.0;

        // Initialize elements relative to the paper sheet top-left (0, 0)
        let elements = vec![];

        // Create sidebar dropdown for page size presets
        let mut dropdown_presets = Dropdown::new(
            PRESETS.iter().map(|p| p.name.to_string()).collect(),
            0
        ).with_label("Page Size Preset");
        dropdown_presets.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut slider_page_x = Slider::new().with_range(100.0, 1500.0).with_value((page_w - 100.0) / (1500.0 - 100.0)).with_readout(true).with_label("Page Width (X)");
        slider_page_x.set_rect(0.0, 0.0, 240.0, 18.0);
        let mut slider_page_y = Slider::new().with_range(100.0, 1500.0).with_value((page_h - 100.0) / (1500.0 - 100.0)).with_readout(true).with_label("Page Height (Y)");
        slider_page_y.set_rect(0.0, 0.0, 240.0, 18.0);

        let mut page_color_selector = ColorSelector::new([245, 245, 250])
            .with_label("Page Color");
        page_color_selector.set_rect(0.0, 0.0, 240.0, 26.0);
        let page_color = [0.96, 0.96, 0.98, 1.0];

        let mut toggle_margin = Toggle::new().with_label("Show Page Margin");
        toggle_margin.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut toggle_word_processor = Toggle::new().with_label("Word Processor");
        toggle_word_processor.set_rect(0.0, 0.0, 240.0, 26.0);

        let wp_text_box = TextBox::new(String::new())
            .with_multiline(true)
            .with_draw_bg_border(false)
            .with_text_color(Some([0x1a, 0x1a, 0x24]))
            .with_max_width(None);

        let mut font_selector = FontSelector::new("monospace".to_string()).with_label("Font");
        font_selector.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_text_align_h = Dropdown::new(
            vec![
                "Left".to_string(),
                "Center".to_string(),
                "Right".to_string(),
            ],
            0,
        ).with_label("Horizontal Alignment");
        dropdown_text_align_h.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_text_align_v = Dropdown::new(
            vec![
                "Top".to_string(),
                "Middle".to_string(),
                "Bottom".to_string(),
            ],
            0,
        ).with_label("Vertical Alignment");
        dropdown_text_align_v.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut toggle_multiline = Toggle::new().with_label("Multiline Text");
        toggle_multiline.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_align_h = Dropdown::new(
            vec![
                "--".to_string(),
                "Left".to_string(),
                "Center".to_string(),
                "Right".to_string(),
            ],
            0,
        ).with_label("Horizontal Alignment");
        dropdown_align_h.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_align_v = Dropdown::new(
            vec![
                "--".to_string(),
                "Top".to_string(),
                "Middle".to_string(),
                "Bottom".to_string(),
            ],
            0,
        ).with_label("Vertical Alignment");
        dropdown_align_v.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_margin_units = Dropdown::new(
            vec![
                "Pixels (px)".to_string(),
                "Inches (in)".to_string(),
                "Centimeters (cm)".to_string(),
                "Millimeters (mm)".to_string(),
            ],
            0,
        ).with_label("Units");
        dropdown_margin_units.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut slider_margin_x = Slider::new().with_range(0.0, 100.0).with_value(0.2).with_readout(true).with_label("Margin Width (X)");
        slider_margin_x.set_rect(0.0, 0.0, 240.0, 18.0);
        let mut slider_margin_y = Slider::new().with_range(0.0, 100.0).with_value(0.2).with_readout(true).with_label("Margin Height (Y)");
        slider_margin_y.set_rect(0.0, 0.0, 240.0, 18.0);

        // Create sidebar spinbox inputs
        let mut sidebar_x = Spinbox::new(0, -2000, 2000, 1).with_label("X Position");
        sidebar_x.set_rect(0.0, 0.0, 240.0, 26.0);
        let mut sidebar_y = Spinbox::new(0, -2000, 2000, 1).with_label("Y Position");
        sidebar_y.set_rect(0.0, 0.0, 240.0, 26.0);
        let mut sidebar_w = Spinbox::new(100, -2000, 2000, 1).with_label("Width");
        sidebar_w.set_rect(0.0, 0.0, 240.0, 26.0);
        let mut sidebar_h = Spinbox::new(50, -2000, 2000, 1).with_label("Height");
        sidebar_h.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_line_cap = Dropdown::new(
            vec![
                "Arrow".to_string(),
                "Round".to_string(),
                "Flat".to_string(),
            ],
            0,
        ).with_label("Line End Shape");
        dropdown_line_cap.set_rect(0.0, 0.0, 240.0, 26.0);
        let mut sidebar_text = TextBox::new(String::new()).with_label("Text Content");
        sidebar_text.set_rect(0.0, 0.0, 240.0, 26.0);
        let mut sidebar_size = Spinbox::new(14, 4, 120, 1).with_label("Text Size");
        sidebar_size.set_rect(0.0, 0.0, 240.0, 26.0);

        // Create color sliders with readout
        let mut slider_r = Slider::new().with_range(0.0, 1.0).with_value(0.5).with_readout(true).with_label("Red Color");
        slider_r.set_rect(0.0, 0.0, 240.0, 18.0);
        let mut slider_g = Slider::new().with_range(0.0, 1.0).with_value(0.5).with_readout(true).with_label("Green Color");
        slider_g.set_rect(0.0, 0.0, 240.0, 18.0);
        let mut slider_b = Slider::new().with_range(0.0, 1.0).with_value(0.5).with_readout(true).with_label("Blue Color");
        slider_b.set_rect(0.0, 0.0, 240.0, 18.0);
        let mut slider_zoom = Slider::new().with_range(0.5, 2.0).with_value((1.0 - 0.5) / (2.0 - 0.5)).with_scroll(true).with_readout(true).with_label("Zoom");
        slider_zoom.set_rect(0.0, 0.0, 240.0, 18.0);

        // Page 1 Buttons
        let mut toggle_grid = Toggle::new().with_label("Toggle Grid");
        toggle_grid.set_rect(0.0, 0.0, 240.0, 26.0);
        toggle_grid.set_toggled(true);

        let mut grid_color_selector = ColorSelector::new([51, 89, 153])
            .with_label("Grid Color");
        grid_color_selector.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut spinbox_grid_size = Spinbox::new(15, 5, 100, 1)
            .with_decimals(1)
            .with_label("Grid Dot Size (px)");
        spinbox_grid_size.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut margin_color_selector = ColorSelector::new([51, 128, 217])
            .with_label("Margin Guide Color");
        margin_color_selector.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut spinbox_margin_thickness = Spinbox::new(1, 1, 20, 1)
            .with_label("Margin Thickness (px)");
        spinbox_margin_thickness.set_rect(0.0, 0.0, 240.0, 26.0);



        let btn_add_text = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Text Box");
        let btn_add_rect = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Rectangle");
        let btn_add_banner = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Banner");
        let btn_add_vector = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Line");

        // Page 3 Buttons
        let mut toggle_rulers = Toggle::new().with_label("Show Rulers");
        toggle_rulers.set_rect(0.0, 0.0, 240.0, 26.0);
        let mut dropdown_units = Dropdown::new(
            vec![
                "Pixels (px)".to_string(),
                "Inches (in)".to_string(),
                "Centimeters (cm)".to_string(),
                "Millimeters (mm)".to_string(),
            ],
            0,
        ).with_label("Ruler Units");
        dropdown_units.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut dropdown_grid_units = Dropdown::new(
            vec![
                "Pixels (px)".to_string(),
                "Inches (in)".to_string(),
                "Centimeters (cm)".to_string(),
                "Millimeters (mm)".to_string(),
            ],
            0,
        ).with_label("Grid Units");
        dropdown_grid_units.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut label_sel_status = Label::new("No Selection").with_color([0x83, 0x83, 0x8a]);
        label_sel_status.set_rect(0.0, 0.0, 240.0, 18.0);

        let mut label_sel_desc1 = Label::new("Click canvas elements").with_color([0x60, 0x60, 0x68]).with_font_size(10.0);
        label_sel_desc1.set_rect(0.0, 0.0, 240.0, 14.0);

        let mut label_sel_desc2 = Label::new("to edit properties.").with_color([0x60, 0x60, 0x68]).with_font_size(10.0);
        label_sel_desc2.set_rect(0.0, 0.0, 240.0, 14.0);

        let mut label_grid_snap = Toggle::new().with_label("Grid Snapping: ON (20px)");
        label_grid_snap.set_rect(0.0, 0.0, 240.0, 26.0);
        label_grid_snap.set_toggled(true);

        let mut label_total_elements = Label::new("Total Elements: 0");
        label_total_elements.set_rect(0.0, 0.0, 240.0, 18.0);

        let recent_files = Self::load_recent_files();
        let recent_files_list = ScrollRegion::new(22.0, 2.0);
        let mut recent_files_buttons = Vec::new();
        for file in &recent_files {
            let label = file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.to_string_lossy().to_string());
            let btn = Button::new_list_row(0.0, 0.0, 0.0, 0.0).with_label(&label);
            recent_files_buttons.push(btn);
        }

        let mut app = Self {
            keys: LayoutKeys::load(),
            btn_new_doc,
            btn_open,
            recent_files,
            recent_files_list,
            recent_files_buttons,
            btn_exit,
            btn_save,
            btn_save_as,
            paginator,
            elements,
            selected_idx: None,
            dragging: None,
            grid_enabled: true,
            width: 1024,
            height: 768,
            scale_factor: 1.0,
            text_prims: Vec::new(),
            font_system: cce_ui::create_font_system(),
            needs_rebuild: true,

            dropdown_presets,
            slider_page_x,
            slider_page_y,
            page_color_selector,
            page_color,
            toggle_margin,
            dropdown_margin_units,
            slider_margin_x,
            slider_margin_y,
            margin_enabled: false,
            margin_x: 20.0,
            margin_y: 20.0,
            toggle_word_processor,
            word_processor_enabled: false,
            wp_text_box,
            sidebar_x,
            sidebar_y,
            sidebar_w,
            sidebar_h,
            sidebar_text,
            sidebar_size,
            font_selector,
            dropdown_text_align_h,
            dropdown_text_align_v,
            toggle_multiline,
            dropdown_align_h,
            dropdown_align_v,
            dropdown_line_cap,
            wp_base_font_size: 14.0,

            slider_r,
            slider_g,
            slider_b,
            slider_zoom,

            toggle_grid,
            grid_color_selector,
            grid_color: [0.20, 0.35, 0.60, 0.8],
            grid_size: 1.5,
            spinbox_grid_size,
            dropdown_grid_units,
            margin_color_selector,
            margin_color: [0.20, 0.50, 0.85, 0.35],
            spinbox_margin_thickness,
            margin_thickness: 1.0,
            btn_add_text,
            btn_add_rect,
            btn_add_banner,
            btn_add_vector,
            toggle_rulers,
            dropdown_units,

            label_sel_status,
            label_sel_desc1,
            label_sel_desc2,
            label_grid_snap,
            label_total_elements,

            last_selected: None,
            page_w,
            page_h,
            pan_x: 0.0,
            pan_y: 0.0,
            sidebar_quads: Vec::new(),
            current_file_path: None,
            layer_buttons: Vec::new(),
            ui_context: cce_ui::context::UiContext::new(),
            last_saved_document: LayoutDocument {
                page_w,
                page_h,
                page_color,
                margin_enabled: false,
                margin_x: 20.0,
                margin_y: 20.0,
                word_processor_enabled: false,
                wp_text: String::new(),
                elements: Vec::new(),
                grid_color: [0.20, 0.35, 0.60, 0.8],
                grid_size: 1.5,
                margin_color: [0.20, 0.50, 0.85, 0.35],
                margin_thickness: 1.0,
                grid_enabled: true,
                grid_units: 0,
                zoom: 0.33333334,
                rulers_enabled: false,
                ruler_units: 0,
            },
        };

        app.paginator.set_selected_page(2);
        app.sync_sidebar_fields();
        app.sync_page_units();
        app.rebuild_text_items();
        app
    }

    fn settings(&self) -> WindowSettings {
        let filename = match &self.current_file_path {
            Some(path) => path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("None"),
            None => "None",
        };
        let unsaved_suffix = if self.has_unsaved_changes() {
            " (Unsaved Changes)"
        } else {
            ""
        };
        WindowSettings {
            title: format!("Clear Layout Interface - {}{}", filename, unsaved_suffix),
            app_id: "cce-layout-interface".to_string(),
            width: 1024,
            height: 768,
            fullscreen: false,
            min_size: Some((800, 600)),
        }
    }

    fn update(&mut self, msg: Self::Message, needs_rebuild: &mut bool, exit: &mut bool) {
        match msg {
            AppMessage::Exit => {
                *exit = true;
            }
            AppMessage::Save => {
                if let Some(ref path) = self.current_file_path {
                    if let Err(e) = self.save_document(path) {
                        log::error!("Failed to save document: {}", e);
                    } else {
                        log::info!("Successfully saved layout to {:?}", path);
                        self.last_saved_document = self.get_current_document();
                        self.add_recent_file(path.clone());
                    }
                } else {
                    self.perform_save_as();
                }
            }
            AppMessage::SaveAs => {
                self.perform_save_as();
            }
            AppMessage::Open => {
                self.perform_open();
            }
            AppMessage::NewDocument => {
                self.elements.clear();
                self.selected_idx = None;
                self.dragging = None;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                self.page_color = [0.96, 0.96, 0.98, 1.0];
                self.page_color_selector.color = [245, 245, 250];
                self.grid_color = [0.20, 0.35, 0.60, 0.8];
                self.grid_color_selector.color = [51, 89, 153];
                self.grid_size = 1.5;
                self.spinbox_grid_size.value = 15;
                self.margin_color = [0.20, 0.50, 0.85, 0.35];
                self.margin_color_selector.color = [51, 128, 217];
                self.margin_thickness = 1.0;
                self.spinbox_margin_thickness.value = 1;
                self.margin_enabled = false;
                self.margin_x = 20.0;
                self.margin_y = 20.0;
                self.toggle_margin.set_toggled(false);

                self.dropdown_presets.selected = 0;
                self.page_w = PRESETS[0].w;
                self.page_h = PRESETS[0].h;
                let val_x = (self.page_w - 100.0) / (1500.0 - 100.0);
                self.slider_page_x.set_value(val_x);
                let val_y = (self.page_h - 100.0) / (1500.0 - 100.0);
                self.slider_page_y.set_value(val_y);

                self.dropdown_margin_units.selected = 0;
                self.sync_page_units();

                self.grid_enabled = true;
                self.toggle_grid.set_toggled(true);
                self.label_grid_snap.set_toggled(true);
                self.dropdown_grid_units.selected = 0;
                self.slider_zoom.set_value(0.33333334);
                self.toggle_rulers.set_toggled(false);
                self.dropdown_units.selected = 0;

                self.toggle_word_processor.set_toggled(false);
                self.word_processor_enabled = false;
                self.wp_text_box.text = String::new();
                self.wp_text_box.edit_buffer = String::new();
                self.wp_text_box.editing = false;

                self.current_file_path = None;

                self.rebuild_layers_tab_widgets();
                self.sync_sidebar_fields();
                self.last_saved_document = self.get_current_document();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddText => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Text {
                    text: "New Text Block".to_string(),
                    x: 40.0,
                    y: 100.0,
                    w: 200.0,
                    h: 30.0,
                    font_size: 14.0,
                    color: [0.0, 0.0, 0.0, 1.0],
                    font_family: "monospace".to_string(),
                    align_h: TextAlignH::Left,
                    align_v: TextAlignV::Top,
                    multiline: true,
                });
                self.selected_idx = Some(next_idx);
                self.rebuild_layers_tab_widgets();
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddRectangle => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Shape {
                    shape_type: ShapeType::Rectangle,
                    x: 40.0,
                    y: 100.0,
                    w: 150.0,
                    h: 100.0,
                    color: [0.2, 0.6, 0.4, 1.0],
                });
                self.selected_idx = Some(next_idx);
                self.rebuild_layers_tab_widgets();
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddBanner => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Shape {
                    shape_type: ShapeType::Banner,
                    x: 40.0,
                    y: 100.0,
                    w: 300.0,
                    h: 50.0,
                    color: [0.8, 0.4, 0.2, 1.0],
                });
                self.selected_idx = Some(next_idx);
                self.rebuild_layers_tab_widgets();
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddVector => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Vector {
                    x1: 40.0,
                    y1: 100.0,
                    x2: 240.0,
                    y2: 100.0,
                    stroke_width: 4.0,
                    color: [0.2, 0.4, 0.8, 1.0],
                    line_cap: LineCap::Arrow,
                });
                self.selected_idx = Some(next_idx);
                self.rebuild_layers_tab_widgets();
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
        }
    }

    fn tick(&mut self, dt: f32, needs_rebuild: &mut bool) {
        if self.paginator.tick(dt, &mut self.ui_context) {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
        if self.active_page_widgets_tick(dt) {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
        let _ = self.page_color_selector.tick(dt, &mut self.ui_context);
        let r = self.page_color_selector.color[0] as f32 / 255.0;
        let g = self.page_color_selector.color[1] as f32 / 255.0;
        let b = self.page_color_selector.color[2] as f32 / 255.0;
        let new_col = [r, g, b, 1.0];
        if self.page_color != new_col {
            self.page_color = new_col;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let mut new_grid_enabled = self.grid_enabled;
        if self.toggle_grid.toggled() != self.grid_enabled {
            new_grid_enabled = self.toggle_grid.toggled();
        } else if self.label_grid_snap.toggled() != self.grid_enabled {
            new_grid_enabled = self.label_grid_snap.toggled();
        }

        if self.grid_enabled != new_grid_enabled {
            self.grid_enabled = new_grid_enabled;
            self.toggle_grid.set_toggled(new_grid_enabled);
            self.label_grid_snap.set_toggled(new_grid_enabled);
            self.sync_sidebar_fields();
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let _ = self.grid_color_selector.tick(dt, &mut self.ui_context);
        let gr = self.grid_color_selector.color[0] as f32 / 255.0;
        let gg = self.grid_color_selector.color[1] as f32 / 255.0;
        let gb = self.grid_color_selector.color[2] as f32 / 255.0;
        let new_grid_col = [gr, gg, gb, 0.8];
        if self.grid_color != new_grid_col {
            self.grid_color = new_grid_col;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let new_grid_size = if self.spinbox_grid_size.editing {
            self.spinbox_grid_size.edit_buffer.parse::<f32>().unwrap_or(1.5)
        } else {
            self.spinbox_grid_size.value as f32 / 10.0
        };
        if self.grid_size != new_grid_size {
            self.grid_size = new_grid_size;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let _ = self.margin_color_selector.tick(dt, &mut self.ui_context);
        let mr = self.margin_color_selector.color[0] as f32 / 255.0;
        let mg = self.margin_color_selector.color[1] as f32 / 255.0;
        let mb = self.margin_color_selector.color[2] as f32 / 255.0;
        let new_margin_col = [mr, mg, mb, 0.35];
        if self.margin_color != new_margin_col {
            self.margin_color = new_margin_col;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let new_margin_thickness = if self.spinbox_margin_thickness.editing {
            self.spinbox_margin_thickness.edit_buffer.parse::<f32>().unwrap_or(1.0)
        } else {
            self.spinbox_margin_thickness.value as f32
        };
        if self.margin_thickness != new_margin_thickness {
            self.margin_thickness = new_margin_thickness;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        // Preset dropdown selection change
        if self.dropdown_presets.take_change() {
            let sel = self.dropdown_presets.selected;
            if sel < PRESETS.len() - 1 {
                self.page_w = PRESETS[sel].w;
                self.page_h = PRESETS[sel].h;
                let val_x = (self.page_w - 100.0) / (1500.0 - 100.0);
                self.slider_page_x.set_value(val_x);
                let val_y = (self.page_h - 100.0) / (1500.0 - 100.0);
                self.slider_page_y.set_value(val_y);
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
        }

        // Margin units dropdown change
        if self.dropdown_margin_units.take_change() {
            self.sync_page_units();
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        // Ruler units dropdown change
        if self.dropdown_units.take_change() {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        // Grid units dropdown change
        if self.dropdown_grid_units.take_change() {
            self.sync_sidebar_fields();
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let unit_idx = self.dropdown_margin_units.selected;
        let px_per_unit = match unit_idx {
            0 => 1.0,           // Pixels
            1 => 60.0,          // Inches
            2 => 23.622047,     // Centimeters
            3 => 2.3622047,     // Millimeters
            _ => 1.0,
        };

        // Page width slider change
        let page_w = self.slider_page_x.get_scaled_value() * px_per_unit;
        if (self.page_w - page_w).abs() > 0.01 {
            self.page_w = page_w;
            self.dropdown_presets.selected = PRESETS.len() - 1; // "Custom"
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        // Page height slider change
        let page_h = self.slider_page_y.get_scaled_value() * px_per_unit;
        if (self.page_h - page_h).abs() > 0.01 {
            self.page_h = page_h;
            self.dropdown_presets.selected = PRESETS.len() - 1; // "Custom"
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let margin_enabled = self.toggle_margin.toggled();
        if self.margin_enabled != margin_enabled {
            self.margin_enabled = margin_enabled;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let margin_x = self.slider_margin_x.get_scaled_value() * px_per_unit;
        if (self.margin_x - margin_x).abs() > 0.01 {
            self.margin_x = margin_x;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let margin_y = self.slider_margin_y.get_scaled_value() * px_per_unit;
        if (self.margin_y - margin_y).abs() > 0.01 {
            self.margin_y = margin_y;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        let wp_enabled = self.toggle_word_processor.toggled();
        if self.word_processor_enabled != wp_enabled {
            self.word_processor_enabled = wp_enabled;
            if wp_enabled {
                self.wp_text_box.focus();
                self.selected_idx = None;
                self.dragging = None;
            } else {
                self.wp_text_box.unfocus();
            }
            self.sync_sidebar_fields();
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
    }

    fn display_list(&mut self, size: LogicalSize, scale: f64) -> Option<cce_ui::scene::paint::DisplayList> {
        // Id-rooted router: the word-processor box dispatches by id — keep its
        // registration fresh (idempotent) whether or not the mode is enabled.
        {
            let (id, ptr) = (self.wp_text_box.id(), self.wp_text_box.as_ptr_mut());
            self.ui_context.register_widget(id, ptr);
        }
        // Phase 6aj single paint path: view() geometry + view_vectors + text prims (single-run
        // via text_with, boxed canvas text via text_boxed). App FontSystem is bundled now (was
        // _with_system_fonts) — kept only for widgets' prepare_text; text renders via the engine
        // cache, fixing the face-ID invisibility.
        let mut __pc = cce_ui::scene::paint::PaintCtx::new();
        let quads = &mut __LayoutQuadSink { pc: &mut __pc };
        let size_changed = self.width != size.width as u32 || self.height != size.height as u32 || self.scale_factor != scale;
        if self.needs_rebuild || size_changed {
            self.width = size.width as u32;
            self.height = size.height as u32;
            self.scale_factor = scale;

            // Set paginator bounds in the sidebar region
            cce_ui::scale::set_scale_factor(scale as f32);
            self.paginator.set_rect(0.0, 0.0, 280.0, size.height as f32);

            self.rebuild_layers_tab_widgets();
            self.rebuild_text_items();
            self.needs_rebuild = false;
        }



        // 3. Render Canvas & centered paper sheet
        let canvas_w = self.width as f32 - 280.0;
        let canvas_h = self.height as f32;
        let zoom = self.render_zoom();

        // Centered Paper Position
        let page_x = 280.0 + (canvas_w - self.page_w * zoom) / 2.0 + self.pan_x;
        let page_y = (canvas_h - self.page_h * zoom) / 2.0 + self.pan_y;

        // Dark slate canvas backdrop
        let mut canvas_bg = [0.12, 0.12, 0.15, 1.0];
        if let Some(opacity) = cce_ui::color::read_opacity_if_configured() {
            canvas_bg[3] = opacity;
        }
        quads.push((280.0, 0.0, canvas_w, canvas_h, canvas_bg));

        // Axis Rulers
        if self.toggle_rulers.toggled() {
            let ruler_bg = [0.18, 0.18, 0.22, 1.0];
            let tick_color = [0.45, 0.45, 0.50, 0.8];
            let border_col = [0.30, 0.30, 0.35, 1.0];

            // Rulers backgrounds
            quads.push((page_x - 20.0, page_y - 20.0, self.page_w * zoom + 20.0, 20.0, ruler_bg));
            quads.push((page_x - 20.0, page_y - 20.0, 20.0, self.page_h * zoom + 20.0, ruler_bg));

            // Outer borders
            quads.push((page_x - 20.0, page_y, self.page_w * zoom + 20.0, 1.0, border_col));
            quads.push((page_x, page_y - 20.0, 1.0, self.page_h * zoom + 20.0, border_col));

            let unit_idx = self.dropdown_units.selected;
            let (px_per_unit, tick_step, label_step, _) = match unit_idx {
                0 => (1.0, 50.0, 100.0, 0),       // Pixels
                1 => (60.0, 0.25, 1.0, 0),        // Inches
                2 => (23.622047, 0.5, 1.0, 0),    // Centimeters
                3 => (2.3622047, 5.0, 10.0, 0),   // Millimeters
                _ => (1.0, 50.0, 100.0, 0),
            };

            // X Axis Ticks
            let max_unit_w = self.page_w / px_per_unit;
            let mut val = 0.0;
            while val <= max_unit_w + 0.001 {
                let tx = page_x + val * px_per_unit * zoom;
                let is_major = (val / label_step).round() * label_step;
                if (val - is_major).abs() < 0.001 {
                    quads.push((tx, page_y - 12.0, 1.0, 12.0, tick_color));
                } else {
                    quads.push((tx, page_y - 6.0, 1.0, 6.0, tick_color));
                }
                val += tick_step;
            }

            // Y Axis Ticks
            let max_unit_h = self.page_h / px_per_unit;
            let mut val = 0.0;
            while val <= max_unit_h + 0.001 {
                let ty = page_y + val * px_per_unit * zoom;
                let is_major = (val / label_step).round() * label_step;
                if (val - is_major).abs() < 0.001 {
                    quads.push((page_x - 12.0, ty, 12.0, 1.0, tick_color));
                } else {
                    quads.push((page_x - 6.0, ty, 6.0, 1.0, tick_color));
                }
                val += tick_step;
            }

            // Corner block
            quads.push((page_x - 20.0, page_y - 20.0, 20.0, 20.0, [0.15, 0.15, 0.18, 1.0]));
            quads.push((page_x - 20.0, page_y - 20.0, 20.0, 1.0, border_col));
            quads.push((page_x - 20.0, page_y - 20.0, 1.0, 20.0, border_col));
        }

        // Paper drop shadow
        quads.push((page_x + 3.0, page_y + 3.0, self.page_w * zoom, self.page_h * zoom, [0.05, 0.05, 0.08, 0.35]));

        // Paper sheet backing (Elegant Off-White)
        quads.push((page_x, page_y, self.page_w * zoom, self.page_h * zoom, self.page_color));

        // Thin paper border outline
        let border_col = [0.75, 0.75, 0.80, 0.5];
        quads.push((page_x, page_y, self.page_w * zoom, 1.0, border_col));
        quads.push((page_x, page_y + self.page_h * zoom - 1.0, self.page_w * zoom, 1.0, border_col));
        quads.push((page_x, page_y, 1.0, self.page_h * zoom, border_col));
        quads.push((page_x + self.page_w * zoom - 1.0, page_y, 1.0, self.page_h * zoom, border_col));

        // Draw page margin inside the paper sheet if enabled
        if self.margin_enabled {
            let mx = page_x + self.margin_x * zoom;
            let my = page_y + self.margin_y * zoom;
            let mw = (self.page_w - 2.0 * self.margin_x) * zoom;
            let mh = (self.page_h - 2.0 * self.margin_y) * zoom;
            if mw > 0.0 && mh > 0.0 {
                let margin_col = self.margin_color;
                let thick = self.margin_thickness;
                quads.push((mx, my, mw, thick, margin_col));
                quads.push((mx, my + mh - thick, mw, thick, margin_col));
                quads.push((mx, my, thick, mh, margin_col));
                quads.push((mx + mw - thick, my, thick, mh, margin_col));
            }
        }

        // 4. Snapping Grid inside the paper bounds
        if self.grid_enabled {
            let spacing = self.grid_spacing() * zoom;
            let mut cx = spacing;
            let dot_size = (self.grid_size * zoom).max(1.0);
            while cx < self.page_w * zoom {
                let mut cy = spacing;
                while cy < self.page_h * zoom {
                    quads.push((page_x + cx, page_y + cy, dot_size, dot_size, self.grid_color));
                    cy += spacing;
                }
                cx += spacing;
            }
        }

        // 5. Render Layout Elements or Word Processor
        if self.word_processor_enabled {
            let mx = page_x + self.margin_x * zoom;
            let my = page_y + self.margin_y * zoom;
            let mw = (self.page_w - 2.0 * self.margin_x) * zoom;
            let mh = (self.page_h - 2.0 * self.margin_y) * zoom;
            if mw > 0.0 && mh > 0.0 {
                self.wp_text_box.set_rect(mx, my, mw, mh);
                self.wp_text_box.font_size = self.wp_base_font_size * zoom;
                quads.extend(self.wp_text_box.extra_quads());
            }
        } else {
            for (idx, element) in self.elements.iter().enumerate() {
                match element {
                    Element::Text { text: _, x, y, w, h, font_size: _, color: _, .. } => {
                        // Translucent text block container overlay
                        quads.push((page_x + *x * zoom, page_y + *y * zoom, *w * zoom, *h * zoom, [0.20, 0.30, 0.70, 0.05]));

                        let border_color = [0.45, 0.45, 0.55, 0.22];
                        quads.push((page_x + *x * zoom, page_y + *y * zoom, *w * zoom, 1.0, border_color));
                        quads.push((page_x + *x * zoom, page_y + *y * zoom + *h * zoom - 1.0, *w * zoom, 1.0, border_color));
                        quads.push((page_x + *x * zoom, page_y + *y * zoom, 1.0, *h * zoom, border_color));
                        quads.push((page_x + *x * zoom + *w * zoom - 1.0, page_y + *y * zoom, 1.0, *h * zoom, border_color));
                    }
                    Element::Shape { shape_type, x, y, w, h, color } => {
                        match shape_type {
                            ShapeType::Rectangle => {
                                quads.push((page_x + *x * zoom, page_y + *y * zoom, *w * zoom, *h * zoom, *color));
                            }
                            ShapeType::Banner => {
                                // Banner drop shadow
                                quads.push((page_x + *x * zoom + 3.0, page_y + *y * zoom + 3.0, *w * zoom, *h * zoom, [0.05, 0.05, 0.08, 0.3]));

                                quads.push((page_x + *x * zoom, page_y + *y * zoom, *w * zoom, *h * zoom, *color));

                                let inner_col = [1.0, 1.0, 1.0, 0.2];
                                quads.push((page_x + *x * zoom + 2.0, page_y + *y * zoom + 2.0, *w * zoom - 4.0, 1.0, inner_col));
                                quads.push((page_x + *x * zoom + 2.0, page_y + *y * zoom + *h * zoom - 3.0, *w * zoom - 4.0, 1.0, inner_col));
                                quads.push((page_x + *x * zoom + 2.0, page_y + *y * zoom + 2.0, 1.0, *h * zoom - 4.0, inner_col));
                                quads.push((page_x + *x * zoom + *w * zoom - 3.0, page_y + *y * zoom + 2.0, 1.0, *h * zoom - 4.0, inner_col));
                            }
                        }
                    }
                    Element::Vector { .. } => {}
                }

                // Draw highlight border if selected
                if self.selected_idx == Some(idx) {
                    let (ex, ey, ew, eh) = match element {
                        Element::Text { x, y, w, h, .. } => (*x, *y, *w, *h),
                        Element::Shape { x, y, w, h, .. } => (*x, *y, *w, *h),
                        Element::Vector { x1, y1, x2, y2, .. } => {
                            let x = x1.min(*x2);
                            let y = y1.min(*y2);
                            let w = (x1 - x2).abs();
                            let h = (y1 - y2).abs();
                            (x, y, w, h)
                        }
                    };
                    let sel_col = [0.15, 0.45, 0.90, 1.0]; // Bright neon blue
                    quads.push((page_x + ex * zoom - 1.0, page_y + ey * zoom - 1.0, ew * zoom + 2.0, 1.0, sel_col));
                    quads.push((page_x + ex * zoom - 1.0, page_y + ey * zoom + eh * zoom, ew * zoom + 2.0, 1.0, sel_col));
                    quads.push((page_x + ex * zoom - 1.0, page_y + ey * zoom - 1.0, 1.0, eh * zoom + 2.0, sel_col));
                    quads.push((page_x + ex * zoom + ew * zoom, page_y + ey * zoom - 1.0, 1.0, eh * zoom + 2.0, sel_col));

                    // Corner handles
                    let hs = 5.0;
                    let hc = [1.0, 1.0, 1.0, 1.0];
                    let hborder = [0.15, 0.45, 0.90, 1.0];
                    let handles = [
                        (page_x + ex * zoom - 2.0, page_y + ey * zoom - 2.0),
                        (page_x + ex * zoom + ew * zoom - 3.0, page_y + ey * zoom - 2.0),
                        (page_x + ex * zoom - 2.0, page_y + ey * zoom + eh * zoom - 3.0),
                        (page_x + ex * zoom + ew * zoom - 3.0, page_y + ey * zoom + eh * zoom - 3.0),
                    ];
                    for (hx, hy) in handles {
                        quads.push((hx, hy, hs, hs, hborder));
                        quads.push((hx + 1.0, hy + 1.0, hs - 2.0, hs - 2.0, hc));
                    }
                }
            }
        }

        // Render Paginator Sidebar (includes backgrounds and active tab sliding container)
        quads.extend(self.paginator.extra_quads());
        if let Some(hq) = self.paginator.highlight_quad(&self.ui_context) {
            quads.push(hq);
        }
        quads.extend(self.sidebar_quads.iter().cloned());

        // Divider between Sidebar and Canvas
        quads.push((280.0, 0.0, 1.0, self.height as f32, [0.20, 0.20, 0.25, 1.0]));

        // Vectors (the legacy view_vectors body), after plain geometry as the wrapper ordered.
        if !self.word_processor_enabled {
            let zoom = self.render_zoom();
            let canvas_w = self.width as f32 - 280.0;
            let canvas_h = self.height as f32;
            let page_x = 280.0 + (canvas_w - self.page_w * zoom) / 2.0 + self.pan_x;
            let page_y = (canvas_h - self.page_h * zoom) / 2.0 + self.pan_y;
            for element in &self.elements {
                if let Element::Vector { x1, y1, x2, y2, stroke_width, color, line_cap } = element {
                    let vx1 = page_x + *x1 * zoom;
                    let vy1 = page_y + *y1 * zoom;
                    let vx2 = page_x + *x2 * zoom;
                    let vy2 = page_y + *y2 * zoom;
                    let vthickness = *stroke_width * zoom;
                    let cap = match line_cap {
                        LineCap::Flat => cce_ui::scene::paint::Cap::Flat,
                        LineCap::Round => cce_ui::scene::paint::Cap::Round,
                        LineCap::Arrow => cce_ui::scene::paint::Cap::Arrow,
                    };
                    __pc.vector(vx1, vy1, vx2, vy2, vthickness, *color, cap);
                }
            }
        }

        // Text prims (single-run and boxed).
        for (text, size, x, y, color, font, bounds, layout) in &self.text_prims {
            match layout {
                Some(l) => __pc.text_boxed(text.clone(), *x, *y, *size, *color, font.clone(), *bounds, cce_ui::scene::paint::TextAttrs::default(), *l),
                None => __pc.text_with(text.clone(), *x, *y, *size, *color, font.clone(), *bounds),
            }
        }

        // Widget text via the paint walk (not the legacy text_labels* getters): the
        // paginator's sidebar tabs, and the word-processor text box when active.
        cce_ui::scene::painter::append_widget_text(&self.ui_context, &self.paginator, &mut __pc);
        if self.word_processor_enabled {
            cce_ui::scene::painter::append_widget_text(&self.ui_context, &self.wp_text_box, &mut __pc);
        }

        // Open dropdown popovers, last, on top of everything — PaintCtx is a
        // RenderTarget, and render_popovers sweeps the registry for open
        // popovers (this app registers none explicitly). Without this pass an
        // open menu was invisible: it hit-tested and occluded, but never drew.
        cce_ui::layout::render_popovers(&mut __pc, &self.ui_context);

        Some(__pc.finish())
    }

    fn display_list_text(&self) -> bool {
        true
    }

    fn handle_pointer_move(&mut self, pos: LogicalPosition, needs_rebuild: &mut bool) {
        let mut changed = false;
        let px = pos.x as f32;
        let py = pos.y as f32;

        let sidebar_w = 280.0;
        if px < sidebar_w {
            if {
                // Self-routing composite: handle_event, not propagate — the router's
                // children-first descent would let the embedded strip consume this.
                let mv = cce_ui::widget::Event::PointerMove { x: px, y: py, local_x: px, local_y: py };
                self.paginator.handle_event(&mv, &mut self.ui_context)
            } { changed = true; }
            if self.active_page_widgets_cursor_moved(px, py) { changed = true; }
            if self.paginator.selected_page() == 2 && changed {
                self.apply_sidebar_changes();
            }
        } else {
            // Clear hover state on sidebar widgets if mouse moves to canvas
            if {
                // Self-routing composite: handle_event, not propagate — the router's
                // children-first descent would let the embedded strip consume this.
                let mv = cce_ui::widget::Event::PointerMove { x: px, y: py, local_x: px, local_y: py };
                self.paginator.handle_event(&mv, &mut self.ui_context)
            } { changed = true; }
            if self.active_page_widgets_cursor_moved(px, py) { changed = true; }

            // Compute centering coordinates for canvas elements
            let canvas_w = self.width as f32 - 280.0;
            let canvas_h = self.height as f32;
            let zoom = self.render_zoom();
            let page_w = self.page_w * zoom;
            let page_h = self.page_h * zoom;
            let page_x = 280.0 + (canvas_w - page_w) / 2.0 + self.pan_x;
            let page_y = (canvas_h - page_h) / 2.0 + self.pan_y;

            let cx = (px - page_x) / zoom;
            let cy = (py - page_y) / zoom;

            if self.word_processor_enabled {
                let mv = cce_ui::widget::Event::PointerMove { x: px, y: py, local_x: px, local_y: py };
                let root = self.wp_text_box.id();
                if self.ui_context.propagate_event(&mv, root) {
                    changed = true;
                }
            } else if let Some((idx, ox, oy)) = self.dragging {
                let mut new_x = cx - ox;
                let mut new_y = cy - oy;

                if self.grid_enabled {
                    let spacing = self.grid_spacing();
                    new_x = (new_x / spacing).round() * spacing;
                    new_y = (new_y / spacing).round() * spacing;
                }

                match &mut self.elements[idx] {
                    Element::Text { x, y, .. } => {
                        *x = new_x;
                        *y = new_y;
                    }
                    Element::Shape { x, y, .. } => {
                        *x = new_x;
                        *y = new_y;
                    }
                    Element::Vector { x1, y1, x2, y2, .. } => {
                        let dx = new_x - *x1;
                        let dy = new_y - *y1;
                        *x1 = new_x;
                        *y1 = new_y;
                        *x2 += dx;
                        *y2 += dy;
                    }
                }
                self.sync_sidebar_fields();
                changed = true;
            }
        }

        if changed {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
    }

    fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState, pos: LogicalPosition, needs_rebuild: &mut bool) -> Option<Self::Message> {
        let mut changed = false;
        let mut msg_out = None;
        let px = pos.x as f32;
        let py = pos.y as f32;

        let sidebar_w = 280.0;
        if px < sidebar_w {
            let pag_mouse = {
                // Self-routing composite: handle_event, not propagate (see pointer move).
                let ev = cce_ui::widget::Event::MouseButton { button, state, x: px, y: py, local_x: px, local_y: py };
                self.paginator.handle_event(&ev, &mut self.ui_context)
            };
            if pag_mouse {
                changed = true;
                if let Some((new_page, _)) = self.paginator.menu_click() {
                    self.active_page_widgets_unfocus();
                    self.apply_sidebar_changes();
                    self.paginator.set_selected_page(new_page);
                }
            } else {
                if state == ElementState::Pressed {
                    self.active_page_widgets_focus_handle(px, py);
                }

                if self.active_page_widgets_mouse_input(button, state, px, py) {
                    changed = true;
                }

                if self.paginator.selected_page() == 2 {
                    self.apply_sidebar_changes();
                }

                let active_page = self.paginator.selected_page();
                if active_page == 0 {
                    if state == ElementState::Released {
                        if self.btn_new_doc.take_click() {
                            msg_out = Some(AppMessage::NewDocument);
                        }
                        if self.btn_open.take_click() {
                            msg_out = Some(AppMessage::Open);
                        }
                        let mut clicked_file = None;
                        for (i, btn) in self.recent_files_buttons.iter_mut().enumerate() {
                            if btn.take_click() {
                                clicked_file = Some(self.recent_files[i].clone());
                            }
                        }
                        if let Some(path) = clicked_file {
                            self.selected_idx = None;
                            self.dragging = None;
                            self.pan_x = 0.0;
                            self.pan_y = 0.0;
                            if let Err(e) = self.load_document(&path) {
                                log::error!("Failed to load document: {}", e);
                            } else {
                                log::info!("Successfully loaded layout from {:?}", path);
                                self.current_file_path = Some(path.clone());
                                self.last_saved_document = self.get_current_document();
                                self.add_recent_file(path);
                            }
                        }
                        if self.btn_save.take_click() {
                            msg_out = Some(AppMessage::Save);
                        }
                        if self.btn_save_as.take_click() {
                            msg_out = Some(AppMessage::SaveAs);
                        }
                        if self.btn_exit.take_click() {
                            msg_out = Some(AppMessage::Exit);
                        }
                    }
                } else if active_page == 1 {
                    let _ = self.toggle_margin.take_click();
                    let _ = self.toggle_word_processor.take_click();

                } else if active_page == 3 {
                    if state == ElementState::Released {
                        if self.btn_add_text.take_click() {
                            msg_out = Some(AppMessage::AddText);
                        }
                        if self.btn_add_rect.take_click() {
                            msg_out = Some(AppMessage::AddRectangle);
                        }
                        if self.btn_add_banner.take_click() {
                            msg_out = Some(AppMessage::AddBanner);
                        }
                        if self.btn_add_vector.take_click() {
                            msg_out = Some(AppMessage::AddVector);
                        }
                    }
                } else if active_page == 4 {
                    let _ = self.toggle_grid.take_click();
                    let _ = self.grid_color_selector.take_click();
                    let _ = self.label_grid_snap.take_click();
                    let _ = self.toggle_rulers.take_click();
                    let _ = self.margin_color_selector.take_click();
                } else if active_page == 5 {
                    if state == ElementState::Released {
                        let mut clicked_idx = None;
                        for (idx, btn) in self.layer_buttons.iter_mut().enumerate() {
                            if btn.take_click() {
                                clicked_idx = Some(idx);
                            }
                        }
                        if let Some(idx) = clicked_idx {
                            self.selected_idx = Some(idx);
                            self.sync_sidebar_fields();
                            self.rebuild_layers_tab_widgets();
                            changed = true;
                        }
                    }
                }
            }
        } else {
            // Compute centering coordinates for canvas elements
            let canvas_w = self.width as f32 - 280.0;
            let canvas_h = self.height as f32;
            let zoom = self.render_zoom();
            let page_w = self.page_w * zoom;
            let page_h = self.page_h * zoom;
            let page_x = 280.0 + (canvas_w - page_w) / 2.0 + self.pan_x;
            let page_y = (canvas_h - page_h) / 2.0 + self.pan_y;

            let cx = (px - page_x) / zoom;
            let cy = (py - page_y) / zoom;

            if self.word_processor_enabled {
                if {
                    let ev = cce_ui::widget::Event::MouseButton { button, state, x: px, y: py, local_x: px, local_y: py };
                    let root = self.wp_text_box.id();
                    self.ui_context.propagate_event(&ev, root)
                } {
                    changed = true;
                } else if state == ElementState::Pressed {
                    self.wp_text_box.unfocus();
                    changed = true;
                }
            } else if state == ElementState::Pressed {
                let mut found = None;
                for (idx, element) in self.elements.iter().enumerate().rev() {
                    match element {
                        Element::Text { x, y, w, h, .. } => {
                            if cx >= *x && cx <= *x + *w && cy >= *y && cy <= *y + *h {
                                found = Some(idx);
                                break;
                            }
                        }
                        Element::Shape { x, y, w, h, .. } => {
                            if cx >= *x && cx <= *x + *w && cy >= *y && cy <= *y + *h {
                                found = Some(idx);
                                break;
                            }
                        }
                        Element::Vector { x1, y1, x2, y2, stroke_width, .. } => {
                            let dist = point_to_line_segment_distance(cx, cy, *x1, *y1, *x2, *y2);
                            let select_tolerance = (stroke_width * 1.5).max(10.0);
                            if dist <= select_tolerance {
                                found = Some(idx);
                                break;
                            }
                        }
                    }
                }

                if let Some(idx) = found {
                    self.selected_idx = Some(idx);
                    let (ex, ey) = match &self.elements[idx] {
                        Element::Text { x, y, .. } => (*x, *y),
                        Element::Shape { x, y, .. } => (*x, *y),
                        Element::Vector { x1, y1, .. } => (*x1, *y1),
                    };
                    self.dragging = Some((idx, cx - ex, cy - ey));
                    self.rebuild_layers_tab_widgets();
                    self.sync_sidebar_fields();
                    changed = true;
                } else {
                    self.selected_idx = None;
                    self.dragging = None;
                    self.rebuild_layers_tab_widgets();
                    self.sync_sidebar_fields();
                    changed = true;
                }
            } else if state == ElementState::Released {
                self.dragging = None;
            }
        }

        if changed {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        msg_out
    }

    fn handle_mouse_wheel(&mut self, delta: &MouseScrollDelta, pos: LogicalPosition, needs_rebuild: &mut bool) {
        let px = pos.x as f32;
        let py = pos.y as f32;
        if px < 280.0 {
            let mut wheel_handled = false;
            if self.active_page_widgets_mouse_wheel(delta, px, py) {
                wheel_handled = true;
            }
            if {
                let wev = cce_ui::widget::Event::MouseWheel { delta: *delta, x: px, y: py, local_x: px, local_y: py };
                self.paginator.handle_event(&wev, &mut self.ui_context)
            } {
                wheel_handled = true;
            }
            if wheel_handled {
                if self.paginator.selected_page() == 2 {
                    self.apply_sidebar_changes();
                }
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
        } else {
            let (dx, dy) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (*x * 24.0, *y * 24.0),
                MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
            };
            self.pan_x += dx;
            self.pan_y += dy;
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
    }

    fn handle_key_input(&mut self, event: &KeyEvent, needs_rebuild: &mut bool) -> Option<Self::Message> {
        if event.state == ElementState::Pressed {
            // Ctrl+letter can arrive as the raw control character; keep the
            // \u{13} form matching whenever the chord is the ctrl+s default.
            let ctrl_char_save = self.keys.save_document == "ctrl+s"
                && event.ctrl
                && matches!(event.logical_key, Key::Character(ref s) if s == "\u{13}");
            if cce_ui::widget::match_key_shortcut(event, &self.keys.save_document) || ctrl_char_save {
                *needs_rebuild = true;
                self.needs_rebuild = true;
                return Some(AppMessage::Save);
            }
        }

        let mut handled = false;

        if self.word_processor_enabled {
            let kev = cce_ui::widget::Event::KeyInput(event.clone());
            let root = self.wp_text_box.id();
            if self.ui_context.propagate_event(&kev, root) {
                handled = true;
            }
        }

        if self.active_page_widgets_keyboard_input(event) {
            handled = true;
        }

        if {
            let kev = cce_ui::widget::Event::KeyInput(event.clone());
            self.paginator.handle_event(&kev, &mut self.ui_context)
        } {
            handled = true;
            let active_page = self.paginator.selected_page();
            if active_page == 2 {
                self.apply_sidebar_changes();
            }
        }

        if !handled && !self.word_processor_enabled && event.state == ElementState::Pressed {
            if cce_ui::widget::match_key_shortcut(event, &self.keys.delete_element) {
                if let Some(idx) = self.selected_idx {
                    if idx < self.elements.len() {
                        self.elements.remove(idx);
                        self.selected_idx = None;
                        self.dragging = None;
                        self.rebuild_layers_tab_widgets();
                        self.sync_sidebar_fields();
                        handled = true;
                    }
                }
            }
        }

        if handled {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }

        None
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();

    cce_ui::engine::run::<LayoutApp>();
}
