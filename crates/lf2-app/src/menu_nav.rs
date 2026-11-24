//! Controllers on the title screens, which the original works only with the mouse:
//! the direction pad or the stick moves the mouse from one place the screen can click
//! (`World::menu_targets`) to the nearest one in that direction, button 0 (bottom) clicks
//! it, and button 1 (right) clicks the screen's back or cancel button. Moving the real
//! mouse, or touching the screen, hands the mouse back.
//!
//! A click comes when the button is let go, so that the press does not reach the screen
//! the click opens (the mode menu after "game start" reads the same button as attack).

use lf2_sim::input::Joystick;
use lf2_sim::menu::main_menu::{MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use lf2_sim::menu::MenuTarget;
use lf2_sim::World;

#[derive(Default)]
pub struct MenuNav {
    /// The target the mouse is on, while the controller has the mouse.
    selected: Option<usize>,
    /// The controller has the mouse.
    active: bool,
    /// What the controllers pressed in the last call.
    held: Joystick,
    /// Buttons 0 and 1 went down while this screen offered targets.
    armed: [bool; 2],
    /// The title screen (`SCREEN`) the targets belong to.
    screen: i32,
    /// A click was written in the last call; it is let go in this one.
    clicked: bool,
}

/// The target nearest to `from` in the direction (dx, dy), each -1, 0 or 1: of the
/// targets whose centre lies that way, the one with the smallest distance along the
/// direction plus twice the distance across it.
fn next(targets: &[MenuTarget], from: usize, (dx, dy): (i32, i32)) -> usize {
    let (fx, fy) = targets[from].centre();
    targets
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let (x, y) = t.centre();
            let (along, across) = ((x - fx) * dx + (y - fy) * dy, ((x - fx) * dy - (y - fy) * dx).abs());
            (i != from && along > 0).then_some((along + 2 * across, i))
        })
        .min()
        .map_or(from, |(_, i)| i)
}

impl MenuNav {
    /// Whether the controller has the mouse.
    pub fn active(&self) -> bool {
        self.active && self.selected.is_some()
    }

    /// Where the menus' cursor is, while the controller has the mouse.
    pub fn cursor(&self, w: &World) -> Option<(i32, i32)> {
        let t = w.menu_targets.get(self.selected?)?;
        self.active.then(|| t.centre())
    }

