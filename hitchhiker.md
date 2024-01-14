
The Hitchhiker guide to Bomberhans
==================================

Last updated for [v0.2.18](https://github.com/wonkodv/bomberhans2/tree/v0.2.18)

*   [`game::GameState`](https://github.com/wonkodv/bomberhans2/blob/v0.2.18/src/game.rs#L148): Current State of the game (time, players, field) and methods to update it. Updates happen 60 times per second
*   [`setting::Settings`](https://github.com/wonkodv/bomberhans2/blob/v0.2.18/src/game.rs#L122): Adjustable rules of the game (like bomb explosion power)
*   [`field::Field`](https://github.com/wonkodv/bomberhans2/blob/v0.2.18/src/field.rs#L133): current state of the field (cell 4/3 is a bomb, placed by player 1, it will explode when game time reaches 4267). There is a string
    representation of the board used for unittests
*   [`gui::MyApp::update()`](https://github.com/wonkodv/bomberhans2/blob/v0.2.18/src/gui.rs#L472): egui's update function draws the gui every frame. MyApp holds a `gui::Step`, which (while running) holds a `game::Game` which (in
    single player mode) holds the `game::GameState` which has the moving parts.

