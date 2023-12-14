use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use eframe::egui;
use egui::pos2;
use egui::Color32;
use egui::Pos2;
use egui::Rect;
use egui::Shape;
use egui::TextureHandle;
use egui::TextureId;

use crate::game::Action;
use crate::game::Cell;
use crate::game::CellPosition;
use crate::game::Direction;
use crate::game::Game;
use crate::game::PlayerState;
use crate::game::Position;
use crate::game::State;
use crate::game::TimeStamp;
use crate::game::TICKS_PER_SECOND;
use crate::settings::Settings;

const PIXEL_PER_CELL: f32 = 42.0;

enum Step {
    Initial,
    Game(State),
    GameOver(String),
}

impl Step {
    fn game_state(&mut self) -> &mut State {
        if let Step::Game(ref mut state) = *self {
            state
        } else {
            panic!("no game running");
        }
    }
}

fn cell_rect(pos: CellPosition, offset: Pos2) -> egui::Rect {
    let x = pos.x as f32 * PIXEL_PER_CELL + offset.x;
    let y = pos.y as f32 * PIXEL_PER_CELL + offset.y;

    Rect::from_min_max(pos2(x, y), pos2(x + PIXEL_PER_CELL, y + PIXEL_PER_CELL))
}

fn player_rect(pos: Position, offset: Pos2) -> egui::Rect {
    let x = pos.x as f32 / Position::ACCURACY as f32 * PIXEL_PER_CELL + offset.x;
    let y = (pos.y as f32 / Position::ACCURACY as f32 - 0.2) * PIXEL_PER_CELL + offset.y;
    let p = PIXEL_PER_CELL / 2.0;

    Rect::from_min_max(pos2(x - p, y - p), pos2(x + p, y + p))
}

