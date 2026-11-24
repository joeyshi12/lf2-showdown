//! The menus that run inside a match frame while `match_phase` (0x44d020) is not 0:
//! `game_menus` (0x429730) and the screens it dispatches to.

pub mod sprites;
pub mod main_menu;
pub mod pick;
pub mod title;
pub mod net_menu;
pub mod battle_menus;

/// The resource a surface global holds, for the renderer.
pub fn sheet_resource(global: u32) -> Option<&'static str> {
    sprites::SHEETS
        .iter()
        .find(|s| s.global == global)
        .map(|s| s.resource)
        .or_else(|| pick::SHEETS.iter().find(|s| s.0 == global).map(|s| s.1))
        .or_else(|| crate::summary::SHEETS.iter().find(|s| s.0 == global).map(|s| s.1))
        .or_else(|| match global {
            battle_menus::BATTLEMODE => Some("BATTLEMODE"),
            battle_menus::BATTLETROOPS => Some("BATTLETROOPS"),
            _ => None,
        })
}
pub mod champ1;

/// A place on a title screen that the mouse can click: the rectangle the screen's
/// hit test accepts, from (x0, y0) to (x1, y1) inclusive. `back` is the screen's
/// back or cancel button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuTarget {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub back: bool,
}

impl MenuTarget {
    pub fn centre(&self) -> (i32, i32) {
        ((self.x0 + self.x1) / 2, (self.y0 + self.y1) / 2)
    }
}

impl crate::World {
    /// Record a place the mouse can click this frame (see `World::menu_targets`).
    pub(crate) fn target(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, back: bool) {
        self.menu_targets.push(MenuTarget { x0, y0, x1, y1, back });
    }
}