    /// Before each tick, after the platform wrote the mouse: `pads` is what the
    /// controllers press, `mouse_used` whether the real mouse or a touch moved or
    /// clicked since the last call. The targets are the ones the last tick offered.
    pub fn update(&mut self, w: &mut World, pads: &[Joystick], mouse_used: bool) {
        if std::mem::take(&mut self.clicked) {
            w.g.m.set_i32(MOUSE_CLICK, 0);
        }
        let now = pads.iter().fold(Joystick::default(), |a, &b| crate::gamepad::merge(a, b));
        let pressed = |n: bool, o: bool| n && !o;
        let held = std::mem::replace(&mut self.held, now);
        let buttons = [(now.buttons[0], held.buttons[0]), (now.buttons[1], held.buttons[1])];
        let in_title = w.g.m.i32(lf2_sim::world::GAME) == 0;
        let screen = w.g.m.i32(lf2_sim::menu::title::SCREEN);
        if mouse_used {
            self.active = false;
            self.selected = None;
        }
        if !in_title {
            self.selected = None;
        }
        if !in_title || w.menu_targets.is_empty() {
            // nothing to click (a match, the mode menu, or a key slot waiting for a
            // button): presses that start here do not click when they end
            self.armed = [false; 2];
            return;
        }
        if screen != self.screen {
            self.screen = screen;
            self.selected = self.active.then_some(0);
        }
        let n = w.menu_targets.len();
        if self.selected.is_some_and(|i| i >= n) {
            self.selected = Some(0);
        }
        let dir = [(pressed(now.up, held.up), (0, -1)), (pressed(now.down, held.down), (0, 1)), (pressed(now.left, held.left), (-1, 0)), (pressed(now.right, held.right), (1, 0))];
        for (on, d) in dir {
            if on {
                self.selected = Some(match (self.active, self.selected) {
                    (true, Some(i)) => next(&w.menu_targets, i, d),
                    _ => 0,
                });
                self.active = true;
            }
        }
        let mut click = None;
        for (k, (now, before)) in buttons.into_iter().enumerate() {
            if now && !before {
                self.armed[k] = true;
            }
            if !now && before && std::mem::take(&mut self.armed[k]) {
                click = Some(k);
            }
        }
        match click {
            Some(0) if self.active() => self.clicked = true,
            Some(0) => {
                // the first press only takes the mouse
                self.active = true;
                self.selected = Some(0);
            }
            Some(_) => {
                if let Some(i) = w.menu_targets.iter().position(|t| t.back) {
                    self.active = true;
                    self.selected = Some(i);
                    self.clicked = true;
                }
            }
            None => {}
        }
        if let Some((x, y)) = self.cursor(w) {
            w.g.m.set_i32(MOUSE_X, x);
            w.g.m.set_i32(MOUSE_Y, y);
        }
        if self.clicked {
            w.g.m.set_i32(MOUSE_CLICK, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(x: i32, y: i32) -> MenuTarget {
        MenuTarget { x0: x - 10, y0: y - 5, x1: x + 10, y1: y + 5, back: false }
    }

    #[test]
    fn next_takes_the_nearest_target_in_the_direction() {
        // a column of three, and one off to the right of the middle one
        let ts = [t(100, 100), t(100, 150), t(100, 200), t(300, 150)];
        assert_eq!(next(&ts, 0, (0, 1)), 1);
        assert_eq!(next(&ts, 1, (0, 1)), 2);
        assert_eq!(next(&ts, 2, (0, 1)), 2, "nothing below the last one");
        assert_eq!(next(&ts, 1, (1, 0)), 3);
        assert_eq!(next(&ts, 3, (-1, 0)), 1);
        assert_eq!(next(&ts, 0, (0, -1)), 0);
    }

    #[test]
    fn next_prefers_a_target_straight_ahead_to_a_nearer_one_off_to_the_side() {
        let ts = [t(100, 100), t(160, 130), t(100, 190)];
        assert_eq!(next(&ts, 0, (0, 1)), 2);
    }

    /// The title screen of the game in LF2_DIR, with the controllers' state written as
    /// the platforms write it each tick.
    struct Title {
        w: World,
        nav: MenuNav,
        joysticks: crate::gamepad::Joysticks,
    }

    impl Title {
        fn new() -> Option<Title> {
            let dir = lf2_data::game_dir_from_env()?;
            let exe = lf2_data::read_exe(&dir).unwrap();
            let gd = std::sync::Arc::new(lf2_data::GameData::load(&dir).unwrap());
            let controls = lf2_data::controls::load(&dir).unwrap_or_default();
            let mut w = World::new_title(gd, &exe, &controls, None, 1);
            w.ads_removed = true;
            let mut t = Title { w, nav: MenuNav::default(), joysticks: Default::default() };
            // the title slides in first
            t.ticks(Joystick::default(), 150);
            assert_eq!(t.screen(), 0);
            Some(t)
        }

        fn ticks(&mut self, j: Joystick, n: usize) {
            for _ in 0..n {
                let pads = [j, Joystick::default()];
                self.w.set_keys(&[b'u'; 256]);
                self.joysticks.write(&mut self.w.g, &pads);
                self.nav.update(&mut self.w, &pads, false);
                self.w.step();
            }
        }

        /// Press and let go, as a person does: a few ticks down, a few up.
        fn tap(&mut self, j: Joystick) {
            self.ticks(j, 3);
            self.ticks(Joystick::default(), 3);
        }

        fn screen(&self) -> i32 {
            self.w.g.m.i32(lf2_sim::menu::title::SCREEN)
        }
    }

    const DOWN: Joystick = Joystick { up: false, down: true, left: false, right: false, buttons: [false; 4] };
    const A: Joystick = Joystick { up: false, down: false, left: false, right: false, buttons: [true, false, false, false] };
    const B: Joystick = Joystick { up: false, down: false, left: false, right: false, buttons: [false, true, false, false] };

    #[test]
    fn the_pad_picks_control_settings_and_b_goes_back() {
        let Some(mut t) = Title::new() else { return };
        // the first press selects "game start", then two more go down two items
        for _ in 0..3 {
            t.tap(DOWN);
        }
        assert_eq!(t.nav.cursor(&t.w).map(|c| c.1), Some(t.w.menu_targets[2].centre().1));
        t.tap(A);
        assert_eq!(t.screen(), 6, "control settings");
        // B is the screen's cancel button
        t.tap(B);
        assert_eq!(t.screen(), 0);
    }

    #[test]
    fn game_start_does_not_also_choose_the_first_mode() {
        let Some(mut t) = Title::new() else { return };
        // the first press of A takes the mouse, the second clicks "game start"
        t.tap(A);
        assert_eq!(t.w.g.m.i32(lf2_sim::world::GAME), 0);
        // held for a quarter of a second, long enough for the mode menu to see it
        t.ticks(A, 8);
        t.ticks(Joystick::default(), 3);
        assert_ne!(t.w.g.m.i32(lf2_sim::world::GAME), 0, "game start");
        t.ticks(Joystick::default(), 20);
        // A is attack for the players on joystick 1 too; the mode menu (phase 10) is
        // still waiting, where taking the same press would have gone on to phase 3
        assert_eq!(t.w.g.match_phase(), 10, "the mode menu took the same press");
        assert!(!t.nav.active(), "the controller has let go of the mouse");
    }

    #[test]
    fn moving_the_mouse_takes_it_back() {
        let Some(mut t) = Title::new() else { return };
        t.tap(DOWN);
        assert!(t.nav.active());
        let pads = [Joystick::default(); 2];
        t.nav.update(&mut t.w, &pads, true);
        assert!(!t.nav.active());
        assert_eq!(t.nav.cursor(&t.w), None);
    }

    #[test]
    fn the_network_menu_is_reached_and_left_with_the_pad() {
        let Some(mut t) = Title::new() else { return };
        t.tap(DOWN);
        t.tap(DOWN);
        t.tap(A);
        assert_eq!(t.screen(), 1, "network game");
        // its first item, "Host a game", is selected; B is "back"
        assert_eq!(t.nav.cursor(&t.w), Some(t.w.menu_targets[0].centre()));
        t.tap(B);
        assert_eq!(t.screen(), 0);
    }
}
