use eframe::egui;

use crate::game::Game;
use crate::game::GameState;
use crate::game::Rules;

enum Step {
    Initial,
    Game(Game),
    GameOver(String),
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

                background_texture: None,
            })
        }),
    )
}

struct MyApp {
    step: Step,
    rules: Rules,
    game_name: String,
    player_name: String,

    background_texture: Option<egui::TextureHandle>,
}

impl MyApp {
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
        let Step::Game(game) = &self.step else {unreachable!();};

        let width = game.game_static.rules.width * 16;
        let height = game.game_static.rules.height * 16;
        let texture: &mut egui::TextureHandle = self.background_texture.get_or_insert_with(|| {
            ui.ctx().load_texture(
                "game_field_background",
                egui::ColorImage::new([width as usize, height as usize], egui::Color32::GRAY),
                egui::TextureOptions::default(),
            )
        });
        let width = width as f32;
        let height = height as f32;

        let game_field = ui.image(
            texture,
            egui::Vec2 {
                x: width,
                y: height,
            },
        );

        let painter = ui.painter_at(game_field.rect);

        let rect = egui::Rect::from_min_size(
            egui::Pos2 ::ZERO,
            egui::Vec2 {
                x: width,
                y: height,
            },
        );

        painter.rect_stroke(
            rect,
            egui::Rounding::none(),
            egui::Stroke {
                width: 2.0,
                color: egui::Color32::GOLD,
            },
        );
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

// fn load_image_from_memory(image_data: &[u8]) -> egui::ColorImage {
//     let image = image::load_from_memory(image_data).expect("resources can be loaded");
//     let size = [image.width() as _, image.height() as _];
//     let image_buffer = image.to_rgba8();
//     let pixels = image_buffer.as_flat_samples();
//     egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
// }
//
// fn load_tiles(ctx: &egui::Context) {
//     ctx.load_texture(
//         "cell_bomb",
//         load_image_from_memory(include_bytes!("../images/cell_bomb.bmp")),
//         egui::TextureOptions::default(),
//     );
// }