pub fn gui() {
    let settings: Settings = match confy::load("bomberhans2", Some("new_game_settings")) {
        Ok(settings) => {
            log::info!("Settings stored");
            settings
        }
        Err(e) => {
            log::error!("Error storing config: {e}");
            Settings::default()
        }
    };

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(600.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native(
        concat!("Bomberhans ", env!("VERSION")),
        options,
        Box::new(|_cc| {
            Box::new(MyApp {
                step: Step::Initial,
                settings,
                player_name: "New Player".into(),
                textures: None,
                last_frame: Instant::now(),
                walking_directions: DirectionStack::new(),
            })
        }),
    );
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

    fn get_player(self: &Rc<Self>, player: &PlayerState, time: TimeStamp) -> TextureId {
        let odd = if time.ticks_from_start() / 15 % 2 == 0 {
            "2"
        } else {
            ""
        };

        let s = match player.action.walking {
            Some(crate::game::Direction::North) => "walking_n",
            Some(crate::game::Direction::West) => "walking_w",
            Some(crate::game::Direction::South) => "walking_s",
            Some(crate::game::Direction::East) => "walking_e",
            None if player.action.placing => "placing",
            _ => "standing",
        };
        self.get_texture(&format!("hans_{s}{odd}"))
    }
}

struct DirectionStack {
    elements: Vec<Direction>,
}

impl DirectionStack {
    pub fn new() -> DirectionStack {
        Self {
            elements: Vec::new(),
        }
    }
    pub fn push(&mut self, dir: Direction) {
        if !self.elements.contains(&dir) {
            self.elements.push(dir);
        }
    }

    pub fn remove(&mut self, dir: Direction) {
        self.elements.remove(
            self.elements
                .iter()
                .position(|x| x == &dir)
                .expect("removing key that was added"),
        );
    }

    pub fn get(&self) -> Option<Direction> {
        self.elements.last().copied()
    }
}

struct MyApp {
    step: Step,
    settings: Settings,
    player_name: String,

    walking_directions: DirectionStack,

    textures: Option<Rc<TextureManager>>,
    last_frame: Instant,
}

impl MyApp {
    fn textures(&mut self, ctx: &egui::Context) -> Rc<TextureManager> {
        Rc::clone(self.textures.get_or_insert_with(|| {
            Rc::new(TextureManager {
                textures: load_tiles(ctx),
            })
        }))
    }

    #[allow(clippy::too_many_lines)] // GUI code has to be long and ugly
    fn update_initial(&mut self, ui: &mut egui::Ui) {
        let textures = self.textures(ui.ctx());

        let settings = &mut self.settings;

        ui.style_mut().spacing.slider_width = 300.0;

        if let Step::GameOver(ref s) = self.step {
            ui.label(format!("GameOver: {s}"));
        }
        ui.add(egui::TextEdit::singleline(&mut settings.game_name))
            .on_hover_text("Name of the Game");

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading("Game Options");
                ui.add(
                    egui::Slider::new(&mut settings.width, Settings::WIDTH_RANGE)
                        .text("Width")
                        .clamp_to_range(true),
                )
                .on_hover_text("Width of the game field [cells]");
                ui.add(
                    egui::Slider::new(&mut settings.height, Settings::HEIGHT_RANGE)
                        .text("Height")
                        .clamp_to_range(true),
                )
                .on_hover_text("Height of the game field [cells]");
                ui.add(
                    egui::Slider::new(&mut settings.players, Settings::PLAYERS_RANGE)
                        .text("Players")
                        .clamp_to_range(true),
                )
                .on_hover_text("Number of players that can join this game");
                ui.add(
                    egui::Slider::new(&mut settings.bomb_explode_time_ms, Settings::BOMB_TIME_RANGE)
                        .text("Bomb Time")
                        .clamp_to_range(true),
                )
                .on_hover_text("Time between placing a bomb and its explosion [ms]");
                ui.add(
                    egui::Slider::new(&mut settings.speed_base, Settings::SPEED_BASE_RANGE)
                        .text("Base Speed"),
                )
                .on_hover_text("Speed of the Player without any upgrades [Cells/s/100]");
                ui.add(
                    egui::Slider::new(&mut settings.speed_multiplyer, Settings::SPEED_MULTIPLYER_RANGE)
                        .text("Speed Increase"),
                )
                .on_hover_text("Player speed increase per speed powerup [Cells/s/100]");
                ui.add(
                    egui::Slider::new(
                        &mut settings.bomb_walking_chance,
                        Settings::BOMB_WALKING_CHANCE_RANGE,
                    )
                    .text("Bomb Walking")
                    .clamp_to_range(true),
                )
                .on_hover_text("Chance that a player can walk over a bomb in an update [%]");
                ui.add(
                    egui::Slider::new(
                        &mut settings.tombstone_walking_chance,
                        Settings::TOMBSTONE_WALKING_CHANCE_RANGE,
                    )
                    .text("Tombstone Walking")
                    .clamp_to_range(true),
                )
                .on_hover_text("Chance that a player can walk over a tombstone in an update [%]");
                ui.add(
                    egui::Slider::new(
                        &mut settings.upgrade_explosion_power,
                        Settings::UPGRADE_EXPLOSION_POWER_RANGE,
                    )
                    .text("Upgrade Explosion"),
                )
                .on_hover_text("Explosion Range of ignited Powerups [cells]");
                ui.add(
                    egui::Slider::new(&mut settings.wood_burn_time_ms, Settings::WOOD_BURN_TIME_RANGE)
                        .text("Wood Burn Time"),
                )
                .on_hover_text("Time that wood burns after igniting [ms]");
                ui.add(
                    egui::Slider::new(&mut settings.fire_burn_time_ms, Settings::FIRE_BURN_TIME_RANGE)
                        .text("Fire Burn Time"),
                )
                .on_hover_text("Time that fire burns [ms]");
                ui.add(
                    egui::Slider::new(&mut settings.bomb_offset, Settings::BOMB_OFFSET_RANGE)
                        .text("Bomb Placement Offset"),
                )
                .on_hover_text("While running, how far behind hans a bomb is placed [cells/100]");
            });

            ui.vertical(|ui| {
                const RATIO_RANGE: std::ops::RangeInclusive<u32> =0..=50;

                ui.heading("Ratios of cells that burned wood will turn into");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut settings.ratios.power, RATIO_RANGE).text("Power Upgrade"),
                    );
                }). response.on_hover_text("Consuming this will upgrade the player's bomb's explosion range");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut settings.ratios.speed, RATIO_RANGE).text("Speed Upgrade"),
                    );
                }). response.on_hover_text("Consuming this will upgrade the player's walking speed");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.bombs, RATIO_RANGE).text("Bomb Upgrade"));
                }). response.on_hover_text("Consuming this will increase how many bombs the player can place simultaneously");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.teleport, RATIO_RANGE).text("Teleport"));
                }). response.on_hover_text("Teleport\nWalking into a teleport will move you to another TB and consume both.\nIgniting a Teleport will ignite another TP as well");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.wall, RATIO_RANGE).text("Wall"));
                }). response.on_hover_text("Wall\nIf this happens too often, you will be stuck.");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.wood, RATIO_RANGE).text("Wood"));
                }). response.on_hover_text("Wood\nYou can try and explode again");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.clear, RATIO_RANGE).text("Empty Cell"));
                }). response.on_hover_text("Just a boring empty Cell");
            });


            ui.vertical(|ui| {
                ui.heading("effective Ratios");
                let image_dims = egui::Vec2 { x: 16.0, y: 16.0 };
                let percentages = settings.ratios.normalize();

                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_upgrade_power"), image_dims);
                    ui.label(format!("{}%", percentages.power));
                }). response.on_hover_text("Consuming this will upgrade the player's bomb's explosion range");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_upgrade_speed"), image_dims);
                    ui.label(format!("{}%", percentages.speed));
                }). response.on_hover_text("Consuming this will upgrade the player's walking speed");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_upgrade_bomb"), image_dims);
                    ui.label(format!("{}%", percentages.bombs));
                }). response.on_hover_text("Consuming this will increase how many bombs the player can place simultaneously");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_teleport"), image_dims);
                    ui.label(format!("{}%", percentages.teleport));
                }). response.on_hover_text("Teleport\nWalking into a teleport will move you to another TB and consume both.\nIgniting a Teleport will ignite another TP as well");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_wall"), image_dims);
                    ui.label(format!("{}%", percentages.wall));
                }). response.on_hover_text("Wall\nIf this happens too often, you will be stuck.");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_wood"), image_dims);
                    ui.label(format!("{}%", percentages.wood));
                }). response.on_hover_text("Wood\nYou can try and explode again");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_empty"), image_dims);
                    ui.label(format!("{}%", percentages.clear));
                }). response.on_hover_text("Just a boring empty Cell");
            });


        });

        ui.horizontal(|ui| {
            if ui.button("Restore Default Settings").clicked() {
                self.settings = Settings::default();
            }

            let start_button = ui
                .button("Start local Game")
                .on_hover_text("Start a local Game without network players");
            {
                let mut memory = ui.memory();
                if memory.focus().is_none() {
                    memory.request_focus(start_button.id); // TODO: this flickers
                }
            }

            if start_button.clicked() {
                match confy::store("bomberhans2", Some("new_game_settings"), &self.settings) {
                    Ok(_) => log::info!("Settings stored"),
                    Err(e) => log::error!("Error storing config: {e}"),
                }

                let game = Game::new_local_game(self.settings.clone());
                let game = Rc::new(game);
                let game_state = State::new(game);
                self.step = Step::Game(game_state);
                return;
            }

            if ui.button("Don't Panic!").clicked() {
                panic!("why would you?");
            }
        });
    }

    fn update_game(&mut self, ui: &mut egui::Ui) {
        self.update_game_simulation();
        self.update_game_inputs(ui);
        self.update_game_draw(ui);
    }

    fn update_game_simulation(&mut self) {
        let game_state = self.step.game_state();

        let now = Instant::now();
        let duration = now - self.last_frame;
        self.last_frame = now;
        let ticks = (duration.as_secs_f32() * TICKS_PER_SECOND as f32).round() as u32;

        for _ in 0..ticks {
            game_state.update();
        }
    }
    fn update_game_inputs(&mut self, ui: &mut egui::Ui) {
        let game_state = self.step.game_state();

        for (key, direction) in [
            (egui::Key::W, Direction::North),
            (egui::Key::S, Direction::South),
            (egui::Key::A, Direction::West),
            (egui::Key::D, Direction::East),
        ] {
            if ui.ctx().input_mut().key_pressed(key) {
                self.walking_directions.push(direction);
            }
            if ui.ctx().input_mut().key_released(key) {
                self.walking_directions.remove(direction);
            }
        }

        let placing = ui.ctx().input_mut().key_down(egui::Key::Space);
        let walking = self.walking_directions.get();
        game_state.set_player_action(game_state.game.local_player, Action { walking, placing });
    }

    fn update_game_draw(&mut self, ui: &mut egui::Ui) {
        let textures = self.textures(ui.ctx());

        let game_over = ui
            .horizontal(|ui| {
                ui.label(&self.step.game_state().game.settings.game_name);
                let button = ui.button("Stop Game");
                if button.clicked() {
                    self.step = Step::GameOver("You pressed Stop".to_owned());
                    true
                } else {
                    false
                }
            })
            .inner;
        if game_over {
            return;
        };

        let step = &mut self.step;
        let game_state = step.game_state();

        let width = game_state.game.settings.width as f32 * PIXEL_PER_CELL;
        let height = game_state.game.settings.height as f32 * PIXEL_PER_CELL;

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

        painter.extend(game_state.field.iter().map(|(pos, cell)| {
            Shape::image(
                textures.get_cell(cell),
                cell_rect(pos, game_field.rect.min),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            )
        }));

        let time = game_state.time;

        painter.extend(game_state.player_states.iter().map(|player| {
            Shape::image(
                textures.get_player(player, time),
                player_rect(player.position, game_field.rect.min),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            )
        }));
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f32(1.0 / TICKS_PER_SECOND as f32));
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bomberhans");
            match self.step {
                Step::GameOver(_) | Step::Initial => self.update_initial(ui),
                Step::Game(_) => self.update_game(ui),
            }
        });
        if !frame.is_web() {
            egui::gui_zoom::zoom_with_keyboard_shortcuts(ctx, frame.info().native_pixels_per_point);
        }
    }
}

