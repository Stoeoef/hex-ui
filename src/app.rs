use eframe::{
    egui::{
        self, Color32, Pos2, Response, Sense, Shape, Slider, Stroke, Ui, Vec2, Visuals, Widget,
    },
    epi,
};
use hexgame::{Color, Coords, Game, Status};
use hexgame_ai::{HexNodeContent, MctsHexGame};
use mcts::{
    action_decision::SelectRobustChild, full_expansion::FullExpansion,
    shuffled_playout::ShuffledPlayout, time_control::ConstIterationCount,
    uct_selection::UctSelection, uct_update::UctUpdate, Game as _, Mcts, NodeContent,
};
use rand::{prelude::SmallRng, SeedableRng};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct HexGameUi {
    #[cfg_attr(feature = "persistence", serde(skip))]
    game: MctsHexGame,
    configured_size: u8,
}

impl Default for HexGameUi {
    fn default() -> Self {
        Self {
            game: MctsHexGame::new(5, 0, 1),
            configured_size: 5,
        }
    }
}

const HEX_SIZE: f32 = 40.0;

const fn hex_coords() -> [Vec2; 6] {
    [
        Vec2::new(0.00000, 1.00000),
        Vec2::new(0.86603, 0.50000),
        Vec2::new(0.86603, -0.50000),
        Vec2::new(0.00000, -1.00000),
        Vec2::new(-0.86603, -0.50000),
        Vec2::new(-0.86603, 0.50000),
    ]
}

impl epi::App for HexGameUi {
    fn name(&self) -> &str {
        "eframe template"
    }

    /// Called once before the first frame.
    fn setup(
        &mut self,
        ctx: &egui::CtxRef,
        _frame: &epi::Frame,
        _storage: Option<&dyn epi::Storage>,
    ) {
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        #[cfg(feature = "persistence")]
        if let Some(storage) = _storage {
            *self = epi::get_value(storage, epi::APP_KEY).unwrap_or_default()
        }

        ctx.set_visuals(Visuals::dark());
    }

    /// Called by the frame work to save state before shutdown.
    /// Note that you must enable the `persistence` feature for this to work.
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn epi::Storage) {
        epi::set_value(storage, epi::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::CtxRef, _: &epi::Frame) {
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Options");

            ui.horizontal(|ui| {
                ui.label("Field Size: ");
                ui.add(Slider::new(&mut self.configured_size, 0..=17));
            });

            if ui.button("Reset game...").clicked() {
                self.game = MctsHexGame::new(self.configured_size, 0, 1);
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to("eframe", "https://github.com/emilk/egui/tree/master/eframe");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(HexWidget {
                game: &mut self.game,
            })
        });
    }
}

fn get_hex_shape(pos: Pos2) -> Vec<Pos2> {
    let factor = 1.1;
    hex_coords()
        .into_iter()
        .map(|offset| pos + offset * (HEX_SIZE * 0.5 * factor))
        .collect()
}

struct HexWidget<'a> {
    game: &'a mut MctsHexGame,
}

fn player_to_color(player: Color) -> Color32 {
    match player {
        Color::Black => Color32::RED,
        Color::White => Color32::BLUE,
    }
}

impl<'a> Widget for HexWidget<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        match self.game().status {
            Status::Ongoing => self.draw_game(ui),
            Status::Finished(player) => self.draw_victory_screen(ui, player),
        }
    }
}

impl<'a> HexWidget<'a> {
    fn game(&mut self) -> &mut Game {
        &mut self.game.game
    }

    fn draw_victory_screen(self, ui: &mut Ui, player: Color) -> Response {
        let text = match player {
            Color::Black => "Player Red wins!",
            Color::White => "Player Blue wins!",
        };
        ui.heading(text)
    }

    fn draw_game(mut self, ui: &mut Ui) -> Response {
        let board = &self.game().board;
        let size = board.size();

        let base_size = size as f32 * HEX_SIZE;
        let response = ui.allocate_response(
            Vec2::new(base_size * 1.5, base_size),
            Sense::click_and_drag(),
        );
        let rect = response.rect;
        let painter = ui.painter_at(rect);
        let mut base_offset = rect.left_top();
        base_offset.x += HEX_SIZE;
        base_offset.y += HEX_SIZE;

        let mut closest_coord = None;
        let mut closest_distance = f32::MAX;

        let pointer = &ui.input().pointer;

        let pos = |x, y| {
            let x = HEX_SIZE * x as f32 + y as f32 * HEX_SIZE * 0.5;
            let y = HEX_SIZE * y as f32 * 0.87;
            base_offset + Vec2::new(x, y)
        };

        for x in 0..size {
            for y in 0..size {
                if let Some(cursor_pos) = pointer.hover_pos() {
                    let selection_range_sq = HEX_SIZE * HEX_SIZE;
                    let distance_sq = cursor_pos.distance_sq(pos(x, y));
                    let is_within_selection_range =
                        cursor_pos.distance_sq(pos(x, y)) < selection_range_sq;
                    let is_closest = distance_sq < closest_distance;
                    if is_within_selection_range && is_closest {
                        closest_distance = distance_sq;
                        closest_coord = Some((x, y))
                    }
                }
            }
        }

        for x in 0..size {
            for y in 0..size {
                let color = match board.get_color(Coords::new(x, y)) {
                    Some(Color::Black) => Color32::RED,
                    Some(Color::White) => Color32::BLUE,
                    None => Color32::LIGHT_GRAY,
                };

                let hex_shape = get_hex_shape(pos(x, y));
                let default_stroke = Stroke::new(1.0, Color32::DARK_GRAY);
                let line = Shape::convex_polygon(hex_shape, color, default_stroke);
                painter.add(line);
            }
        }

        if let Some((x, y)) = closest_coord {
            if board.get_color(Coords::new(x, y)).is_none() {
                let player_color = player_to_color(self.game().current_player);
                let hex_shape = get_hex_shape(pos(x, y));
                let line = Shape::closed_line(hex_shape, Stroke::new(4.0, player_color));
                painter.add(line);
            }

            if response.clicked() {
                self.game.play(Coords::new(x, y)).ok();

                if self.game.get_winner().is_none() {
                    let mcts: Mcts<
                        MctsHexGame,
                        HexNodeContent,
                        SmallRng,
                        UctSelection,
                        FullExpansion,
                        ShuffledPlayout,
                        UctUpdate,
                        SelectRobustChild,
                        ConstIterationCount,
                    > = Mcts::new(
                        UctSelection {
                            exploration_parameter: 0.5,
                        },
                        FullExpansion,
                        ShuffledPlayout,
                        UctUpdate,
                        SelectRobustChild,
                        ConstIterationCount::new(10),
                    );

                    let mut rng = SmallRng::seed_from_u64(123467123321);
                    let result = mcts.suggest_action(self.game, &mut rng);
                    let action = result
                        .tree
                        .get_content(result.node_id.expect("AI did not set node ID"))
                        .get_action()
                        .expect("Failed to retrieve a valid action");

                    self.game.play(action).expect("Failed to play AI move");
                }
            }
        }

        response
    }
}
