use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;

use bomberhans_lib::game_state::GameState;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::Ready;
use bomberhans_lib::network::ServerLobbyList;
use bomberhans_lib::utils::Idx as _;
use bomberhans_lib::utils::PlayerId;
use eframe::egui;
use egui::load::SizedTexture;
use egui::pos2;
use egui::Color32;
use egui::ImageSource;
use egui::Pos2;
use egui::Rect;
use egui::Shape;
use egui::TextureHandle;
use egui::TextureId;
use serde::Deserialize;
use serde::Serialize;

use crate::app::{GameController, State};
use bomberhans_lib::field::Cell;
use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::PlayerState;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::CellPosition;
use bomberhans_lib::utils::Direction;
use bomberhans_lib::utils::GameTime;
use bomberhans_lib::utils::Position;

const PIXEL_PER_CELL: f32 = 42.0;

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

pub fn gui(mut game_controller: GameController) {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 600.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(
                    &include_bytes!("../../images/icon_client.png")[..],
                )
                .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        &format!("Bomberhans {}", bomberhans_lib::VERSION),
        native_options,
        Box::new(|cc| {
            let frame = cc.egui_ctx.clone();
            game_controller.set_update_callback(Box::new(move || {
                frame.request_repaint();
            }));

            Ok(Box::new(MyApp {
                app_settings: AppSettings::load(),
                textures: None,
                walking_directions: DirectionStack::new(),
                game_controller,
            }))
        }),
    );
}

struct TextureManager {
    textures: HashMap<&'static str, ImageSource<'static>>,
}

impl TextureManager {
    fn get_texture(self: &Rc<Self>, texture: &str) -> ImageSource<'static> {
        self.textures
            .get(texture)
            .ok_or_else(|| format!("Expected {texture} to exist"))
            .unwrap()
            .clone() // cheap clone
    }

    fn get_cell(self: &Rc<Self>, cell: &Cell) -> ImageSource<'static> {
        self.get_texture(&format!("cell_{}", cell.name()))
    }

    fn get_player(self: &Rc<Self>, player: &PlayerState, time: GameTime) -> ImageSource<'static> {
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
        self.elements.retain(|x| x != &dir);
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

enum ReadOnly {
    ReadOnly,
    ReadWrite,
}

struct MyApp {
    walking_directions: DirectionStack,
    textures: Option<Rc<TextureManager>>,
    game_controller: GameController,
    app_settings: AppSettings,
}

impl MyApp {
    fn textures(&mut self, ctx: &egui::Context) -> Rc<TextureManager> {
        Rc::clone(self.textures.get_or_insert_with(|| {
            Rc::new(TextureManager {
                textures: load_tiles(ctx),
            })
        }))
    }

