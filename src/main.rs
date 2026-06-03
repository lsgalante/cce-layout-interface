use wayland_client::QueueHandle;
use glyphon::{FontSystem, Buffer, Metrics, Attrs};
use clear_ui::engine::{Application, EngineState, LogicalPosition, LogicalSize, WindowSettings};
use clear_ui::widget::{
    MouseButton, ElementState, MouseScrollDelta, KeyEvent, TextItem, Widget,
    MenuBar, TextBox, Slider, TextLabel, Paginator, Button, Dropdown, Toggle
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
    sidebar_x: TextBox,
    sidebar_y: TextBox,
    sidebar_w: TextBox,
    sidebar_h: TextBox,
    sidebar_text: TextBox,
    sidebar_size: TextBox,
    
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

    last_selected: Option<usize>,
    page_w: f32,
    page_h: f32,
}

impl LayoutApp {
    fn rebuild_text_items(&mut self) {
        self.text_items.clear();
        
        let mut labels = Vec::new();

        let canvas_w = self.width as f32 - 280.0;
        let canvas_h = self.height as f32 - 26.0;
        let zoom = self.slider_zoom.value();
        let page_w = self.page_w * zoom;
        let page_h = self.page_h * zoom;
        let page_x = 280.0 + (canvas_w - page_w) / 2.0;
        let page_y = 26.0 + (canvas_h - page_h) / 2.0;
        
        // 1. MenuBar text labels
        labels.extend(self.menu_bar.text_labels());
        
        // 2. Paginator sidebar tabs
        labels.extend(self.paginator.text_labels());
        
        // 3. Conditional Page rendering
        if self.paginator.selected_page() == 0 {
            // Page Settings tab (no header)
            labels.extend(self.dropdown_presets.text_labels());

            // Add drop down list labels dynamically if open
            if self.dropdown_presets.open {
                let mut collector = clear_ui::layout::PopoverCollector::new();
                self.dropdown_presets.render_popover(&mut collector);
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
            }

            // Display active page width and height (shifted up from 160/180)
            labels.push(TextLabel {
                text: format!("Width: {} px", self.page_w),
                x: 20.0,
                y: 140.0,
                font_size: 11.0,
                color: [0x83, 0x83, 0x8a],
            });
            labels.push(TextLabel {
                text: format!("Height: {} px", self.page_h),
                x: 20.0,
                y: 160.0,
                font_size: 11.0,
                color: [0x83, 0x83, 0x8a],
            });
        } else if self.paginator.selected_page() == 1 {
            // Element Properties tab (renamed from Layout)
            labels.extend(self.sidebar_x.text_labels());
            labels.extend(self.sidebar_y.text_labels());
            labels.extend(self.sidebar_w.text_labels());
            labels.extend(self.sidebar_h.text_labels());
            labels.extend(self.sidebar_text.text_labels());
            labels.extend(self.sidebar_size.text_labels());
            
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
            
            if let Some(idx) = self.selected_idx {
                labels.push(TextLabel {
                    text: format!("Selected Element #{}", idx + 1),
                    x: 20.0,
                    y: 445.0,
                    font_size: 12.0,
                    color: [0x3b, 0x82, 0xf6],
                });
            } else {
                labels.push(TextLabel {
                    text: "No Selection".to_string(),
                    x: 20.0,
                    y: 445.0,
                    font_size: 12.0,
                    color: [0x83, 0x83, 0x8a],
                });
                labels.push(TextLabel {
                    text: "Click canvas elements".to_string(),
                    x: 20.0,
                    y: 467.0,
                    font_size: 10.0,
                    color: [0x60, 0x60, 0x68],
                });
                labels.push(TextLabel {
                    text: "to edit properties.".to_string(),
                    x: 20.0,
                    y: 483.0,
                    font_size: 10.0,
                    color: [0x60, 0x60, 0x68],
                });
            }
        } else if self.paginator.selected_page() == 2 {
            // Canvas Options Page (no header)
            labels.extend(self.btn_toggle_grid.text_labels());
            labels.extend(self.btn_clear_canvas.text_labels());
            labels.extend(self.btn_add_text.text_labels());
            labels.extend(self.btn_add_rect.text_labels());
            labels.extend(self.btn_add_banner.text_labels());
            
            labels.push(TextLabel {
                text: format!("Grid Snapping: {}", if self.grid_enabled { "ON (20px)" } else { "OFF" }),
                x: 20.0,
                y: 350.0,
                font_size: 11.0,
                color: [0x83, 0x83, 0x8a],
            });
            labels.push(TextLabel {
                text: format!("Total Elements: {}", self.elements.len()),
                x: 20.0,
                y: 370.0,
                font_size: 11.0,
                color: [0x83, 0x83, 0x8a],
            });
        } else {
            // View Options Page (no header)
            labels.push(TextLabel {
                text: format!("Zoom: {:.0}%", self.slider_zoom.value() * 100.0),
                x: 20.0,
                y: 90.0,
                font_size: 11.0,
                color: [0x83, 0x83, 0x8a],
            });

            labels.extend(self.toggle_rulers.text_labels());
        }

        if self.toggle_rulers.toggled() {
            // X Ruler Labels
            let mut val = 0.0;
            while val <= self.page_w {
                if val as i32 % 100 == 0 {
                    labels.push(TextLabel {
                        text: format!("{}", val),
                        x: page_x + val * zoom + 2.0,
                        y: page_y - 16.0,
                        font_size: 8.0,
                        color: [0xaa, 0xaa, 0xbb],
                    });
                }
                val += 50.0;
            }

            // Y Ruler Labels
            let mut val = 0.0;
            while val <= self.page_h {
                if val as i32 % 100 == 0 {
                    labels.push(TextLabel {
                        text: format!("{}", val),
                        x: page_x - 18.0,
                        y: page_y + val * zoom + 2.0,
                        font_size: 8.0,
                        color: [0xaa, 0xaa, 0xbb],
                    });
                }
                val += 50.0;
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

        for (idx, element) in self.elements.iter().enumerate() {
            match element {
                Element::Text { text, x, y, w: _, h: _, font_size, color } => {
                    labels.push(TextLabel {
                        text: text.clone(),
                        x: page_x + *x * zoom + 8.0 * zoom,
                        y: page_y + *y * zoom + 6.0 * zoom,
                        font_size: *font_size * zoom,
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
                        x: page_x + *x * zoom + 8.0 * zoom,
                        y: page_y + *y * zoom + 6.0 * zoom,
                        font_size: 11.0 * zoom,
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

        // Build the sidebar Paginator
        let paginator = Paginator::new(80.0, vec!["Page".to_string(), "Element".to_string(), "Canvas".to_string(), "View".to_string()])
            .with_tabs_at_top(true);

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
            },
        ];

        // Create sidebar dropdown for page size presets
        let dropdown_presets = Dropdown::new(
            PRESETS.iter().map(|p| p.name.to_string()).collect(),
            0
        ).with_label("Page Size Preset");

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
        let slider_zoom = Slider::new().with_range(0.5, 2.0).with_value(1.0).with_scroll(true);

        // Page 1 Buttons
        let btn_toggle_grid = Button::new(0.0, 0.0, 0.0, 0.0).with_label("Toggle Grid");
        let btn_clear_canvas = Button::new(0.0, 0.0, 0.0, 0.0).with_label("Clear Canvas");
        let btn_add_text = Button::new(0.0, 0.0, 0.0, 0.0).with_label("Add Text Box");
        let btn_add_rect = Button::new(0.0, 0.0, 0.0, 0.0).with_label("Add Rectangle");
        let btn_add_banner = Button::new(0.0, 0.0, 0.0, 0.0).with_label("Add Banner");

        // Page 3 Buttons
        let toggle_rulers = Toggle::new().with_label("Show Rulers");

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
            sidebar_x,
            sidebar_y,
            sidebar_w,
            sidebar_h,
            sidebar_text,
            sidebar_size,
            
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
            
            last_selected: None,
            page_w,
            page_h,
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
                    x: 40.0,
                    y: 100.0,
                    w: 200.0,
                    h: 30.0,
                    font_size: 14.0,
                    color: [0.0, 0.0, 0.0, 1.0],
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
                    x: 40.0,
                    y: 100.0,
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
                    x: 40.0,
                    y: 100.0,
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

    fn tick(&mut self, dt: f32, needs_rebuild: &mut bool) {
        if self.paginator.tick(dt) {
            *needs_rebuild = true;
            self.needs_rebuild = true;
        }
    }

    fn view(&mut self, quads: &mut Vec<(f32, f32, f32, f32, [f32; 4])>, size: LogicalSize, scale: f64) {
        let size_changed = self.width != size.width as u32 || self.height != size.height as u32 || self.scale_factor != scale;
        if self.needs_rebuild || size_changed {
            self.width = size.width as u32;
            self.height = size.height as u32;
            self.scale_factor = scale;
            
            // Layout MenuBar at the top
            self.menu_bar.set_rect(0.0, 0.0, size.width, 26.0);
            
            // Set paginator bounds in the sidebar region
            self.paginator.set_rect(0.0, 26.0, 280.0, size.height - 26.0);
            
            // Layout presets dropdown selector (shifted up to y=90.0)
            self.dropdown_presets.set_rect(20.0, 90.0, 240.0, 26.0);
 
            // Position Layout properties control widgets (widened to 240px and realigned below top tabs)
            self.sidebar_x.set_rect(20.0, 90.0, 110.0, 26.0);
            self.sidebar_y.set_rect(150.0, 90.0, 110.0, 26.0);
            self.sidebar_w.set_rect(20.0, 146.0, 110.0, 26.0);
            self.sidebar_h.set_rect(150.0, 146.0, 110.0, 26.0);
            self.sidebar_text.set_rect(20.0, 202.0, 240.0, 26.0);
            self.sidebar_size.set_rect(20.0, 258.0, 240.0, 26.0);
            
            self.slider_r.set_rect(20.0, 330.0, 240.0, 18.0);
            self.slider_g.set_rect(20.0, 370.0, 240.0, 18.0);
            self.slider_b.set_rect(20.0, 410.0, 240.0, 18.0);
            self.slider_zoom.set_rect(20.0, 110.0, 240.0, 18.0);
 
            // Canvas Page buttons (shifted up by 20px)
            self.btn_toggle_grid.set_rect(20.0, 90.0, 240.0, 26.0);
            self.btn_clear_canvas.set_rect(20.0, 140.0, 240.0, 26.0);
            self.btn_add_text.set_rect(20.0, 190.0, 240.0, 26.0);
            self.btn_add_rect.set_rect(20.0, 240.0, 240.0, 26.0);
            self.btn_add_banner.set_rect(20.0, 290.0, 240.0, 26.0);

            // View Page rulers button
            self.toggle_rulers.set_rect(20.0, 180.0, 240.0, 26.0);
 
            self.rebuild_text_items();
            self.needs_rebuild = false;
        }
 
        // 1. Render Paginator Sidebar (includes backgrounds and active tab sliding container)
        quads.extend(self.paginator.extra_quads());
        if let Some(hq) = self.paginator.highlight_quad() {
            quads.push(hq);
        }
 
        // 2. Render Page Content
        if self.paginator.selected_page() == 0 {
            // Render Page 0: Presets Dropdown (shifted up to y=90)
            quads.push((20.0, 90.0, 240.0, 26.0, self.dropdown_presets.color()));
            quads.extend(self.dropdown_presets.extra_quads());
        } else if self.paginator.selected_page() == 1 {
            // Render Page 1: Element Properties
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
        } else if self.paginator.selected_page() == 2 {
            // Render Page 2: Canvas settings (shifted up by 20px)
            quads.push((20.0, 90.0, 240.0, 26.0, self.btn_toggle_grid.color()));
            quads.extend(self.btn_toggle_grid.extra_quads());
            
            quads.push((20.0, 140.0, 240.0, 26.0, self.btn_clear_canvas.color()));
            quads.extend(self.btn_clear_canvas.extra_quads());
            
            quads.push((20.0, 190.0, 240.0, 26.0, self.btn_add_text.color()));
            quads.extend(self.btn_add_text.extra_quads());
            
            quads.push((20.0, 240.0, 240.0, 26.0, self.btn_add_rect.color()));
            quads.extend(self.btn_add_rect.extra_quads());
            
            quads.push((20.0, 290.0, 240.0, 26.0, self.btn_add_banner.color()));
            quads.extend(self.btn_add_banner.extra_quads());
        } else {
            // Render Page 3: View settings
            let (zx, zy, zw, zh) = self.slider_zoom.rect();
            quads.push((zx, zy, zw, zh, self.slider_zoom.color()));
            quads.extend(self.slider_zoom.extra_quads());

            quads.push((20.0, 180.0, 240.0, 26.0, self.toggle_rulers.color()));
            quads.extend(self.toggle_rulers.extra_quads());
        }

        // Divider between Sidebar and Canvas
        quads.push((280.0, 26.0, 1.0, self.height as f32 - 26.0, [0.20, 0.20, 0.25, 1.0]));

        // 3. Render Canvas & centered paper sheet
        let canvas_w = self.width as f32 - 280.0;
        let canvas_h = self.height as f32 - 26.0;
        let zoom = self.slider_zoom.value();
        
        // Centered Paper Position
        let page_x = 280.0 + (canvas_w - self.page_w * zoom) / 2.0;
        let page_y = 26.0 + (canvas_h - self.page_h * zoom) / 2.0;

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

            // X Axis Ticks
            let mut val = 0.0;
            while val <= self.page_w {
                let tx = page_x + val * zoom;
                if val as i32 % 100 == 0 {
                    quads.push((tx, page_y - 12.0, 1.0, 12.0, tick_color));
                } else {
                    quads.push((tx, page_y - 6.0, 1.0, 6.0, tick_color));
                }
                val += 50.0;
            }

            // Y Axis Ticks
            let mut val = 0.0;
            while val <= self.page_h {
                let ty = page_y + val * zoom;
                if val as i32 % 100 == 0 {
                    quads.push((page_x - 12.0, ty, 12.0, 1.0, tick_color));
                } else {
                    quads.push((page_x - 6.0, ty, 6.0, 1.0, tick_color));
                }
                val += 50.0;
            }

            // Corner block
            quads.push((page_x - 20.0, page_y - 20.0, 20.0, 20.0, [0.15, 0.15, 0.18, 1.0]));
            quads.push((page_x - 20.0, page_y - 20.0, 20.0, 1.0, border_col));
            quads.push((page_x - 20.0, page_y - 20.0, 1.0, 20.0, border_col));
        }

        // Paper drop shadow
        quads.push((page_x + 3.0, page_y + 3.0, self.page_w * zoom, self.page_h * zoom, [0.05, 0.05, 0.08, 0.35]));

        // Paper sheet backing (Elegant Off-White)
        quads.push((page_x, page_y, self.page_w * zoom, self.page_h * zoom, [0.96, 0.96, 0.98, 1.0]));
        
        // Thin paper border outline
        let border_col = [0.75, 0.75, 0.80, 0.5];
        quads.push((page_x, page_y, self.page_w * zoom, 1.0, border_col));
        quads.push((page_x, page_y + self.page_h * zoom - 1.0, self.page_w * zoom, 1.0, border_col));
        quads.push((page_x, page_y, 1.0, self.page_h * zoom, border_col));
        quads.push((page_x + self.page_w * zoom - 1.0, page_y, 1.0, self.page_h * zoom, border_col));

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

        // 5. Render Layout Elements (drawn relative to the paper sheet)
        for (idx, element) in self.elements.iter().enumerate() {
            match element {
                Element::Text { text: _, x, y, w, h, font_size: _, color: _ } => {
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

        // Render Page presets drop down popover box (if open, overlaps other sidebar elements)
        if self.paginator.selected_page() == 0 && self.dropdown_presets.open {
            let mut collector = clear_ui::layout::PopoverCollector::new();
            self.dropdown_presets.render_popover(&mut collector);
            for (color, x, y, w, h) in collector.rects {
                quads.push((x, y, w, h, color));
            }
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
                
                if self.paginator.selected_page() == 0 {
                    // Check dropdown presets moves first
                    if self.dropdown_presets.open || self.dropdown_presets.hit_test(px, py) {
                        if self.dropdown_presets.on_cursor_moved(px, py) {
                            changed = true;
                        }
                    }
                } else if self.paginator.selected_page() == 1 {
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
                } else if self.paginator.selected_page() == 2 {
                    if self.btn_toggle_grid.on_cursor_moved(px, py) { changed = true; }
                    if self.btn_clear_canvas.on_cursor_moved(px, py) { changed = true; }
                    if self.btn_add_text.on_cursor_moved(px, py) { changed = true; }
                    if self.btn_add_rect.on_cursor_moved(px, py) { changed = true; }
                    if self.btn_add_banner.on_cursor_moved(px, py) { changed = true; }
                } else {
                    if self.slider_zoom.is_dragging() {
                        if self.slider_zoom.drag_update(px, py) { changed = true; }
                    } else if self.slider_zoom.on_cursor_moved(px, py) { changed = true; }
                    if self.toggle_rulers.on_cursor_moved(px, py) { changed = true; }
                }
            } else {
                // Compute centering coordinates for canvas elements
                let canvas_w = self.width as f32 - 280.0;
                let canvas_h = self.height as f32 - 26.0;
                let zoom = self.slider_zoom.value();
                let page_w = self.page_w * zoom;
                let page_h = self.page_h * zoom;
                let page_x = 280.0 + (canvas_w - page_w) / 2.0;
                let page_y = 26.0 + (canvas_h - page_h) / 2.0;
                
                let cx = (px - page_x) / zoom;
                let cy = (py - page_y) / zoom;

                if let Some((idx, ox, oy)) = self.dragging {
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
                        self.sidebar_x.unfocus();
                        self.sidebar_y.unfocus();
                        self.sidebar_w.unfocus();
                        self.sidebar_h.unfocus();
                        self.sidebar_text.unfocus();
                        self.sidebar_size.unfocus();
                    }
                } else if self.paginator.selected_page() == 0 {
                    if state == ElementState::Pressed {
                        let mut clicked = false;
                        
                        // Check presets dropdown clicks first
                        if self.dropdown_presets.open || self.dropdown_presets.hit_test(px, py) {
                            if self.dropdown_presets.mouse_input(button, state, px, py) {
                                clicked = true;
                                if self.dropdown_presets.take_change() {
                                    let sel = self.dropdown_presets.selected;
                                    if sel < PRESETS.len() {
                                        self.page_w = PRESETS[sel].w;
                                        self.page_h = PRESETS[sel].h;
                                    }
                                }
                            }
                        }

                        if !self.dropdown_presets.hit_test(px, py) { self.dropdown_presets.unfocus(); }

                        if clicked {
                            changed = true;
                        }
                    }
                } else if self.paginator.selected_page() == 1 {
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
                } else if self.paginator.selected_page() == 2 {
                    // Canvas settings page input
                    if state == ElementState::Pressed {
                        if self.btn_toggle_grid.mouse_input(button, state, px, py) { changed = true; }
                        if self.btn_clear_canvas.mouse_input(button, state, px, py) { changed = true; }
                        if self.btn_add_text.mouse_input(button, state, px, py) { changed = true; }
                        if self.btn_add_rect.mouse_input(button, state, px, py) { changed = true; }
                        if self.btn_add_banner.mouse_input(button, state, px, py) { changed = true; }
                    } else if state == ElementState::Released {
                        if self.btn_toggle_grid.mouse_input(button, state, px, py) {
                            if self.btn_toggle_grid.take_click() {
                                msg_out = Some(AppMessage::ToggleGrid);
                            }
                            changed = true;
                        }
                        if self.btn_clear_canvas.mouse_input(button, state, px, py) {
                            if self.btn_clear_canvas.take_click() {
                                msg_out = Some(AppMessage::NewDocument);
                            }
                            changed = true;
                        }
                        if self.btn_add_text.mouse_input(button, state, px, py) {
                            if self.btn_add_text.take_click() {
                                msg_out = Some(AppMessage::AddText);
                            }
                            changed = true;
                        }
                        if self.btn_add_rect.mouse_input(button, state, px, py) {
                            if self.btn_add_rect.take_click() {
                                msg_out = Some(AppMessage::AddRectangle);
                            }
                            changed = true;
                        }
                        if self.btn_add_banner.mouse_input(button, state, px, py) {
                            if self.btn_add_banner.take_click() {
                                msg_out = Some(AppMessage::AddBanner);
                            }
                            changed = true;
                        }
                    }
                } else {
                    // View page input (zoom slider & rulers button)
                    if state == ElementState::Pressed {
                        if self.toggle_rulers.mouse_input(button, state, px, py) { changed = true; }
                        if self.slider_zoom.mouse_input(button, state, px, py) { changed = true; }
                    } else if state == ElementState::Released {
                        if self.toggle_rulers.mouse_input(button, state, px, py) {
                            let _ = self.toggle_rulers.take_click();
                            changed = true;
                        }
                        if self.slider_zoom.mouse_input(button, state, px, py) { changed = true; }
                    }
                }
            } else {
                // Compute centering coordinates for canvas elements
                let canvas_w = self.width as f32 - 280.0;
                let canvas_h = self.height as f32 - 26.0;
                let zoom = self.slider_zoom.value();
                let page_w = self.page_w * zoom;
                let page_h = self.page_h * zoom;
                let page_x = 280.0 + (canvas_w - page_w) / 2.0;
                let page_y = 26.0 + (canvas_h - page_h) / 2.0;
                
                let cx = (px - page_x) / zoom;
                let cy = (py - page_y) / zoom;

                if state == ElementState::Pressed {
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

    fn handle_mouse_wheel(&mut self, delta: &MouseScrollDelta, pos: LogicalPosition, needs_rebuild: &mut bool) {
        let px = pos.x as f32;
        let py = pos.y as f32;
        if px < 280.0 {
            if self.paginator.selected_page() == 1 {
                let mut changed = false;
                if self.slider_r.mouse_wheel(delta, px, py) { changed = true; }
                if self.slider_g.mouse_wheel(delta, px, py) { changed = true; }
                if self.slider_b.mouse_wheel(delta, px, py) { changed = true; }
                if changed {
                    self.apply_sidebar_changes();
                    *needs_rebuild = true;
                    self.needs_rebuild = true;
                }
            } else if self.paginator.selected_page() == 3 {
                if self.slider_zoom.mouse_wheel(delta, px, py) {
                    *needs_rebuild = true;
                    self.needs_rebuild = true;
                }
            }
        }
    }

    fn handle_key_input(&mut self, event: &KeyEvent, needs_rebuild: &mut bool) -> Option<Self::Message> {
        let mut handled = false;
        
        if self.paginator.selected_page() == 0 {
            if self.dropdown_presets.open {
                if self.dropdown_presets.keyboard_input(event) {
                    handled = true;
                    if self.dropdown_presets.take_change() {
                        let sel = self.dropdown_presets.selected;
                        if sel < PRESETS.len() {
                            self.page_w = PRESETS[sel].w;
                            self.page_h = PRESETS[sel].h;
                        }
                    }
                }
            }
        } else if self.paginator.selected_page() == 1 {
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
