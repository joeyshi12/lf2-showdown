//! The menus that run inside a match frame while `match_phase` (0x44d020) is not 0:
//! `game_menus` (0x429730) and the screens it dispatches to.

pub mod sprites;
pub mod main_menu;
pub mod pick;
pub mod title;
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