/// Create an image from byte slice
///
/// `image_data` the image bytes (e.g. a Bitmap)
/// `transparent` turn all pixels with the same color as the top left corenr transparent
fn load_image_from_memory(image_data: &[u8], transparent: bool) -> egui::ColorImage {
    let image = image::load_from_memory(image_data).expect("resources can be loaded");
    let size = [image.width() as _, image.height() as _];
    let mut image_buffer = image.to_rgba8();
    let top_left = image_buffer[(0, 0)];
    if transparent {
        for pixel in image_buffer.pixels_mut() {
            if *pixel == top_left {
                pixel[3] = 0;
            }
        }
    }
    let pixels = image_buffer.as_flat_samples();
    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
}

fn load_tiles(ctx: &egui::Context) -> HashMap<&'static str, TextureHandle> {
    let mut map = HashMap::new();

    macro_rules! load {
        ($x:expr, $t:expr) => {
            map.insert(
                $x,
                ctx.load_texture(
                    $x,
                    load_image_from_memory(include_bytes!(concat!("../images/", $x, ".bmp")), $t),
                    egui::TextureOptions::default(),
                ),
            )
        };
    }

    load!("cell_bomb", false);
    load!("cell_empty", false);
    load!("cell_fire", false);
    load!("cell_start_point", false);
    load!("cell_teleport", false);
    load!("cell_tomb_stone", false);
    load!("cell_upgrade_speed", false);
    load!("cell_upgrade_bomb", false);
    load!("cell_upgrade_power", false);
    load!("cell_wall", false);
    load!("cell_wood", false);
    load!("cell_wood_burning", false);

    load!("hans_placing", true);
    load!("hans_placing2", true);
    load!("hans_standing", true);
    load!("hans_standing2", true);
    load!("hans_walking_e2", true);
    load!("hans_walking_e", true);
    load!("hans_walking_n2", true);
    load!("hans_walking_n", true);
    load!("hans_walking_s2", true);
    load!("hans_walking_s", true);
    load!("hans_walking_w2", true);
    load!("hans_walking_w", true);

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
