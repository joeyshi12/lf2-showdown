//! A player whose input kind in the control settings is a joystick plays from the
//! joysticks' state the platform writes with `input::set_joystick`, in a match and in
//! a network game's local input.
use lf2_sim::input::{set_joystick, Joystick};
use lf2_sim::setup::{Fighter, VsSetup, DEFAULT_CONTROLS};
use lf2_sim::world::CONTROLS;
use lf2_sim::World;
use std::sync::Arc;

/// A VS match of two humans, player 1 on joystick 1 with attack, jump and defend on
/// buttons 0, 1 and 3.
fn match_on_joystick() -> Option<World> {
    let dir = lf2_data::game_dir_from_env()?;
    let exe = lf2_data::read_exe(&dir).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let davis = gd.objects.iter().position(|o| o.kind == 0 && o.name.eq_ignore_ascii_case(b"Davis")).unwrap();
    let setup = VsSetup {
        fighters: vec![
            Fighter { slot: 0, data: davis, team: 0, human: true, pos: None },
            Fighter { slot: 1, data: davis, team: 0, human: true, pos: None },
        ],
        background: 0,
        difficulty: 2,
        seed: 1,
    };
    let mut w = World::new_vs_match(gd, &exe, &setup, &DEFAULT_CONTROLS);
    let base = CONTROLS + 0x50;
    w.g.m.set_i32(base, 1);
    for (at, button) in [(0x20, 0), (0x24, 1), (0x28, 3)] {
        w.g.m.set_i32(base + at, button);
    }
    w.set_keys(&[b'u'; 256]);
    Some(w)
}

fn step(w: &mut World, j: Joystick, ticks: usize) {
    for _ in 0..ticks {
        set_joystick(&mut w.g, 0, &j);
        w.step();
    }
}

#[test]
fn joystick_right_walks_player_1_right() {
    let Some(mut w) = match_on_joystick() else { return };
    step(&mut w, Joystick::default(), 10);
    let x0 = w.objs[0].x();
    step(&mut w, Joystick { right: true, ..Default::default() }, 20);
    assert!(w.objs[0].x() > x0 + 20.0, "x went from {} to {}", x0, w.objs[0].x());
    // player 2 is on the keyboard, which nobody pressed
    assert_eq!(w.objs[1].right(), 0);
}

#[test]
fn joystick_buttons_are_the_ones_control_txt_chose() {
    let Some(mut w) = match_on_joystick() else { return };
    step(&mut w, Joystick::default(), 10);
    // button 3 is defend, button 2 is nothing
    step(&mut w, Joystick { buttons: [false, false, true, false], ..Default::default() }, 2);
    assert_eq!(w.objs[0].m.u8(0xd3), 0, "button 2 pressed defend");
    step(&mut w, Joystick { buttons: [false, false, false, true], ..Default::default() }, 2);
    assert_eq!(w.objs[0].m.u8(0xd3), 1, "button 3 did not press defend");
}

#[test]
fn network_input_of_a_joystick_player() {
    let Some(mut w) = match_on_joystick() else { return };
    let mut keys = [b'u'; 256];
    set_joystick(&mut w.g, 0, &Joystick { left: true, buttons: [true, false, false, false], ..Default::default() });
    // left (0x20) and attack (8), as up down left right attack jump defend
    assert_eq!(lf2_sim::net::local_input(&w, &mut keys)[0], 0x20 | 8);
    // the keyboard keys of player 1 do nothing while it is on a joystick
    set_joystick(&mut w.g, 0, &Joystick::default());
    keys[DEFAULT_CONTROLS[0][4] as usize] = b'd';
    assert_eq!(lf2_sim::net::local_input(&w, &mut keys)[0], 0);
}
