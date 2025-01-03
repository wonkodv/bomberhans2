use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;

use eframe::egui;
use egui::pos2;
use egui::Color32;
use egui::Pos2;
use egui::Rect;
use egui::Shape;
use egui::TextureHandle;
use egui::TextureId;
use serde::Deserialize;
use serde::Serialize;

use crate::connection::connect;
use crate::connection::Connection;
use crate::game::Game;
use bomberhans_lib::field::Cell;
use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::PlayerState;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::CellPosition;
use bomberhans_lib::utils::Direction;
use bomberhans_lib::utils::Position;
use bomberhans_lib::utils::TimeStamp;
use bomberhans_lib::utils::TICKS_PER_SECOND;

const PIXEL_PER_CELL: f32 = 42.0;

enum State {
    Initial,
    SinglePlayerSettings,
    MultiPlayerConnectingToServer,
    MultiPlayerServerView,
    MultiPlayerServerGuest,
    MultiPlayerServerHost,
    Game(Game),
    GameOver(String),
    MpOpeningLobby,
}

impl State {
    fn game(&mut self) -> &mut Game {
        if let State::Game(game) = self {
            game
        } else {
            panic!("no game running");
        }
    }
}

fn cell_rect(pos: CellPosition, offset: Pos2) -> egui::Rect {
    let x = (pos.x + 1) as f32 * PIXEL_PER_CELL + offset.x;
    let y = (pos.y + 1) as f32 * PIXEL_PER_CELL + offset.y;

    Rect::from_min_max(pos2(x, y), pos2(x + PIXEL_PER_CELL, y + PIXEL_PER_CELL))
}

fn player_rect(pos: Position, offset: Pos2) -> egui::Rect {
    let x = (pos.x as f32 / Position::ACCURACY as f32 + 1.0) * PIXEL_PER_CELL + offset.x;
    let y = (pos.y as f32 / Position::ACCURACY as f32 - 0.2 + 1.0) * PIXEL_PER_CELL + offset.y;
    let p = PIXEL_PER_CELL / 2.0;

    Rect::from_min_max(pos2(x - p, y - p), pos2(x + p, y + p))
}

