use wayland_client::QueueHandle;
use glyphon::{FontSystem, Buffer, Metrics, Attrs};
use clear_ui::engine::{Application, EngineState, LogicalPosition, LogicalSize, WindowSettings};
use clear_ui::widget::{
    MouseButton, ElementState, MouseScrollDelta, KeyEvent, TextItem, Widget,
    MenuBar, TextBox, Slider, TextLabel, Paginator, Button, Dropdown, Toggle, ColorSelector,
    Label, SectionHeader, Spinbox
};

#[derive(Debug, Clone)]
enum AppMessage {
    Exit,
    NewDocument,
    AddText,
    AddRectangle,
    AddBanner,
    ToggleGrid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ShapeType {
    Rectangle,
    Banner,
}

#[derive(Debug, Clone)]
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
    },
    Shape {
        shape_type: ShapeType,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
}

struct PagePreset {
    name: &'static str,
    w: f32,
    h: f32,
}

const PRESETS: &[PagePreset] = &[
    PagePreset { name: "Letter (8.5\" x 11\")", w: 510.0, h: 660.0 },
    PagePreset { name: "Tabloid (11\" x 17\")", w: 660.0, h: 1020.0 },
    PagePreset { name: "A4 (210 x 297 mm)", w: 496.0, h: 701.0 },
    PagePreset { name: "A5 (148 x 210 mm)", w: 350.0, h: 496.0 },
    PagePreset { name: "Poster (8\" x 8\")", w: 480.0, h: 480.0 },
    PagePreset { name: "Custom", w: 510.0, h: 660.0 },
];

struct LayoutApp {
    menu_bar: MenuBar,
    paginator: Paginator,
    elements: Vec<Element>,
    selected_idx: Option<usize>,
    dragging: Option<(usize, f32, f32)>, // Index, offset_x, offset_y
    grid_enabled: bool,
    width: u32,
    height: u32,
    scale_factor: f64,
    text_items: Vec<TextItem>,
    font_system: FontSystem,
    needs_rebuild: bool,

    // Page 0: Layout properties controls
    dropdown_presets: Dropdown,
    slider_page_x: Slider,
    slider_page_y: Slider,
    page_color_selector: ColorSelector,
    page_color: [f32; 4],
    toggle_margin: Toggle,
    dropdown_margin_units: Dropdown,
    slider_margin_x: Slider,
    slider_margin_y: Slider,
    margin_enabled: bool,
    margin_x: f32,
    margin_y: f32,
    toggle_word_processor: Toggle,
    word_processor_enabled: bool,
    wp_text_box: TextBox,
    sidebar_x: TextBox,
    sidebar_y: TextBox,
    sidebar_w: TextBox,
    sidebar_h: TextBox,
    sidebar_text: TextBox,
    sidebar_size: Spinbox,
    dropdown_font_family: Dropdown,
    wp_base_font_size: f32,
    
    slider_r: Slider,
    slider_g: Slider,
    slider_b: Slider,
    slider_zoom: Slider,
    
    // Page 1: Canvas settings controls
    btn_toggle_grid: Button,
    btn_clear_canvas: Button,
    btn_add_text: Button,
    btn_add_rect: Button,
    btn_add_banner: Button,

    // Page 3: View settings controls
    toggle_rulers: Toggle,
    dropdown_units: Dropdown,

    section_rulers: SectionHeader,
    label_width: Label,
    label_height: Label,
    label_sel_status: Label,
    label_sel_desc1: Label,
    label_sel_desc2: Label,
    label_grid_snap: Label,
    label_total_elements: Label,

    last_selected: Option<usize>,
    page_w: f32,
    page_h: f32,
    pan_x: f32,
    pan_y: f32,
    widgets_registered: bool,
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

impl LayoutApp {
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
        // Sync labels with current state
        self.label_width.set_text(&format!("Width: {} px", self.page_w));
        self.label_height.set_text(&format!("Height: {} px", self.page_h));

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

        self.label_grid_snap.set_text(&format!("Grid Snapping: {}", if self.grid_enabled { "ON (20px)" } else { "OFF" }));
        self.label_total_elements.set_text(&format!("Total Elements: {}", self.elements.len()));

        self.text_items.clear();
        
        let mut labels = Vec::new();