    /// Settings UI
    ///
    /// Return None if settings stayed the same,
    /// new Settings otherwise
    #[allow(clippy::too_many_lines)] // GUI code has to be long and ugly
    fn update_settings(
        &mut self,
        ui: &mut egui::Ui,
        settings: &Settings,
        read_only: ReadOnly,
    ) -> Option<Settings> {
        let textures = self.textures(ui.ctx());

        let mut settings_mut = settings.clone();

        ui.style_mut().spacing.slider_width = 300.0;

        ui.add(egui::TextEdit::singleline(&mut settings_mut.game_name))
            .on_hover_text("Name of the Game");

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading("Game Options");
                ui.add(
                    egui::Slider::new(&mut settings_mut.width, Settings::WIDTH_RANGE)
                        .text("Width")
                        .clamping(egui::SliderClamping::Always)
                )
                .on_hover_text("Width of the game field [cells]");
                ui.add(
                    egui::Slider::new(&mut settings_mut.height, Settings::HEIGHT_RANGE)
                        .text("Height")
                        .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text("Height of the game field [cells]");
                ui.add(
                    egui::Slider::new(&mut settings_mut.players, Settings::PLAYERS_RANGE)
                        .text("Players")
                        .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text("Number of players that can join this game");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.bomb_explode_time_ms,
                        Settings::BOMB_TIME_RANGE,
                    )
                    .text("Bomb Time")
                    .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text("Time between placing a bomb and its explosion [ms]");
                ui.add(
                    egui::Slider::new(&mut settings_mut.speed_base, Settings::SPEED_BASE_RANGE)
                        .text("Base Speed")
                        .clamping(egui::SliderClamping::Never),
                )
                .on_hover_text("Speed of the Player without any upgrades [Cells/s/100]");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.speed_multiplyer,
                        Settings::SPEED_MULTIPLYER_RANGE,
                    )
                    .text("Speed Increase")
                    .clamping(egui::SliderClamping::Never),
                )
                .on_hover_text("Player speed increase per speed powerup [Cells/s/100]");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.bomb_walking_chance,
                        Settings::BOMB_WALKING_CHANCE_RANGE,
                    )
                    .text("Bomb Walking")
                    .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text("Chance that a player can walk over a bomb in an update [%]");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.tombstone_walking_chance,
                        Settings::TOMBSTONE_WALKING_CHANCE_RANGE,
                    )
                    .text("Tombstone Walking")
                    .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text("Chance that a player can walk over a tombstone in an update [%]");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.upgrade_explosion_power,
                        Settings::UPGRADE_EXPLOSION_POWER_RANGE,
                    )
                    .text("Upgrade Explosion")
                    .clamping(egui::SliderClamping::Never),
                )
                .on_hover_text("Explosion Range of ignited Powerups [cells]");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.wood_burn_time_ms,
                        Settings::WOOD_BURN_TIME_RANGE,
                    )
                    .text("Wood Burn Time")
                    .clamping(egui::SliderClamping::Never),
                )
                .on_hover_text("Time that wood burns after igniting [ms]");
                ui.add(
                    egui::Slider::new(
                        &mut settings_mut.fire_burn_time_ms,
                        Settings::FIRE_BURN_TIME_RANGE,
                    )
                    .text("Fire Burn Time")
                    .clamping(egui::SliderClamping::Never),
                )
                .on_hover_text("Time that fire burns [ms]");
                ui.add(
                    egui::Slider::new(&mut settings_mut.bomb_offset, Settings::BOMB_OFFSET_RANGE)
                        .text("Bomb Placement Offset")
                        .clamping(egui::SliderClamping::Never),
                )
                .on_hover_text("While running, how far behind hans a bomb is placed [cells/100]");
            });
            ui.vertical(|ui| {
                const RATIO_RANGE: std::ops::RangeInclusive<u32> = 0..=50;
                ui.heading("Ratios of cells that burned wood will turn into");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut settings_mut.ratios.power, RATIO_RANGE).text("Power Upgrade"),
                    );
                })
                .response
                .on_hover_text("Consuming this will upgrade the player's bomb's explosion range");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut settings_mut.ratios.speed, RATIO_RANGE).text("Speed Upgrade"),
                    );
                })
                .response
                .on_hover_text("Consuming this will upgrade the player's walking speed");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings_mut.ratios.bombs, RATIO_RANGE).text("Bomb Upgrade"));
                })
                .response
                .on_hover_text(
                    "Consuming this will increase how many bombs the player can place simultaneously",
                );
                ui.horizontal(|ui| { ui.add(egui::Slider::new(&mut settings_mut.ratios.teleport, RATIO_RANGE).text("Teleport")); }). response.on_hover_text("Teleport\nWalking into a teleport will move you to another TB and consume both.\nIgniting a Teleport will ignite another TP as well");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings_mut.ratios.wall, RATIO_RANGE).text("Wall"));
                })
                .response
                .on_hover_text("Wall\nIf this happens too often, you will be stuck.");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings_mut.ratios.wood, RATIO_RANGE).text("Wood"));
                })
                .response
                .on_hover_text("Wood\nYou can try and explode again");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut settings_mut.ratios.clear, RATIO_RANGE).text("Empty Cell"));
                })
                .response
                .on_hover_text("Just a boring empty Cell");
            });
            ui.vertical(|ui| {
                ui.heading("effective Ratios");
                let image_dims = egui::Vec2 { x: 16.0, y: 16.0 };
                let percentages = settings_mut.ratios.normalize();
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
        if settings == &settings_mut {
            None
        } else {
            Some(settings_mut)
        }
    }

    fn update_game(&mut self, ui: &mut egui::Ui, game_state: &GameState) {
        self.update_game_inputs(ui);
        self.update_game_draw(ui, game_state);
    }

    fn update_game_inputs(&mut self, ui: &mut egui::Ui) {
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
        self.game_controller.set_action(Action { walking, placing });
    }

    fn update_game_draw(&mut self, ui: &mut egui::Ui, game_state: &GameState) {
        let textures = self.textures(ui.ctx());

        let width = (game_state.settings.width + 2) as f32 * PIXEL_PER_CELL;
        let height = (game_state.settings.height + 2) as f32 * PIXEL_PER_CELL;

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

        painter.extend(game_state.field.iter_with_border().map(|(pos, cell)| {
            Shape::image(
                textures.get_cell(cell),
                cell_rect(pos, game_field.rect.min),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            )
        }));

        let time = game_state.time;

        painter.extend(game_state.players.values().map(|(player, state)| {
            Shape::image(
                textures.get_player(state, time),
                player_rect(state.position, game_field.rect.min),
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            )
        }));
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
                // self.app_settings.save(); // TODO: should only save game-settings?
                self.game_controller.start_local_game();
            }
        });
        ui.horizontal(|ui| {
            let server_text_edit = ui.add(egui::TextEdit::singleline(&mut self.app_settings.server));

            let connect_button = ui.button("Connect").on_hover_text("Connect to Server");
            {
                let mut memory = ui.memory();
                if memory.focus().is_none() {
                    memory.request_focus(connect_button.id);
                }
            }


            let server = self.app_settings.server.parse::<SocketAddr>();
            match server {
                Err(err) => {
                    server_text_edit.on_hover_text(format!("Server (name/ip) and optionally port\nFor Example:\n-   [::1]:4267\n-   bomberhans.hanstool.org\nCurrent Problem: {err:#?}"));
                    // TODO: make the textedit red
                }
                Ok(server) => {
                server_text_edit.on_hover_text(format!("Server (name/ip) and optionally port\nFor Example:\n-   [::1]:4267\n-   bomberhans.hanstool.org\nCurrent Value: {server:#?}"));
                if connect_button.clicked() {
                    self.app_settings.save(); // TODO: should only save server

                    self.game_controller.connect_to_server(server);

                }
                }
            }



        });
    }

    fn update_multiplayer_view(&mut self, ui: &mut egui::Ui, server_info: &ServerLobbyList) {
        ui.heading(format!("Multiplayer Games on {}", server_info.server_name,));
        let button = ui.button("Host new Game");
        {
            let mut memory = ui.memory();
            if memory.focus().is_none() {
                memory.request_focus(button.id);
            }
        }
        if button.clicked() {
            self.game_controller
                .open_new_lobby(self.app_settings.player_name.clone());
        }
        if ui.button("Cancel").clicked() {
            self.game_controller.disconnect();
        }

        for (game_id, game_name) in &server_info.lobbies {
            ui.horizontal(|ui| {
                if ui.button("Join").clicked() {
                    self.game_controller
                        .join_lobby(*game_id, self.app_settings.player_name.clone());
                }
                ui.label(game_name);
            });
        }
    }

    fn update_multiplayer(
        &mut self,
        ui: &mut egui::Ui,
        settings: &Settings,
        players: &Vec<Player>,
        players_ready: &Vec<Ready>,
        local_player_id: &PlayerId,
        host: bool,
    ) {
        if host {
            ui.heading(format!("Hosting Multiplayer Game {}", settings.game_name));
            if let Some(new_settings) = self.update_settings(ui, &settings, ReadOnly::ReadWrite) {
                self.game_controller.update_settings(new_settings);
            }
        } else {
            ui.heading(format!("Guest in Multiplayer Game {}", settings.game_name));
            ui.label(format!("{:?}", settings));
        }

        if let Ready::NotReady = players_ready[local_player_id.idx()] {
            let button = ui.button("Ready");
            {
                let mut memory = ui.memory();
                if memory.focus().is_none() {
                    memory.request_focus(button.id);
                }
            }
            if button.clicked() {
                self.game_controller.set_ready(Ready::Ready);
            }
        } else {
            if ui.button("NotReady").clicked() {
                self.game_controller.set_ready(Ready::NotReady);
            }
        }
        if ui.button("Cancel").clicked() {
            self.game_controller.disconnect();
        }

        for (player, ready) in players.iter().zip(players_ready) {
            ui.horizontal(|ui| {
                ui.label(format!("{}", player.id.0));
                ui.label(format!("{}", player.name));
                ui.label(format!("{:?}", ready));
            });
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let state = self.game_controller.get_state();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bomberhans");
            match &state {
                State::Initial => self.update_initial(ui),
                State::SpSettings => {
                    //      self.app_settings.game_settings =
                    //          self.update_settings(ui, self.app_settings.game_settings.clone(), false);
                    ui.horizontal(|ui| {
                        //          if ui.button("Restore Default Settings").clicked() {
                        //              self.app_settings.game_settings = Settings::default();
                        //          }

                        let start_button = ui.button("Start").on_hover_text("Start local game");
                        default_focus(ctx, &start_button);

                        if start_button.clicked() {
                            //              self.app_settings.save();
                            //              self.app_settings.game_settings.clone()
                            self.game_controller.start_local_game();
                        }

                        assert!(!ui.button("Don't click").clicked(), "Don't click!");
                    });
                }
                State::SpGame(game) => {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Local Game: {}",
                            &game.game_state().settings.game_name
                        ));
                    });
                    self.update_game(ui, game.game_state());
                }
                State::MpConnecting => {
                    ui.label("connecting to server".to_owned());
                    if ui.button("Cancel ").clicked() {
                        self.game_controller.disconnect();
                    }
                }
                State::MpView(server_info) => self.update_multiplayer_view(ui, &server_info),
                State::MpOpeningNewLobby => {
                    ui.label("Waiting for new Lobby to open".to_owned());
                    if ui.button("Cancel ").clicked() {
                        self.game_controller.disconnect();
                    }
                }

                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                } => {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Multiplayer Game: {}",
                            local_game_state.settings.game_name
                        ));
                    });
                    self.update_game(ui, &local_game_state);
                }
                State::MpServerLost(game) => {
                    ui.label("Server not responding".to_owned());
                    if ui.button("Cancel ").clicked() {
                        self.game_controller.disconnect();
                    }
                }
                State::Disconnected(reason) => {
                    ui.label(format!("Server disconnected {reason}"));
                    if ui.button("Ack ").clicked() {
                        self.game_controller.disconnect();
                    }
                }
                State::GuiClosed => {
                    panic!("Controller assumes Gui closed, but we are trying to draw that")
                }
                State::Invalid => panic!(),
                State::MpLobby {
                    host: false,
                    settings,
                    players,
                    players_ready,
                    local_player_id,
                } => {
                    self.update_multiplayer(
                        ui,
                        settings,
                        players,
                        players_ready,
                        local_player_id,
                        false,
                    );
                }
                State::MpLobby {
                    host: true,
                    settings,
                    players,
                    players_ready,
                    local_player_id,
                } => {
                    self.update_multiplayer(
                        ui,
                        settings,
                        players,
                        players_ready,
                        local_player_id,
                        true,
                    );
                }
                State::MpJoiningLobby { game_id } => {
                    ui.label("Joining Lobby".to_owned());
                    if ui.button("Cancel").clicked() {
                        self.game_controller.disconnect();
                    }
                }
            };
        });
    }
}

fn default_focus(ctx: &egui::Context, start_button: &egui::Response) {
    ctx.memory_mut(|memory| {
        if memory.focused().is_none() {
            memory.request_focus(start_button.id);
        }
    });
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

fn load_tiles(ctx: &egui::Context) -> HashMap<&'static str, ImageSource<'static>> {
    let mut map = HashMap::new();

    macro_rules! load {
        ($x:expr, $t:expr) => {
            let image =
                load_image_from_memory(include_bytes!(concat!("../../images/", $x, ".bmp")), $t);
            map.insert(
                $x,
                ImageSource::Texture(SizedTexture {
                    id: ctx.load_texture($x, image, egui::TextureOptions::default()),
                    size: image.size,
                }),
            );
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
