use std::collections::HashMap;
use std::rc::Rc;

use eframe::egui;
use egui::pos2;
use egui::Color32;
use egui::Pos2;
use egui::Rect;
use egui::Shape;
use egui::TextureHandle;
use egui::TextureId;

use crate::game::Cell;
use crate::game::CellPosition;
use crate::game::Game;
use crate::game::GameState;
use crate::game::Rules;

const PIXEL_PER_CELL: f32 = 16.0;

enum Step {
    Initial,
    Game(Game),
    GameOver(String),
}

fn cell_rect(pos: CellPosition, offset: Pos2) -> egui::Rect {
    let x = pos.x as f32 * PIXEL_PER_CELL + offset.x;
    let y = pos.y as f32 * PIXEL_PER_CELL + offset.y;

    Rect::from_min_max(pos2(x, y), pos2(x + PIXEL_PER_CELL, y + PIXEL_PER_CELL))
}

pub fn gui() {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };
    eframe::run_native(
        "BomberHans",
        options,
        Box::new(|_cc| {
            Box::new(MyApp {
                step: Step::Initial,
                rules: Rules::default(),
                game_name: "A Game of Bomberhans".into(),
                player_name: "New Player".into(),
                textures: None,
            })
        }),
    )
}

struct TextureManager {
    textures: HashMap<&'static str, TextureHandle>,
}

impl TextureManager {
    fn get_texture(self: &Rc<Self>, texture: &str) -> TextureId {
        self.textures
            .get(texture)
            .ok_or_else(|| format!("Expected {texture} to exist"))
            .unwrap()
            .into()
    }

    fn get_cell(self: &Rc<Self>, cell: &Cell) -> TextureId {
        self.get_texture(&format!("cell_{}", cell.name()))
    }
}

struct MyApp {
    step: Step,
    rules: Rules,
    game_name: String,
    player_name: String,

    textures: Option<Rc<TextureManager>>,
}

impl MyApp {
    fn textures(&mut self, ctx: &egui::Context) -> Rc<TextureManager> {
        Rc::clone(self.textures.get_or_insert_with(|| {
            Rc::new(TextureManager {
                textures: load_tiles(ctx),
            })
        }))
    }

    fn update_initial(&mut self, ui: &mut egui::Ui) {
        if let Step::GameOver(s) = &self.step {
            ui.label(s);
        }
        ui.add(
            egui::Slider::new(&mut self.rules.width, 7..=99)
                .text("Width")
                .clamp_to_range(true),
        );
        ui.add(
            egui::Slider::new(&mut self.rules.height, 7..=99)
                .text("Height")
                .clamp_to_range(true),
        );
        ui.add(egui::Slider::new(&mut self.rules.players, 1..=4).text("Players"));
        if ui.button("Start local Game").clicked() {
            let game = Game::new_local_game(self.game_name.clone(), self.rules.clone());
            self.step = Step::Game(game);
        }
    }

    fn update_game(&mut self, ui: &mut egui::Ui) {
        let textures = self.textures(ui.ctx());
        let Step::Game(game) = &self.step else {unreachable!();};

        let width = game.game_static.rules.width as f32 * PIXEL_PER_CELL;
        let height = game.game_static.rules.height as f32 * PIXEL_PER_CELL;

        let game_field = ui.image(
            textures.get_texture("background"),
            egui::Vec2 {
                x: width,
                y: height,
            },
        );

        let painter = ui.painter_at(game_field.rect);

        painter.rect_stroke(
            game_field.rect,
            egui::Rounding::none(),
            egui::Stroke {
                width: 2.0,
                color: egui::Color32::GOLD,
            },
        );

        painter.extend(game.game_state.field.iter().map(|(pos, cell)| {
            Shape::image(
                textures.get_cell(cell),
                cell_rect(pos, game_field.rect.min),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            )
        }));
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bomberhans");
            match self.step {
                Step::GameOver(_) | Step::Initial => self.update_initial(ui),
                Step::Game(_) => self.update_game(ui),
            }
        });
    }
}

fn load_image_from_memory(image_data: &[u8]) -> egui::ColorImage {
    let image = image::load_from_memory(image_data).expect("resources can be loaded");
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
}

fn load_tiles(ctx: &egui::Context) -> HashMap<&'static str, TextureHandle> {
    let mut map = HashMap::new();
    macro_rules! load {
        ($x:expr) => {
            map.insert(
                $x,
                ctx.load_texture(
                    $x,
                    load_image_from_memory(include_bytes!(concat!("../images/", $x, ".bmp"))),
                    egui::TextureOptions::default(),
                ),
            )
        };
    }

    load!("cell_bomb");
    load!("cell_empty");
    load!("cell_fire");
    load!("cell_start_point");
    load!("cell_teleport");
    load!("cell_tomb_stone");
    load!("cell_upgrade_speed");
    load!("cell_upgrade_speed");
    load!("cell_upgrade_speed");
    load!("cell_wall");
    load!("cell_wood");
    load!("cell_wood_burning");

    map.insert(
        "background",
        ctx.load_texture(
            "background",
            egui::ColorImage::new([1, 1], egui::Color32::GRAY),
            egui::TextureOptions::default(),
        ),
    );
    map
}