        let canvas_w = self.width as f32 - 280.0;
        let canvas_h = self.height as f32 - 26.0;
        let zoom = self.render_zoom();
        let page_w = self.page_w * zoom;
        let page_h = self.page_h * zoom;
        let page_x = 280.0 + (canvas_w - page_w) / 2.0 + self.pan_x;
        let page_y = 26.0 + (canvas_h - page_h) / 2.0 + self.pan_y;
        
        // 1. MenuBar text labels
        labels.extend(self.menu_bar.text_labels());
        
        // 2. Paginator sidebar tabs
        labels.extend(self.paginator.text_labels());
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/layout_labels_debug.txt") {
            use std::io::Write;
            let _ = writeln!(file, "rebuild_text_items: labels={:?}", labels.iter().map(|l| &l.text).collect::<Vec<_>>());
        }
        
        // 3. Collect and add active page popover labels
        let mut collector = clear_ui::layout::PopoverCollector::new();
        self.paginator.render_popover(&mut collector);
        for (text, size, x, y, color, _font) in collector.texts {
            labels.push(TextLabel {
                text,
                x,
                y,
                font_size: size,
                color: [
                    (color[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (color[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (color[2] * 255.0).clamp(0.0, 255.0) as u8,
                ],
            });
        }

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

        // Help Instructions at the bottom of the sidebar
        labels.push(TextLabel {
            text: "INSTRUCTIONS:".to_string(),
            x: 20.0,
            y: 550.0,
            font_size: 11.0,
            color: [0xaa, 0xaa, 0xbb],
        });
        labels.push(TextLabel {
            text: "- Drag elements on canvas to move".to_string(),
            x: 20.0,
            y: 570.0,
            font_size: 10.0,
            color: [0x83, 0x83, 0x8a],
        });
        labels.push(TextLabel {
            text: "- Edit fields or drag RGB Sliders".to_string(),
            x: 20.0,
            y: 588.0,
            font_size: 10.0,
            color: [0x83, 0x83, 0x8a],
        });
        labels.push(TextLabel {
            text: "- Presets resize sheet centering".to_string(),
            x: 20.0,
            y: 606.0,
            font_size: 10.0,
            color: [0x83, 0x83, 0x8a],
        });

        // 5. Canvas Element Labels (drawn relative to the paper sheet)
        if self.word_processor_enabled {
            let font_family = self.wp_text_box.font_family.clone();
            for label in self.wp_text_box.text_labels() {
                let metrics = Metrics::new(label.font_size, label.font_size * 1.4);
                let mut buf = Buffer::new(&mut self.font_system, metrics);
                let attrs = Attrs::new().family(glyphon::Family::Name(&font_family));
                buf.set_text(&mut self.font_system, &label.text, attrs, glyphon::Shaping::Advanced);
                buf.shape_until_scroll(&mut self.font_system, true);
                self.text_items.push(TextItem {
                    buffer: buf,
                    x: label.x,
                    y: label.y,
                    color: glyphon::Color::rgb(label.color[0], label.color[1], label.color[2]),
                });
            }
        } else {
            for (idx, element) in self.elements.iter().enumerate() {
                match element {
                    Element::Text { text, x, y, w: _, h: _, font_size, color, font_family } => {
                        let label_x = page_x + *x * zoom + 8.0 * zoom;
                        let label_y = page_y + *y * zoom + 6.0 * zoom;
                        let label_font_size = *font_size * zoom;
                        let label_color = [
                            (color[0] * 255.0).clamp(0.0, 255.0) as u8,
                            (color[1] * 255.0).clamp(0.0, 255.0) as u8,
                            (color[2] * 255.0).clamp(0.0, 255.0) as u8,
                        ];

                        let metrics = Metrics::new(label_font_size, label_font_size * 1.4);
                        let mut buf = Buffer::new(&mut self.font_system, metrics);
                        let attrs = Attrs::new().family(glyphon::Family::Name(font_family));
                        buf.set_text(&mut self.font_system, text, attrs, glyphon::Shaping::Advanced);
                        buf.shape_until_scroll(&mut self.font_system, true);
                        self.text_items.push(TextItem {
                            buffer: buf,
                            x: label_x,
                            y: label_y,
                            color: glyphon::Color::rgb(label_color[0], label_color[1], label_color[2]),
                        });
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
                }
            }
        }
        
        for label in labels {
            let metrics = Metrics::new(label.font_size, label.font_size * 1.4);
            let mut buf = Buffer::new(&mut self.font_system, metrics);
            buf.set_text(&mut self.font_system, &label.text, Attrs::new(), glyphon::Shaping::Advanced);
            buf.shape_until_scroll(&mut self.font_system, true);
            self.text_items.push(TextItem {
                buffer: buf,
                x: label.x,
                y: label.y,
                color: glyphon::Color::rgb(label.color[0], label.color[1], label.color[2]),
            });
        }
    }

    fn rebuild_element_tab_widgets(&mut self) {
        self.paginator.clear_page_widgets(1);
        
        let self_ptr = self as *mut Self;
        unsafe {
            if self.word_processor_enabled {
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).dropdown_font_family as *mut (dyn Widget + 'static));
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_size as *mut (dyn Widget + 'static));
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_r as *mut (dyn Widget + 'static));
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_g as *mut (dyn Widget + 'static));
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_b as *mut (dyn Widget + 'static));
            } else if let Some(idx) = self.selected_idx {
                match &self.elements[idx] {
                    Element::Text { .. } => {
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_x as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_y as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_w as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_h as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_text as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).dropdown_font_family as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_size as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_r as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_g as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_b as *mut (dyn Widget + 'static));
                    }
                    Element::Shape { .. } => {
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_x as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_y as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_w as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).sidebar_h as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_r as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_g as *mut (dyn Widget + 'static));
                        (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).slider_b as *mut (dyn Widget + 'static));
                    }
                }
            } else {
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).label_sel_status as *mut (dyn Widget + 'static));
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).label_sel_desc1 as *mut (dyn Widget + 'static));
                (*self_ptr).paginator.add_widget_to_page(1, &mut (*self_ptr).label_sel_desc2 as *mut (dyn Widget + 'static));
            }
        }
        let (px, py, pw, ph) = self.paginator.rect();
        self.paginator.set_rect(px, py, pw, ph);
    }

    fn sync_sidebar_fields(&mut self) {
        if self.word_processor_enabled {
            if !self.sidebar_size.editing {
                self.sidebar_size.value = self.wp_base_font_size.round() as i32;
            }
            let dropdown_idx = match self.wp_text_box.font_family.as_str() {
                "sans-serif" => 1,
                "serif" => 2,
                _ => 0, // monospace
            };
            self.dropdown_font_family.selected = dropdown_idx;

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
                Element::Text { text, x, y, w, h, font_size, color, font_family } => {
                    if !self.sidebar_text.editing { self.sidebar_text.text = text.clone(); }
                    if !self.sidebar_x.editing { self.sidebar_x.text = format!("{:.0}", x); }
                    if !self.sidebar_y.editing { self.sidebar_y.text = format!("{:.0}", y); }
                    if !self.sidebar_w.editing { self.sidebar_w.text = format!("{:.0}", w); }
                    if !self.sidebar_h.editing { self.sidebar_h.text = format!("{:.0}", h); }
                    if !self.sidebar_size.editing { self.sidebar_size.value = font_size.round() as i32; }
                    
                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);

                    let dropdown_idx = match font_family.as_str() {
                        "sans-serif" => 1,
                        "serif" => 2,
                        _ => 0, // monospace
                    };
                    self.dropdown_font_family.selected = dropdown_idx;
                }
                Element::Shape { shape_type: _, x, y, w, h, color } => {
                    if !self.sidebar_text.editing { self.sidebar_text.text = "Shape Mode".to_string(); }
                    if !self.sidebar_x.editing { self.sidebar_x.text = format!("{:.0}", x); }
                    if !self.sidebar_y.editing { self.sidebar_y.text = format!("{:.0}", y); }
                    if !self.sidebar_w.editing { self.sidebar_w.text = format!("{:.0}", w); }
                    if !self.sidebar_h.editing { self.sidebar_h.text = format!("{:.0}", h); }
                    
                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);
                }
            }
        }
    }

    fn apply_sidebar_changes(&mut self) {
        if self.word_processor_enabled {
            let family_str = match self.dropdown_font_family.selected {
                1 => "sans-serif",
                2 => "serif",
                _ => "monospace",
            }.to_string();
            self.wp_text_box.font_family = family_str;

            let size_val = if self.sidebar_size.editing {
                self.sidebar_size.edit_buffer.parse::<f32>().unwrap_or(14.0)
            } else {
                self.sidebar_size.value as f32
            };
            self.wp_base_font_size = size_val;

            let r = (self.slider_r.value() * 255.0).clamp(0.0, 255.0) as u8;
            let g = (self.slider_g.value() * 255.0).clamp(0.0, 255.0) as u8;
            let b = (self.slider_b.value() * 255.0).clamp(0.0, 255.0) as u8;
            self.wp_text_box.text_color = Some([r, g, b]);
        } else if let Some(idx) = self.selected_idx {
            let text_val = if self.sidebar_text.editing { self.sidebar_text.edit_buffer.clone() } else { self.sidebar_text.text.clone() };
            let x_val = if self.sidebar_x.editing { &self.sidebar_x.edit_buffer } else { &self.sidebar_x.text }.parse::<f32>().unwrap_or(0.0);
            let y_val = if self.sidebar_y.editing { &self.sidebar_y.edit_buffer } else { &self.sidebar_y.text }.parse::<f32>().unwrap_or(0.0);
            let w_val = if self.sidebar_w.editing { &self.sidebar_w.edit_buffer } else { &self.sidebar_w.text }.parse::<f32>().unwrap_or(100.0);
            let h_val = if self.sidebar_h.editing { &self.sidebar_h.edit_buffer } else { &self.sidebar_h.text }.parse::<f32>().unwrap_or(50.0);
            let size_val = if self.sidebar_size.editing {
                self.sidebar_size.edit_buffer.parse::<f32>().unwrap_or(16.0)
            } else {
                self.sidebar_size.value as f32
            };
            
            let r_val = self.slider_r.value();
            let g_val = self.slider_g.value();
            let b_val = self.slider_b.value();

            let family_str = match self.dropdown_font_family.selected {
                1 => "sans-serif",
                2 => "serif",
                _ => "monospace",
            }.to_string();
            
            match &mut self.elements[idx] {
                Element::Text { text, x, y, w, h, font_size, color, font_family } => {
                    *text = text_val;
                    *x = x_val;
                    *y = y_val;
                    *w = w_val;
                    *h = h_val;
                    *font_size = size_val;
                    *color = [r_val, g_val, b_val, 1.0];
                    *font_family = family_str;
                }
                Element::Shape { shape_type: _, x, y, w, h, color } => {
                    *x = x_val;
                    *y = y_val;
                    *w = w_val;
                    *h = h_val;
                    *color = [r_val, g_val, b_val, 1.0];
                }
            }
        }
    }
}

impl Application for LayoutApp {
    type Message = AppMessage;

    fn new(_qh: &QueueHandle<EngineState<Self>>, _sender: calloop::channel::Sender<Self::Message>) -> Self {
        // Build the main MenuBar
        let menu_bar = MenuBar::new(0.0, 0.0, 1024.0, 26.0)
            .with_title("clear-layout")
            .with_item("File", &["New Document", "Exit"])
            .with_item("Insert", &["Add Text Box", "Add Rectangle", "Add Banner"])
            .with_item("View", &["Toggle Grid"]);

        // Build the sidebar Paginator
        let paginator = Paginator::new(56.0, vec!["Page".to_string(), "Element".to_string(), "Canvas".to_string(), "View".to_string()])
            .with_tabs_at_top(false)
            .with_tabs_rotated(true);

        // Default paper sheet sizing (Letter)
        let page_w = 510.0;
        let page_h = 660.0;

        // Initialize elements relative to the paper sheet top-left (0, 0)
        let elements = vec![
            Element::Text {
                text: "Welcome to Clear Layout!".to_string(),
                x: 40.0,
                y: 50.0,
                w: 400.0,
                h: 40.0,
                font_size: 20.0,
                color: [0.3, 0.7, 1.0, 1.0],
                font_family: "monospace".to_string(),
            },
            Element::Shape {
                shape_type: ShapeType::Rectangle,
                x: 40.0,
                y: 100.0,
                w: 430.0,
                h: 4.0,
                color: [0.2, 0.5, 0.9, 1.0],
            },
            Element::Text {
                text: "This is a word processor and layout editor layout.".to_string(),
                x: 40.0,
                y: 130.0,
                w: 430.0,
                h: 30.0,
                font_size: 14.0,
                color: [0.8, 0.8, 0.85, 1.0],
                font_family: "monospace".to_string(),
            },
            Element::Shape {
                shape_type: ShapeType::Rectangle,
                x: 40.0,
                y: 190.0,
                w: 120.0,
                h: 80.0,
                color: [0.15, 0.25, 0.4, 1.0],
            },
            Element::Text {
                text: "Card Label".to_string(),
                x: 50.0,
                y: 205.0,
                w: 100.0,
                h: 20.0,
                font_size: 11.0,
                color: [0.9, 0.9, 0.9, 1.0],
                font_family: "monospace".to_string(),
            },
        ];

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

        let mut dropdown_font_family = Dropdown::new(
            vec![
                "Monospace".to_string(),
                "Sans-Serif".to_string(),
                "Serif".to_string(),
            ],
            0,
        ).with_label("Font Family");
        dropdown_font_family.set_rect(0.0, 0.0, 240.0, 26.0);

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

        // Create sidebar textbox inputs
        let mut sidebar_x = TextBox::new(String::new()).with_label("X Position");
        sidebar_x.set_rect(0.0, 0.0, 110.0, 26.0);
        let mut sidebar_y = TextBox::new(String::new()).with_label("Y Position");
        sidebar_y.set_rect(0.0, 0.0, 110.0, 26.0);
        let mut sidebar_w = TextBox::new(String::new()).with_label("Width");
        sidebar_w.set_rect(0.0, 0.0, 110.0, 26.0);
        let mut sidebar_h = TextBox::new(String::new()).with_label("Height");
        sidebar_h.set_rect(0.0, 0.0, 110.0, 26.0);
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
        let btn_toggle_grid = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Toggle Grid");
        let btn_clear_canvas = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Clear Canvas");
        let btn_add_text = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Add Text Box");
        let btn_add_rect = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Add Rectangle");
        let btn_add_banner = Button::new(0.0, 0.0, 240.0, 26.0).with_label("Add Banner");

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

        // Setup the new Label and SectionHeader widgets
        let mut section_rulers = SectionHeader::new("RULERS");
        section_rulers.set_rect(0.0, 0.0, 240.0, 26.0);

        let mut label_width = Label::new("Width: 510 px");
        label_width.set_rect(0.0, 0.0, 240.0, 18.0);
        let mut label_height = Label::new("Height: 660 px");
        label_height.set_rect(0.0, 0.0, 240.0, 18.0);

        let mut label_sel_status = Label::new("No Selection").with_color([0x83, 0x83, 0x8a]);
        label_sel_status.set_rect(0.0, 0.0, 240.0, 18.0);

        let mut label_sel_desc1 = Label::new("Click canvas elements").with_color([0x60, 0x60, 0x68]).with_font_size(10.0);
        label_sel_desc1.set_rect(0.0, 0.0, 240.0, 14.0);

        let mut label_sel_desc2 = Label::new("to edit properties.").with_color([0x60, 0x60, 0x68]).with_font_size(10.0);
        label_sel_desc2.set_rect(0.0, 0.0, 240.0, 14.0);

        let mut label_grid_snap = Label::new("Grid Snapping: ON (20px)");
        label_grid_snap.set_rect(0.0, 0.0, 240.0, 18.0);

        let mut label_total_elements = Label::new("Total Elements: 5");
        label_total_elements.set_rect(0.0, 0.0, 240.0, 18.0);

        let mut app = Self {
            menu_bar,
            paginator,
            elements,
            selected_idx: None,
            dragging: None,
            grid_enabled: true,
            width: 1024,
            height: 768,
            scale_factor: 1.0,
            text_items: Vec::new(),
            font_system: FontSystem::new(),
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
            dropdown_font_family,
            wp_base_font_size: 14.0,
            
            slider_r,
            slider_g,
            slider_b,
            slider_zoom,
 
            btn_toggle_grid,
            btn_clear_canvas,
            btn_add_text,
            btn_add_rect,
            btn_add_banner,
            toggle_rulers,
            dropdown_units,

            section_rulers,
            label_width,
            label_height,
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
            widgets_registered: false,
        };

        app.rebuild_element_tab_widgets();
        app.sync_sidebar_fields();
        app.sync_page_units();
        app.rebuild_text_items();
        app
    }

    fn settings(&self) -> WindowSettings {
        WindowSettings {
            title: "Clear Layout Interface".to_string(),
            app_id: "clear-layout-interface".to_string(),
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
            AppMessage::NewDocument => {
                self.elements.clear();
                self.selected_idx = None;
                self.dragging = None;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                self.page_color = [0.96, 0.96, 0.98, 1.0];
                self.page_color_selector.color = [245, 245, 250];
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
                
                self.toggle_word_processor.set_toggled(false);
                self.word_processor_enabled = false;
                self.wp_text_box.text = String::new();
                self.wp_text_box.edit_buffer = String::new();
                self.wp_text_box.editing = false;

                self.rebuild_element_tab_widgets();
                self.sync_sidebar_fields();
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
                });
                self.selected_idx = Some(next_idx);
                self.rebuild_element_tab_widgets();
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
                self.rebuild_element_tab_widgets();
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
                self.rebuild_element_tab_widgets();
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::ToggleGrid => {
                self.grid_enabled = !self.grid_enabled;
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
        }
    }

    fn tick(&mut self, dt: f32, needs_rebuild: &mut bool) {
        if self.paginator.tick(dt) {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
        let _ = self.page_color_selector.tick(dt);
        let r = self.page_color_selector.color[0] as f32 / 255.0;
        let g = self.page_color_selector.color[1] as f32 / 255.0;
        let b = self.page_color_selector.color[2] as f32 / 255.0;
        let new_col = [r, g, b, 1.0];
        if self.page_color != new_col {
            self.page_color = new_col;
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
            self.rebuild_element_tab_widgets();
            self.sync_sidebar_fields();
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
    }

    fn view(&mut self, quads: &mut Vec<(f32, f32, f32, f32, [f32; 4])>, size: LogicalSize, scale: f64) {
        let size_changed = self.width != size.width as u32 || self.height != size.height as u32 || self.scale_factor != scale;
        if self.needs_rebuild || size_changed {
            if !self.widgets_registered {
                let self_ptr = self as *mut Self;
                unsafe {
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).dropdown_margin_units as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).dropdown_presets as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).slider_page_x as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).slider_page_y as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).page_color_selector as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).toggle_margin as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).toggle_word_processor as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).slider_margin_x as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).slider_margin_y as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).label_width as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(0, &mut (*self_ptr).label_height as *mut (dyn Widget + 'static));

                    // Page 1 (Element) widgets are registered dynamically via rebuild_element_tab_widgets()

                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).btn_toggle_grid as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).btn_clear_canvas as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).btn_add_text as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).btn_add_rect as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).btn_add_banner as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).label_grid_snap as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(2, &mut (*self_ptr).label_total_elements as *mut (dyn Widget + 'static));

                    (*self_ptr).paginator.add_widget_to_page(3, &mut (*self_ptr).slider_zoom as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(3, &mut (*self_ptr).section_rulers as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(3, &mut (*self_ptr).toggle_rulers as *mut (dyn Widget + 'static));
                    (*self_ptr).paginator.add_widget_to_page(3, &mut (*self_ptr).dropdown_units as *mut (dyn Widget + 'static));
                }
                self.widgets_registered = true;
            }

            self.width = size.width as u32;
            self.height = size.height as u32;
            self.scale_factor = scale;
            
            // Layout MenuBar at the top
            self.menu_bar.set_rect(0.0, 0.0, size.width, 26.0);
            
            // Set paginator bounds in the sidebar region
            self.paginator.set_rect(0.0, 26.0, 280.0, size.height - 26.0);
            
            self.rebuild_text_items();
            self.needs_rebuild = false;
        }
 
        // 1. Render Paginator Sidebar (includes backgrounds and active tab sliding container)
        quads.extend(self.paginator.extra_quads());
        if let Some(hq) = self.paginator.highlight_quad() {
            quads.push(hq);
        }
 
        // Divider between Sidebar and Canvas
        quads.push((280.0, 26.0, 1.0, self.height as f32 - 26.0, [0.20, 0.20, 0.25, 1.0]));

        // 3. Render Canvas & centered paper sheet
        let canvas_w = self.width as f32 - 280.0;
        let canvas_h = self.height as f32 - 26.0;
        let zoom = self.render_zoom();
        
        // Centered Paper Position
        let page_x = 280.0 + (canvas_w - self.page_w * zoom) / 2.0 + self.pan_x;
        let page_y = 26.0 + (canvas_h - self.page_h * zoom) / 2.0 + self.pan_y;

        // Dark slate canvas backdrop
        quads.push((280.0, 26.0, canvas_w, canvas_h, [0.12, 0.12, 0.15, 1.0]));

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
                let margin_col = [0.20, 0.50, 0.85, 0.35]; // Translucent blue margin line
                quads.push((mx, my, mw, 1.0, margin_col));
                quads.push((mx, my + mh - 1.0, mw, 1.0, margin_col));
                quads.push((mx, my, 1.0, mh, margin_col));
                quads.push((mx + mw - 1.0, my, 1.0, mh, margin_col));
            }
        }

        // 4. Snapping Grid inside the paper bounds
        if self.grid_enabled {
            let spacing = 20.0 * zoom;
            let dot_color = [0.20, 0.35, 0.60, 0.22];
            let mut cx = spacing;
            while cx < self.page_w * zoom {
                let mut cy = spacing;
                while cy < self.page_h * zoom {
                    quads.push((page_x + cx, page_y + cy, 1.5, 1.5, dot_color));
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
                }

                // Draw highlight border if selected
                if self.selected_idx == Some(idx) {
                    let (ex, ey, ew, eh) = match element {
                        Element::Text { x, y, w, h, .. } => (*x, *y, *w, *h),
                        Element::Shape { x, y, w, h, .. } => (*x, *y, *w, *h),
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

        // Render Paginator active page popovers (e.g. presets and units dropdowns)
        // This is done near the end of view() to ensure popovers overlap canvas elements
        let mut collector = clear_ui::layout::PopoverCollector::new();
        self.paginator.render_popover(&mut collector);
        for (color, x, y, w, h) in collector.rects {
            quads.push((x, y, w, h, color));
        }

        // 6. MenuBar Background
        let mb_color = self.menu_bar.color();
        let (mb_x, mb_y, mb_w, mb_h) = self.menu_bar.rect();
        quads.push((mb_x, mb_y, mb_w, mb_h, mb_color));

        // Header Highlights for Open Menus
        for menu in &self.menu_bar.menus {
            if let Some(hq) = menu.highlight_quad() {
                quads.push(hq);
            }
        }
        quads.extend(self.menu_bar.extra_quads());
    }

    fn text_items(&self) -> &[TextItem] {
        &self.text_items
    }

    fn handle_pointer_move(&mut self, pos: LogicalPosition, needs_rebuild: &mut bool) {
        let mut changed = false;
        let px = pos.x as f32;
        let py = pos.y as f32;

        if self.menu_bar.is_menu_open() || self.menu_bar.hit_test(px, py) {
            if self.menu_bar.on_cursor_moved(px, py) {
                changed = true;
            }
        } else {
            let sidebar_w = 280.0;
            if px < sidebar_w {
                if self.paginator.on_cursor_moved(px, py) { changed = true; }
                if self.paginator.selected_page() == 1 && changed {
                    self.apply_sidebar_changes();
                }
            } else {
                // Compute centering coordinates for canvas elements
                let canvas_w = self.width as f32 - 280.0;
                let canvas_h = self.height as f32 - 26.0;
                let zoom = self.render_zoom();
                let page_w = self.page_w * zoom;
                let page_h = self.page_h * zoom;
                let page_x = 280.0 + (canvas_w - page_w) / 2.0 + self.pan_x;
                let page_y = 26.0 + (canvas_h - page_h) / 2.0 + self.pan_y;
                
                let cx = (px - page_x) / zoom;
                let cy = (py - page_y) / zoom;

                if self.word_processor_enabled {
                    if self.wp_text_box.on_cursor_moved(px, py) {
                        changed = true;
                    }
                } else if let Some((idx, ox, oy)) = self.dragging {
                    let mut new_x = cx - ox;
                    let mut new_y = cy - oy;
                    
                    if self.grid_enabled {
                        new_x = (new_x / 20.0).round() * 20.0;
                        new_y = (new_y / 20.0).round() * 20.0;
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
                    }
                    self.sync_sidebar_fields();
                    changed = true;
                }
            }
            if self.menu_bar.on_cursor_moved(px, py) {
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

        if self.menu_bar.is_menu_open() || self.menu_bar.hit_test(px, py) {
            if self.menu_bar.mouse_input(button, state, px, py) {
                changed = true;
                if let Some((menu_idx, item_idx)) = self.menu_bar.menu_click() {
                    match (menu_idx, item_idx) {
                        (0, 0) => msg_out = Some(AppMessage::NewDocument),
                        (0, 1) => msg_out = Some(AppMessage::Exit),
                        (1, 0) => msg_out = Some(AppMessage::AddText),
                        (1, 1) => msg_out = Some(AppMessage::AddRectangle),
                        (1, 2) => msg_out = Some(AppMessage::AddBanner),
                        (2, 0) => msg_out = Some(AppMessage::ToggleGrid),
                        _ => {}
                    }
                }
            }
            if state == ElementState::Pressed && !self.menu_bar.hit_test(px, py) {
                self.menu_bar.unfocus();
                changed = true;
            }
        } else {
            let sidebar_w = 280.0;
            if px < sidebar_w {
                if self.paginator.mouse_input(button, state, px, py) {
                    changed = true;
                    if self.paginator.take_click() {
                        self.dropdown_presets.unfocus();
                        self.slider_page_x.unfocus();
                        self.slider_page_y.unfocus();
                        self.dropdown_units.unfocus();
                        self.page_color_selector.unfocus();
                        self.toggle_margin.unfocus();
                        self.toggle_word_processor.unfocus();
                        self.dropdown_margin_units.unfocus();
                        self.slider_margin_x.unfocus();
                        self.slider_margin_y.unfocus();
                        self.sidebar_x.unfocus();
                        self.sidebar_y.unfocus();
                        self.sidebar_w.unfocus();
                        self.sidebar_h.unfocus();
                        self.sidebar_text.unfocus();
                        self.sidebar_size.unfocus();
                    } else {
                        let active_page = self.paginator.selected_page();
                        if active_page == 0 {
                            let _ = self.toggle_margin.take_click();
                            let _ = self.toggle_word_processor.take_click();
                        } else if active_page == 1 {
                            self.apply_sidebar_changes();
                        } else if active_page == 2 {
                            if state == ElementState::Released {
                                if self.btn_toggle_grid.take_click() {
                                    msg_out = Some(AppMessage::ToggleGrid);
                                }
                                if self.btn_clear_canvas.take_click() {
                                    msg_out = Some(AppMessage::NewDocument);
                                }
                                if self.btn_add_text.take_click() {
                                    msg_out = Some(AppMessage::AddText);
                                }
                                if self.btn_add_rect.take_click() {
                                    msg_out = Some(AppMessage::AddRectangle);
                                }
                                if self.btn_add_banner.take_click() {
                                    msg_out = Some(AppMessage::AddBanner);
                                }
                            }
                        } else if active_page == 3 {
                            let _ = self.toggle_rulers.take_click();
                        }
                    }
                }
            } else {
                // Compute centering coordinates for canvas elements
                let canvas_w = self.width as f32 - 280.0;
                let canvas_h = self.height as f32 - 26.0;
                let zoom = self.render_zoom();
                let page_w = self.page_w * zoom;
                let page_h = self.page_h * zoom;
                let page_x = 280.0 + (canvas_w - page_w) / 2.0 + self.pan_x;
                let page_y = 26.0 + (canvas_h - page_h) / 2.0 + self.pan_y;
                
                let cx = (px - page_x) / zoom;
                let cy = (py - page_y) / zoom;

                if self.word_processor_enabled {
                    if self.wp_text_box.mouse_input(button, state, px, py) {
                        changed = true;
                    } else if state == ElementState::Pressed {
                        self.wp_text_box.unfocus();
                        changed = true;
                    }
                } else if state == ElementState::Pressed {
                    let mut found = None;
                    for (idx, element) in self.elements.iter().enumerate().rev() {
                        let (ex, ey, ew, eh) = match element {
                            Element::Text { x, y, w, h, .. } => (*x, *y, *w, *h),
                            Element::Shape { x, y, w, h, .. } => (*x, *y, *w, *h),
                        };
                        if cx >= ex && cx <= ex + ew && cy >= ey && cy <= ey + eh {
                            found = Some(idx);
                            break;
                        }
                    }

                    if let Some(idx) = found {
                        self.selected_idx = Some(idx);
                        let (ex, ey) = match &self.elements[idx] {
                            Element::Text { x, y, .. } => (*x, *y),
                            Element::Shape { x, y, .. } => (*x, *y),
                        };
                        self.dragging = Some((idx, cx - ex, cy - ey));
                        self.rebuild_element_tab_widgets();
                        self.sync_sidebar_fields();
                        changed = true;
                    } else {
                        self.selected_idx = None;
                        self.dragging = None;
                        self.rebuild_element_tab_widgets();
                        self.sync_sidebar_fields();
                        changed = true;
                    }
                } else if state == ElementState::Released {
                    self.dragging = None;
                }
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
            if self.paginator.mouse_wheel(delta, px, py) {
                if self.paginator.selected_page() == 1 {
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
        let mut handled = false;
        
        if self.word_processor_enabled {
            if self.wp_text_box.keyboard_input(event) {
                handled = true;
            }
        }

        if self.paginator.keyboard_input(event) {
            handled = true;
            let active_page = self.paginator.selected_page();
            if active_page == 1 {
                self.apply_sidebar_changes();
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
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();
    
    clear_ui::engine::run::<LayoutApp>();
}