pub fn gui() {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(600.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native(
        &format!("Bomberhans {}", bomberhans_lib::VERSION),
        options,
        Box::new(|_cc| {
            Box::new(MyApp {
                state: State::Initial,
                app_settings: AppSettings::load(),
                textures: None,
                walking_directions: DirectionStack::new(),
                connection: None,
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
            Some(Direction::North) => "walking_n",
            Some(Direction::West) => "walking_w",
            Some(Direction::South) => "walking_s",
            Some(Direction::East) => "walking_e",
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

#[derive(Debug, Serialize, Deserialize)]
struct AppSettings {
    // TODO: When strings come after Structs, Toml Serializing fails. Ditch Confy, roll my own
    // thing !
    player_name: String,
    server: String,
    game_settings: Settings,
}

impl AppSettings {
    fn save(&self) {
        match confy::store("bomberhans2", Some("client"), self) {
            Ok(()) => log::info!("Settings stored"),
            Err(e) => log::error!("Error storing config: {e}"),
        }
    }

    fn load() -> Self {
        match confy::load("bomberhans2", Some("client")) {
            Ok(settings) => {
                log::info!("Settings loaded");
                settings
            }
            Err(e) => {
                log::error!("Error restoring settings: {e:?}");
                Self::default()
            }
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            game_settings: Settings::default(),
            player_name: String::from("Hans"),
            server: String::from("[::1]:4267"),
        }
    }
}

struct MyApp {
    state: State,
    walking_directions: DirectionStack,
    textures: Option<Rc<TextureManager>>,

    app_settings: AppSettings,

    // TODO: The following values should live in step
    connection: Option<Connection>,
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
    fn update_singleplayer_settings(&mut self, ui: &mut egui::Ui) {
        let textures = self.textures(ui.ctx());

        let settings = &mut self.app_settings.game_settings;

        ui.style_mut().spacing.slider_width = 300.0;

        if let State::GameOver(s) = &self.state {
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
                    egui::Slider::new(
                        &mut settings.bomb_explode_time_ms,
                        Settings::BOMB_TIME_RANGE,
                    )
                    .text("Bomb Time")
                    .clamp_to_range(true),
                )
                .on_hover_text("Time between placing a bomb and its explosion [ms]");
                ui.add(
                    egui::Slider::new(&mut settings.speed_base, Settings::SPEED_BASE_RANGE)
                        .text("Base Speed")
                        .clamp_to_range(false),
                )
                .on_hover_text("Speed of the Player without any upgrades [Cells/s/100]");
                ui.add(
                    egui::Slider::new(
                        &mut settings.speed_multiplyer,
                        Settings::SPEED_MULTIPLYER_RANGE,
                    )
                    .text("Speed Increase")
                    .clamp_to_range(false),
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
                    .text("Upgrade Explosion")
                    .clamp_to_range(false),
                )
                .on_hover_text("Explosion Range of ignited Powerups [cells]");
                ui.add(
                    egui::Slider::new(
                        &mut settings.wood_burn_time_ms,
                        Settings::WOOD_BURN_TIME_RANGE,
                    )
                    .text("Wood Burn Time")
                    .clamp_to_range(false),
                )
                .on_hover_text("Time that wood burns after igniting [ms]");
                ui.add(
                    egui::Slider::new(
                        &mut settings.fire_burn_time_ms,
                        Settings::FIRE_BURN_TIME_RANGE,
                    )
                    .text("Fire Burn Time")
                    .clamp_to_range(false),
                )
                .on_hover_text("Time that fire burns [ms]");
                ui.add(
                    egui::Slider::new(&mut settings.bomb_offset, Settings::BOMB_OFFSET_RANGE)
                        .text("Bomb Placement Offset")
                        .clamp_to_range(false),
                )
                .on_hover_text("While running, how far behind hans a bomb is placed [cells/100]");
            });
            ui.vertical(|ui| {
                const RATIO_RANGE: std::ops::RangeInclusive<u32> = 0..=50;
                ui.heading("Ratios of cells that burned wood will turn into");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut settings.ratios.power, RATIO_RANGE).text("Power Upgrade"),
                    );
                })
                .response
                .on_hover_text("Consuming this will upgrade the player's bomb's explosion range");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut settings.ratios.speed, RATIO_RANGE).text("Speed Upgrade"),
                    );
                })
                .response
                .on_hover_text("Consuming this will upgrade the player's walking speed");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.bombs, RATIO_RANGE).text("Bomb Upgrade"));
                })
                .response
                .on_hover_text(
                    "Consuming this will increase how many bombs the player can place simultaneously",
                );
                ui.horizontal(|ui| { ui.add(egui::Slider::new(&mut settings.ratios.teleport, RATIO_RANGE).text("Teleport")); }). response.on_hover_text("Teleport\nWalking into a teleport will move you to another TB and consume both.\nIgniting a Teleport will ignite another TP as well");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.wall, RATIO_RANGE).text("Wall"));
                })
                .response
                .on_hover_text("Wall\nIf this happens too often, you will be stuck.");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.wood, RATIO_RANGE).text("Wood"));
                })
                .response
                .on_hover_text("Wood\nYou can try and explode again");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings.ratios.clear, RATIO_RANGE).text("Empty Cell"));
                })
                .response
                .on_hover_text("Just a boring empty Cell");
            });
            ui.vertical(|ui| {
                ui.heading("effective Ratios");
                let image_dims = egui::Vec2 { x: 16.0, y: 16.0 };
                let percentages = settings.ratios.normalize();
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_upgrade_power"), image_dims);
                    ui.label(format!("{}%", percentages.power));
                })
                .response
                .on_hover_text("Consuming this will upgrade the player's bomb's explosion range");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_upgrade_speed"), image_dims);
                    ui.label(format!("{}%", percentages.speed));
                })
                .response
                .on_hover_text("Consuming this will upgrade the player's walking speed");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_upgrade_bomb"), image_dims);
                    ui.label(format!("{}%", percentages.bombs));
                })
                .response
                .on_hover_text(
                    "Consuming this will increase how many bombs the player can place simultaneously",
                );
                ui.horizontal(|ui| { ui.image(textures.get_texture("cell_teleport"), image_dims); ui.label(format!("{}%", percentages.teleport)); }). response.on_hover_text("Teleport\nWalking into a teleport will move you to another TB and consume both.\nIgniting a Teleport will ignite another TP as well");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_wall"), image_dims);
                    ui.label(format!("{}%", percentages.wall));
                })
                .response
                .on_hover_text("Wall\nIf this happens too often, you will be stuck.");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_wood"), image_dims);
                    ui.label(format!("{}%", percentages.wood));
                })
                .response
                .on_hover_text("Wood\nYou can try and explode again");
                ui.horizontal(|ui| {
                    ui.image(textures.get_texture("cell_empty"), image_dims);
                    ui.label(format!("{}%", percentages.clear));
                })
                .response
                .on_hover_text("Just a boring empty Cell");
            });
        });
        ui.horizontal(|ui| {
            if ui.button("Restore Default Settings").clicked() {
                self.app_settings.game_settings = Settings::default();
            }

            let start_button = ui.button("Start").on_hover_text("Start local game");
            {
                let mut memory = ui.memory();
                if memory.focus().is_none() {
                    memory.request_focus(start_button.id); // TODO: this flickers
                }
            }

            if start_button.clicked() {
                todo!("update settings, save");
                let game = Game::new_local_game(self.app_settings.game_settings.clone());
                self.state = State::Game(game);
                return;
            }

            if ui.button("Don't click").clicked() {
                panic!("Don't click!");
            }
        });
    }

    fn update_game(&mut self, ui: &mut egui::Ui) {
        self.update_game_inputs(ui);
        self.update_game_draw(ui);
    }

    fn update_game_inputs(&mut self, ui: &mut egui::Ui) {
        let game = self.state.game();

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
        game.set_local_player_action(Action { walking, placing });
    }

    fn update_game_draw(&mut self, ui: &mut egui::Ui) {
        let textures = self.textures(ui.ctx());

        let game_over = ui
            .horizontal(|ui| {
                ui.label(&self.state.game().settings().game_name);
                let button = ui.button("Stop Game");
                if button.clicked() {
                    self.state = State::GameOver("You pressed Stop".to_owned());
                    true
                } else {
                    false
                }
            })
            .inner;
        if game_over {
            return;
        };

        let step = &mut self.state;
        let game = step.game();

        let width = (game.settings().width + 2) as f32 * PIXEL_PER_CELL;
        let height = (game.settings().height + 2) as f32 * PIXEL_PER_CELL;

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

        painter.extend(
            game.local_state()
                .field
                .iter_with_border()
                .map(|(pos, cell)| {
                    Shape::image(
                        textures.get_cell(cell),
                        cell_rect(pos, game_field.rect.min),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    )
                }),
        );

        let time = game.local_state().time;

        painter.extend(game.local_state().player_states.values().map(|player| {
            Shape::image(
                textures.get_player(player, time),
                player_rect(player.position, game_field.rect.min),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            )
        }));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f32(
                1.0 / TICKS_PER_SECOND as f32,
            ));
    }

    fn update_initial(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::TextEdit::singleline(
            &mut self.app_settings.player_name,
        ))
        .on_hover_text("Player Name");
        ui.horizontal(|ui| {
            let local_button = ui
                .button("Single Player")
                .on_hover_text("Start a local Game without network players");

            if local_button.clicked() {
                self.app_settings.save(); // TODO: should only save game-settings?
                self.state = State::SinglePlayerSettings;
            }
        });
        ui.horizontal(|ui| {
            let server_text_edit = ui.add(egui::TextEdit::singleline(&mut self.app_settings.server));

            let connect_button = ui.button("Connect").on_hover_text("Connect to Server");
            {
                let mut memory = ui.memory();
                if memory.focus().is_none() {
                    memory.request_focus(connect_button.id); // TODO: this flickers
                }
            }


            let server = self.app_settings.server.parse::<SocketAddr>();
            match server {
                Err(err) => {
                    server_text_edit.on_hover_text(&format!("Server (name/ip) and optionally port\nFor Example:\n-   [::1]:4267\n-   bomberhans.hanstool.org\nCurrent Problem: {err:#?}"));
                    // TODO: make the textedit red
                }
                Ok(server) => {
                server_text_edit.on_hover_text(&format!("Server (name/ip) and optionally port\nFor Example:\n-   [::1]:4267\n-   bomberhans.hanstool.org\nCurrent Value: {server:#?}"));
                if connect_button.clicked() {
                    self.app_settings.save(); // TODO: should only save server

                    self.connection = Some(connect(server, self.app_settings.player_name.clone()));
                    self.state = State::MultiPlayerConnectingToServer; // TODO: connection should
                                                                     // live in step
                }
                }
            }



        });
    }

    fn update_multiplayer_view(&mut self, ui: &mut egui::Ui) {
        let connection = self.connection.as_ref().unwrap();
        if let Some(Ok((lobbies, server_info))) = connection.get_server_info() {
            ui.heading(&format!(
                "Multiplayer Games on {} ({}), Ping {:.1}",
                server_info.server_name,
                connection.server,
                server_info.ping.as_secs_f32() / 1000.0
            ));
            for (game_id, game_name) in lobbies {
                ui.horizontal(|ui| {
                    if ui.button("Join").clicked() {
                        todo!("join {game_id:?}");
                    }
                    ui.label(game_name);
                });
            }
            if ui.button("Host new Game").clicked() {
                connection.open_new_lobby();
                self.state = State::MpOpeningLobby;
            }
        };
    }

    fn update_multiplayer_guest(&self, ui: &mut egui::Ui) {
        todo!()
    }

    fn update_multiplayer_host(&self, ui: &mut egui::Ui) {
        todo!()
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bomberhans");
            match self.state {
                State::Initial => self.update_initial(ui),
                State::GameOver(_) | State::SinglePlayerSettings => {
                    self.update_singleplayer_settings(ui)
                }
                State::Game(_) => self.update_game(ui),
                State::MultiPlayerConnectingToServer => {
                    let connection = self.connection.as_ref().unwrap();
                    match connection.get_server_info() {
                        Some(Ok(server_info)) => {
                            self.state = State::MultiPlayerServerView;
                            self.update_multiplayer_view(ui);
                        }
                        Some(Err(err)) => {
                            let server = connection.server;
                            self.update_initial(ui);
                            ui.label(&format!("Error connecting to {}: {}", server, err));
                        }
                        None => {
                            ui.label(&format!(
                                "connecting to {}",
                                self.connection.as_ref().unwrap().server
                            ));
                            if ui.button("Cancel ").clicked() {
                                self.state = State::Initial;
                            }
                        }
                    }
                }
                State::MultiPlayerServerView => self.update_multiplayer_view(ui),
                State::MpOpeningLobby => {
                    ui.label(&format!("Waiting for new Lobby to open",));
                    if ui.button("Cancel ").clicked() {
                        self.state = State::Initial;
                    }
                }
                State::MultiPlayerServerGuest => self.update_multiplayer_guest(ui),
                State::MultiPlayerServerHost => self.update_multiplayer_host(ui),
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
                    load_image_from_memory(
                        include_bytes!(concat!("../../images/", $x, ".bmp")),
                        $t,
                    ),
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
