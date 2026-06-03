use wayland_client::QueueHandle;
use glyphon::{FontSystem, Buffer, Metrics, Attrs};
use clear_ui::engine::{Application, EngineState, LogicalPosition, LogicalSize, WindowSettings};
use clear_ui::widget::{
    MouseButton, ElementState, MouseScrollDelta, KeyEvent, TextItem, Widget,
    MenuBar, TextBox, Slider, TextLabel
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

struct LayoutApp {
    menu_bar: MenuBar,
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

    // Sidebar property controls
    sidebar_x: TextBox,
    sidebar_y: TextBox,
    sidebar_w: TextBox,
    sidebar_h: TextBox,
    sidebar_text: TextBox,
    sidebar_size: TextBox,
    
    // Sliders for RGB colors
    slider_r: Slider,
    slider_g: Slider,
    slider_b: Slider,
    
    last_selected: Option<usize>,
}

impl LayoutApp {
    fn rebuild_text_items(&mut self) {
        self.text_items.clear();
        
        let mut labels = Vec::new();
        
        // 1. MenuBar text labels
        labels.extend(self.menu_bar.text_labels());
        
        // 2. Sidebar text boxes
        labels.extend(self.sidebar_x.text_labels());
        labels.extend(self.sidebar_y.text_labels());
        labels.extend(self.sidebar_w.text_labels());
        labels.extend(self.sidebar_h.text_labels());
        labels.extend(self.sidebar_text.text_labels());
        labels.extend(self.sidebar_size.text_labels());
        
        // 3. Slider Labels
        labels.push(TextLabel {
            text: format!("Red Color: {:.2}", self.slider_r.value()),
            x: 20.0,
            y: 320.0,
            font_size: 11.0,
            color: [0x83, 0x83, 0x8a],
        });
        labels.push(TextLabel {
            text: format!("Green Color: {:.2}", self.slider_g.value()),
            x: 20.0,
            y: 360.0,
            font_size: 11.0,
            color: [0x83, 0x83, 0x8a],
        });
        labels.push(TextLabel {
            text: format!("Blue Color: {:.2}", self.slider_b.value()),
            x: 20.0,
            y: 400.0,
            font_size: 11.0,
            color: [0x83, 0x83, 0x8a],
        });
        
        // 4. Section headers
        labels.push(TextLabel {
            text: "LAYOUT PROPERTIES".to_string(),
            x: 20.0,
            y: 42.0,
            font_size: 13.0,
            color: [0xee, 0xee, 0xf5],
        });
        
        // Selection feedback
        if let Some(idx) = self.selected_idx {
            labels.push(TextLabel {
                text: format!("Selected Element #{}", idx + 1),
                x: 20.0,
                y: 450.0,
                font_size: 12.0,
                color: [0x3b, 0x82, 0xf6],
            });
        } else {
            labels.push(TextLabel {
                text: "No Selection".to_string(),
                x: 20.0,
                y: 450.0,
                font_size: 12.0,
                color: [0x83, 0x83, 0x8a],
            });
            labels.push(TextLabel {
                text: "Click canvas elements to edit properties.".to_string(),
                x: 20.0,
                y: 472.0,
                font_size: 11.0,
                color: [0x60, 0x60, 0x68],
            });
        }

        // Help Instructions at the bottom of the sidebar
        labels.push(TextLabel {
            text: "INSTRUCTIONS:".to_string(),
            x: 20.0,
            y: 530.0,
            font_size: 11.0,
            color: [0xaa, 0xaa, 0xbb],
        });
        labels.push(TextLabel {
            text: "- Click & Drag canvas elements to move".to_string(),
            x: 20.0,
            y: 550.0,
            font_size: 10.0,
            color: [0x83, 0x83, 0x8a],
        });
        labels.push(TextLabel {
            text: "- Use Menu Bar to add elements".to_string(),
            x: 20.0,
            y: 568.0,
            font_size: 10.0,
            color: [0x83, 0x83, 0x8a],
        });
        labels.push(TextLabel {
            text: "- Edit fields or drag Sliders to customize".to_string(),
            x: 20.0,
            y: 586.0,
            font_size: 10.0,
            color: [0x83, 0x83, 0x8a],
        });

        // 5. Canvas Element Labels
        for (idx, element) in self.elements.iter().enumerate() {
            match element {
                Element::Text { text, x, y, w: _, h: _, font_size, color } => {
                    labels.push(TextLabel {
                        text: text.clone(),
                        x: *x + 8.0,
                        y: *y + 6.0,
                        font_size: *font_size,
                        color: [
                            (color[0] * 255.0).clamp(0.0, 255.0) as u8,
                            (color[1] * 255.0).clamp(0.0, 255.0) as u8,
                            (color[2] * 255.0).clamp(0.0, 255.0) as u8,
                        ],
                    });
                }
                Element::Shape { shape_type, x, y, w: _, h: _, color: _ } => {
                    let type_str = match shape_type {
                        ShapeType::Rectangle => "Rectangle",
                        ShapeType::Banner => "Banner",
                    };
                    labels.push(TextLabel {
                        text: format!("{} #{}", type_str, idx + 1),
                        x: *x + 8.0,
                        y: *y + 6.0,
                        font_size: 11.0,
                        color: [0xee, 0xee, 0xf5],
                    });
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

    fn sync_sidebar_fields(&mut self) {
        if let Some(idx) = self.selected_idx {
            if self.last_selected != Some(idx) {
                // Focus reset on selection swap
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
                Element::Text { text, x, y, w, h, font_size, color } => {
                    if !self.sidebar_text.editing { self.sidebar_text.text = text.clone(); }
                    if !self.sidebar_x.editing { self.sidebar_x.text = format!("{:.0}", x); }
                    if !self.sidebar_y.editing { self.sidebar_y.text = format!("{:.0}", y); }
                    if !self.sidebar_w.editing { self.sidebar_w.text = format!("{:.0}", w); }
                    if !self.sidebar_h.editing { self.sidebar_h.text = format!("{:.0}", h); }
                    if !self.sidebar_size.editing { self.sidebar_size.text = format!("{:.0}", font_size); }
                    
                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);
                }
                Element::Shape { shape_type: _, x, y, w, h, color } => {
                    if !self.sidebar_text.editing { self.sidebar_text.text = "Shape Mode".to_string(); }
                    if !self.sidebar_x.editing { self.sidebar_x.text = format!("{:.0}", x); }
                    if !self.sidebar_y.editing { self.sidebar_y.text = format!("{:.0}", y); }
                    if !self.sidebar_w.editing { self.sidebar_w.text = format!("{:.0}", w); }
                    if !self.sidebar_h.editing { self.sidebar_h.text = format!("{:.0}", h); }
                    if !self.sidebar_size.editing { self.sidebar_size.text = "N/A".to_string(); }
                    
                    self.slider_r.set_value(color[0]);
                    self.slider_g.set_value(color[1]);
                    self.slider_b.set_value(color[2]);
                }
            }
        }
    }

    fn apply_sidebar_changes(&mut self) {
        if let Some(idx) = self.selected_idx {
            let text_val = if self.sidebar_text.editing { self.sidebar_text.edit_buffer.clone() } else { self.sidebar_text.text.clone() };
            let x_val = if self.sidebar_x.editing { &self.sidebar_x.edit_buffer } else { &self.sidebar_x.text }.parse::<f32>().unwrap_or(0.0);
            let y_val = if self.sidebar_y.editing { &self.sidebar_y.edit_buffer } else { &self.sidebar_y.text }.parse::<f32>().unwrap_or(0.0);
            let w_val = if self.sidebar_w.editing { &self.sidebar_w.edit_buffer } else { &self.sidebar_w.text }.parse::<f32>().unwrap_or(100.0);
            let h_val = if self.sidebar_h.editing { &self.sidebar_h.edit_buffer } else { &self.sidebar_h.text }.parse::<f32>().unwrap_or(50.0);
            let size_val = if self.sidebar_size.editing { &self.sidebar_size.edit_buffer } else { &self.sidebar_size.text }.parse::<f32>().unwrap_or(16.0);
            
            let r_val = self.slider_r.value();
            let g_val = self.slider_g.value();
            let b_val = self.slider_b.value();
            
            match &mut self.elements[idx] {
                Element::Text { text, x, y, w, h, font_size, color } => {
                    *text = text_val;
                    *x = x_val;
                    *y = y_val;
                    *w = w_val;
                    *h = h_val;
                    *font_size = size_val;
                    *color = [r_val, g_val, b_val, 1.0];
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

        // Initialize elements with a basic greeting layout
        let elements = vec![
            Element::Text {
                text: "Welcome to Clear Layout!".to_string(),
                x: 320.0,
                y: 80.0,
                w: 400.0,
                h: 40.0,
                font_size: 20.0,
                color: [0.3, 0.7, 1.0, 1.0],
            },
            Element::Shape {
                shape_type: ShapeType::Rectangle,
                x: 320.0,
                y: 130.0,
                w: 600.0,
                h: 4.0,
                color: [0.2, 0.5, 0.9, 1.0],
            },
            Element::Text {
                text: "This is a word processor and layout editor layout.".to_string(),
                x: 320.0,
                y: 160.0,
                w: 500.0,
                h: 30.0,
                font_size: 14.0,
                color: [0.8, 0.8, 0.85, 1.0],
            },
            Element::Shape {
                shape_type: ShapeType::Rectangle,
                x: 320.0,
                y: 220.0,
                w: 120.0,
                h: 80.0,
                color: [0.15, 0.25, 0.4, 1.0],
            },
            Element::Text {
                text: "Card Label".to_string(),
                x: 330.0,
                y: 235.0,
                w: 100.0,
                h: 20.0,
                font_size: 11.0,
                color: [0.9, 0.9, 0.9, 1.0],
            },
        ];

        // Create sidebar textbox inputs
        let sidebar_x = TextBox::new(String::new()).with_label("X Position");
        let sidebar_y = TextBox::new(String::new()).with_label("Y Position");
        let sidebar_w = TextBox::new(String::new()).with_label("Width");
        let sidebar_h = TextBox::new(String::new()).with_label("Height");
        let sidebar_text = TextBox::new(String::new()).with_label("Text Content");
        let sidebar_size = TextBox::new(String::new()).with_label("Text Size");

        // Create color sliders
        let slider_r = Slider::new().with_range(0.0, 1.0).with_value(0.5);
        let slider_g = Slider::new().with_range(0.0, 1.0).with_value(0.5);
        let slider_b = Slider::new().with_range(0.0, 1.0).with_value(0.5);

        let mut app = Self {
            menu_bar,
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
            
            sidebar_x,
            sidebar_y,
            sidebar_w,
            sidebar_h,
            sidebar_text,
            sidebar_size,
            
            slider_r,
            slider_g,
            slider_b,
            
            last_selected: None,
        };

        app.sync_sidebar_fields();
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
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddText => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Text {
                    text: "New Text Block".to_string(),
                    x: 350.0,
                    y: 200.0,
                    w: 200.0,
                    h: 30.0,
                    font_size: 14.0,
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                self.selected_idx = Some(next_idx);
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddRectangle => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Shape {
                    shape_type: ShapeType::Rectangle,
                    x: 350.0,
                    y: 200.0,
                    w: 150.0,
                    h: 100.0,
                    color: [0.2, 0.6, 0.4, 1.0],
                });
                self.selected_idx = Some(next_idx);
                self.sync_sidebar_fields();
                *needs_rebuild = true;
                self.needs_rebuild = true;
            }
            AppMessage::AddBanner => {
                let next_idx = self.elements.len();
                self.elements.push(Element::Shape {
                    shape_type: ShapeType::Banner,
                    x: 350.0,
                    y: 200.0,
                    w: 300.0,
                    h: 50.0,
                    color: [0.8, 0.4, 0.2, 1.0],
                });
                self.selected_idx = Some(next_idx);
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

    fn tick(&mut self, _dt: f32, _needs_rebuild: &mut bool) {}

    fn view(&mut self, quads: &mut Vec<(f32, f32, f32, f32, [f32; 4])>, size: LogicalSize, scale: f64) {
        let size_changed = self.width != size.width as u32 || self.height != size.height as u32 || self.scale_factor != scale;
        if self.needs_rebuild || size_changed {
            self.width = size.width as u32;
            self.height = size.height as u32;
            self.scale_factor = scale;
            
            // Layout MenuBar at the top
            self.menu_bar.set_rect(0.0, 0.0, size.width, 26.0);
            
            // Position sidebar controls
            self.sidebar_x.set_rect(20.0, 70.0, 110.0, 26.0);
            self.sidebar_y.set_rect(150.0, 70.0, 110.0, 26.0);
            self.sidebar_w.set_rect(20.0, 130.0, 110.0, 26.0);
            self.sidebar_h.set_rect(150.0, 130.0, 110.0, 26.0);
            self.sidebar_text.set_rect(20.0, 190.0, 240.0, 26.0);
            self.sidebar_size.set_rect(20.0, 250.0, 240.0, 26.0);
            
            self.slider_r.set_rect(20.0, 330.0, 240.0, 18.0);
            self.slider_g.set_rect(20.0, 370.0, 240.0, 18.0);
            self.slider_b.set_rect(20.0, 410.0, 240.0, 18.0);

            self.rebuild_text_items();
            self.needs_rebuild = false;
        }

        // 1. Sidebar Background Panel
        quads.push((0.0, 26.0, 280.0, self.height as f32 - 26.0, [0.08, 0.08, 0.10, 1.0]));

        // 2. Canvas Background
        quads.push((280.0, 26.0, self.width as f32 - 280.0, self.height as f32 - 26.0, [0.12, 0.12, 0.15, 1.0]));

        // Divider between Sidebar and Canvas
        quads.push((280.0, 26.0, 1.0, self.height as f32 - 26.0, [0.20, 0.20, 0.25, 1.0]));

        // 3. Grid rendering
        if self.grid_enabled {
            let spacing = 20.0;
            let dot_color = [0.25, 0.25, 0.32, 0.35];
            let mut cx = 280.0 + spacing;
            while cx < self.width as f32 {
                let mut cy = 26.0 + spacing;
                while cy < self.height as f32 {
                    quads.push((cx, cy, 1.5, 1.5, dot_color));
                    cy += spacing;
                }
                cx += spacing;
            }
        }

        // 4. Render Layout Elements
        for (idx, element) in self.elements.iter().enumerate() {
            match element {
                Element::Text { text: _, x, y, w, h, font_size: _, color: _ } => {
                    // Translucent backing for text boundary
                    quads.push((*x, *y, *w, *h, [0.18, 0.18, 0.22, 0.25]));
                    // Subtle text box border
                    let border_color = [0.25, 0.25, 0.30, 0.5];
                    quads.push((*x, *y, *w, 1.0, border_color));
                    quads.push((*x, *y + *h - 1.0, *w, 1.0, border_color));
                    quads.push((*x, *y, 1.0, *h, border_color));
                    quads.push((*x + *w - 1.0, *y, 1.0, *h, border_color));
                }
                Element::Shape { shape_type, x, y, w, h, color } => {
                    match shape_type {
                        ShapeType::Rectangle => {
                            quads.push((*x, *y, *w, *h, *color));
                        }
                        ShapeType::Banner => {
                            // Fancy drop shadow
                            quads.push((*x + 3.0, *y + 3.0, *w, *h, [0.05, 0.05, 0.08, 0.4]));
                            // Main banner quad
                            quads.push((*x, *y, *w, *h, *color));
                            // Inner outline
                            let inner_col = [1.0, 1.0, 1.0, 0.2];
                            quads.push((*x + 2.0, *y + 2.0, *w - 4.0, 1.0, inner_col));
                            quads.push((*x + 2.0, *y + *h - 3.0, *w - 4.0, 1.0, inner_col));
                            quads.push((*x + 2.0, *y + 2.0, 1.0, *h - 4.0, inner_col));
                            quads.push((*x + *w - 3.0, *y + 2.0, 1.0, *h - 4.0, inner_col));
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
                let sel_col = [0.23, 0.51, 0.96, 1.0]; // Bright neon blue
                quads.push((ex - 1.0, ey - 1.0, ew + 2.0, 1.0, sel_col));
                quads.push((ex - 1.0, ey + eh, ew + 2.0, 1.0, sel_col));
                quads.push((ex - 1.0, ey - 1.0, 1.0, eh + 2.0, sel_col));
                quads.push((ex + ew, ey - 1.0, 1.0, eh + 2.0, sel_col));

                // Corner handles
                let hs = 5.0;
                let hc = [1.0, 1.0, 1.0, 1.0];
                let hborder = [0.23, 0.51, 0.96, 1.0];
                let handles = [
                    (ex - 2.0, ey - 2.0),
                    (ex + ew - 3.0, ey - 2.0),
                    (ex - 2.0, ey + eh - 3.0),
                    (ex + ew - 3.0, ey + eh - 3.0),
                ];
                for (hx, hy) in handles {
                    quads.push((hx, hy, hs, hs, hborder));
                    quads.push((hx + 1.0, hy + 1.0, hs - 2.0, hs - 2.0, hc));
                }
            }
        }

        // 5. Sidebar Widget extra quads (backgrounds, highlights, values)
        quads.extend(self.sidebar_x.extra_quads());
        quads.extend(self.sidebar_y.extra_quads());
        quads.extend(self.sidebar_w.extra_quads());
        quads.extend(self.sidebar_h.extra_quads());
        quads.extend(self.sidebar_text.extra_quads());
        quads.extend(self.sidebar_size.extra_quads());

        let (rx, ry, rw, rh) = self.slider_r.rect();
        quads.push((rx, ry, rw, rh, self.slider_r.color()));
        quads.extend(self.slider_r.extra_quads());

        let (gx, gy, gw, gh) = self.slider_g.rect();
        quads.push((gx, gy, gw, gh, self.slider_g.color()));
        quads.extend(self.slider_g.extra_quads());

        let (bx, by, bw, bh) = self.slider_b.rect();
        quads.push((bx, by, bw, bh, self.slider_b.color()));
        quads.extend(self.slider_b.extra_quads());

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
                if self.sidebar_x.on_cursor_moved(px, py) { changed = true; }
                if self.sidebar_y.on_cursor_moved(px, py) { changed = true; }
                if self.sidebar_w.on_cursor_moved(px, py) { changed = true; }
                if self.sidebar_h.on_cursor_moved(px, py) { changed = true; }
                if self.sidebar_text.on_cursor_moved(px, py) { changed = true; }
                if self.sidebar_size.on_cursor_moved(px, py) { changed = true; }
                
                if self.slider_r.is_dragging() {
                    if self.slider_r.drag_update(px, py) { changed = true; }
                } else if self.slider_r.on_cursor_moved(px, py) { changed = true; }

                if self.slider_g.is_dragging() {
                    if self.slider_g.drag_update(px, py) { changed = true; }
                } else if self.slider_g.on_cursor_moved(px, py) { changed = true; }

                if self.slider_b.is_dragging() {
                    if self.slider_b.drag_update(px, py) { changed = true; }
                } else if self.slider_b.on_cursor_moved(px, py) { changed = true; }
                
                if changed {
                    self.apply_sidebar_changes();
                }
            } else {
                if let Some((idx, ox, oy)) = self.dragging {
                    let mut new_x = px - ox;
                    let mut new_y = py - oy;
                    
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
                } else {
                    // Feed hovered states to buttons
                    if self.sidebar_x.on_cursor_moved(px, py) { changed = true; }
                    if self.sidebar_y.on_cursor_moved(px, py) { changed = true; }
                    if self.sidebar_w.on_cursor_moved(px, py) { changed = true; }
                    if self.sidebar_h.on_cursor_moved(px, py) { changed = true; }
                    if self.sidebar_text.on_cursor_moved(px, py) { changed = true; }
                    if self.sidebar_size.on_cursor_moved(px, py) { changed = true; }
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
                if state == ElementState::Pressed {
                    let mut clicked = false;
                    if self.sidebar_x.mouse_input(button, state, px, py) { clicked = true; }
                    if self.sidebar_y.mouse_input(button, state, px, py) { clicked = true; }
                    if self.sidebar_w.mouse_input(button, state, px, py) { clicked = true; }
                    if self.sidebar_h.mouse_input(button, state, px, py) { clicked = true; }
                    if self.sidebar_text.mouse_input(button, state, px, py) { clicked = true; }
                    if self.sidebar_size.mouse_input(button, state, px, py) { clicked = true; }
                    
                    if self.slider_r.mouse_input(button, state, px, py) { clicked = true; }
                    if self.slider_g.mouse_input(button, state, px, py) { clicked = true; }
                    if self.slider_b.mouse_input(button, state, px, py) { clicked = true; }

                    if !self.sidebar_x.hit_test(px, py) { self.sidebar_x.unfocus(); }
                    if !self.sidebar_y.hit_test(px, py) { self.sidebar_y.unfocus(); }
                    if !self.sidebar_w.hit_test(px, py) { self.sidebar_w.unfocus(); }
                    if !self.sidebar_h.hit_test(px, py) { self.sidebar_h.unfocus(); }
                    if !self.sidebar_text.hit_test(px, py) { self.sidebar_text.unfocus(); }
                    if !self.sidebar_size.hit_test(px, py) { self.sidebar_size.unfocus(); }

                    if clicked {
                        self.apply_sidebar_changes();
                        changed = true;
                    }
                } else {
                    self.slider_r.mouse_input(button, state, px, py);
                    self.slider_g.mouse_input(button, state, px, py);
                    self.slider_b.mouse_input(button, state, px, py);
                    self.apply_sidebar_changes();
                    changed = true;
                }
            } else {
                if state == ElementState::Pressed {
                    let mut found = None;
                    for (idx, element) in self.elements.iter().enumerate().rev() {
                        let (ex, ey, ew, eh) = match element {
                            Element::Text { x, y, w, h, .. } => (*x, *y, *w, *h),
                            Element::Shape { x, y, w, h, .. } => (*x, *y, *w, *h),
                        };
                        if px >= ex && px <= ex + ew && py >= ey && py <= ey + eh {
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
                        self.dragging = Some((idx, px - ex, py - ey));
                        self.sync_sidebar_fields();
                        changed = true;
                    } else {
                        self.selected_idx = None;
                        self.dragging = None;
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

    fn handle_mouse_wheel(&mut self, _delta: &MouseScrollDelta, _pos: LogicalPosition, _needs_rebuild: &mut bool) {}

    fn handle_key_input(&mut self, event: &KeyEvent, needs_rebuild: &mut bool) -> Option<Self::Message> {
        let mut handled = false;
        
        if self.sidebar_x.editing {
            if self.sidebar_x.keyboard_input(event) { handled = true; }
        } else if self.sidebar_y.editing {
            if self.sidebar_y.keyboard_input(event) { handled = true; }
        } else if self.sidebar_w.editing {
            if self.sidebar_w.keyboard_input(event) { handled = true; }
        } else if self.sidebar_h.editing {
            if self.sidebar_h.keyboard_input(event) { handled = true; }
        } else if self.sidebar_text.editing {
            if self.sidebar_text.keyboard_input(event) { handled = true; }
        } else if self.sidebar_size.editing {
            if self.sidebar_size.keyboard_input(event) { handled = true; }
        }

        if handled {
            *needs_rebuild = true;
            self.needs_rebuild = true;
            self.apply_sidebar_changes();
        }

        None
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();
    
    clear_ui::engine::run::<LayoutApp>();
}
