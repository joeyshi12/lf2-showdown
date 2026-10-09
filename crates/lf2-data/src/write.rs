//! Writers that turn parsed data back into the text format. Parsing what these write
//! gives back an equal value; the tests check that for every file in the game folder.

use crate::bg::Background;
use crate::object::{Itr, ObjectData};
use crate::stage::Stage;
use std::fmt::Write;

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn fmt_f64(v: f64) -> String {
    // `{:?}` prints the shortest text that parses back to the same f64.
    format!("{:?}", v)
}

pub fn write_object(o: &ObjectData) -> Result<String, String> {
    let mut w = String::new();
    w.push_str("<bmp_begin>\n");
    writeln!(w, "name: {}", s(&o.name)).unwrap();
    writeln!(w, "head: {}", s(&o.head)).unwrap();
    if !o.small.is_empty() {
        writeln!(w, "small: {}", s(&o.small)).unwrap();
    }
    for k in 1..=o.sheet_count.min(o.sheets.len() - 1) {
        let sh = &o.sheets[k];
        writeln!(w, "file: {}  w: {}  h: {}  row: {}  col: {}", s(&sh.file), sh.w, sh.h, sh.row, sh.col)
            .unwrap();
    }
    if let Some(v) = o.walking_frame_rate {
        writeln!(w, "walking_frame_rate {}", v).unwrap();
    }
    for (k, v) in [
        ("walking_speed", o.walking_speed),
        ("walking_speedz", o.walking_speedz),
        ("running_speed", o.running_speed),
        ("running_speedz", o.running_speedz),
        ("heavy_walking_speed", o.heavy_walking_speed),
        ("heavy_walking_speedz", o.heavy_walking_speedz),
        ("heavy_running_speed", o.heavy_running_speed),
        ("heavy_running_speedz", o.heavy_running_speedz),
        ("jump_height", o.jump_height),
        ("jump_distance", o.jump_distance),
        ("jump_distancez", o.jump_distancez),
        ("dash_height", o.dash_height),
        ("dash_distance", o.dash_distance),
        ("dash_distancez", o.dash_distancez),
        ("rowing_height", o.rowing_height),
        ("rowing_distance", o.rowing_distance),
    ] {
        if let Some(v) = v {
            writeln!(w, "{} {}", k, fmt_f64(v)).unwrap();
        }
    }
    if let Some(v) = o.running_frame_rate {
        writeln!(w, "running_frame_rate {}", v).unwrap();
    }
    writeln!(w, "weapon_hp: {}", o.weapon_hp).unwrap();
    writeln!(w, "weapon_drop_hurt: {}", o.weapon_drop_hurt).unwrap();
    for (k, v) in [
        ("weapon_hit_sound:", &o.weapon_hit_sound),
        ("weapon_drop_sound:", &o.weapon_drop_sound),
        ("weapon_broken_sound:", &o.weapon_broken_sound),
    ] {
        if let Some(v) = v {
            writeln!(w, "{} {}", k, s(v)).unwrap();
        }
    }
    w.push_str("<bmp_end>\n\n");

    write_strength(&mut w, o)?;

    for (i, f) in o.frames.iter().enumerate() {
        if !f.exists {
            continue;
        }
        writeln!(w, "<frame> {} {}", i, s(&f.name)).unwrap();
        writeln!(
            w,
            "   pic: {} state: {} wait: {} next: {} dvx: {} dvy: {} dvz: {} centerx: {} centery: {}",
            f.pic, f.state, f.wait, f.next, f.dvx, f.dvy, f.dvz, f.centerx, f.centery
        )
        .unwrap();
        writeln!(
            w,
            "   hit_a: {} hit_d: {} hit_j: {} hit_Fa: {} hit_Ua: {} hit_Da: {} hit_Fj: {} hit_Uj: {} hit_Dj: {} hit_ja: {} mp: {}",
            f.hit_a, f.hit_d, f.hit_j, f.hit_fa, f.hit_ua, f.hit_da, f.hit_fj, f.hit_uj, f.hit_dj, f.hit_ja, f.mp
        )
        .unwrap();
        if let Some(snd) = &f.sound {
            writeln!(w, "   sound: {}", s(snd)).unwrap();
        }
        let p = &f.opoint;
        writeln!(
            w,
            "   opoint:\n      kind: {} x: {} y: {} action: {} dvx: {} dvy: {} oid: {} facing: {}\n   opoint_end:",
            p.kind, p.x, p.y, p.action, p.dvx, p.dvy, p.oid, p.facing
        )
        .unwrap();
        writeln!(w, "   bpoint:\n      x: {} y: {}\n   bpoint_end:", f.bpoint.0, f.bpoint.1).unwrap();
        let c = &f.cpoint;
        writeln!(
            w,
            "   cpoint:\n      kind: {} x: {} y: {} injury: {} cover: {} vaction: {} aaction: {} jaction: {} \
             taction: {} daction: {} throwvx: {} throwvy: {} hurtable: {} \
             decrease: {} dircontrol: {}",
            c.kind, c.x, c.y, c.injury, c.cover, c.vaction, c.aaction, c.jaction, c.taction, c.daction,
            c.throwvx, c.throwvy, c.hurtable, c.decrease, c.dircontrol
        )
        .unwrap();
        if let Some(v) = c.throwvz {
            writeln!(w, "      throwvz: {}", v).unwrap();
        }
        if let Some(v) = c.throwinjury {
            writeln!(w, "      throwinjury: {}", v).unwrap();
        }
        w.push_str("   cpoint_end:\n");
        let p = &f.wpoint;
        writeln!(
            w,
            "   wpoint:\n      kind: {} x: {} y: {} weaponact: {} attacking: {} cover: {} dvx: {} dvy: {} dvz: {}\n   wpoint_end:",
            p.kind, p.x, p.y, p.weaponact, p.attacking, p.cover, p.dvx, p.dvy, p.dvz
        )
        .unwrap();
        for i in &f.itrs {
            write_itr(&mut w, i);
        }
        for b in &f.bdys {
            writeln!(w, "   bdy:\n      kind: {} x: {} y: {} w: {} h: {}\n   bdy_end:", b.kind, b.x, b.y, b.w, b.h)
                .unwrap();
        }
        w.push_str("<frame_end>\n\n");
    }
    Ok(w)
}

fn write_itr(w: &mut String, i: &Itr) {
    writeln!(
        w,
        "   itr:\n      kind: {} x: {} y: {} w: {} h: {} dvx: {} dvy: {} fall: {} arest: {} vrest: {} \
         respond: {} effect: {} catchingact: {} {} caughtact: {} {} bdefend: {} injury: {} zwidth: {}\n   itr_end:",
        i.kind, i.x, i.y, i.w, i.h, i.dvx, i.dvy, i.fall, i.arest, i.vrest, i.respond, i.effect,
        i.catchingact[0], i.catchingact[1], i.caughtact[0], i.caughtact[1], i.bdefend, i.injury, i.zwidth
    )
    .unwrap();
}

/// `entry: N name` stores `name` in the slot of the previous entry line (or slot 0 for
/// the first line), so the names are written one line late.
fn write_strength(w: &mut String, o: &ObjectData) -> Result<(), String> {
    let used = |k: usize| {
        let e = &o.strength[k];
        e.itr != Itr::default() || !e.name.is_empty() || (k > 0 && !o.strength[k - 1].name.is_empty())
    };
    let slots: Vec<usize> = (0..o.strength.len()).filter(|&k| used(k)).collect();
    if slots.is_empty() {
        return Ok(());
    }
    w.push_str("<weapon_strength_list>\n");
    let mut prev = 0usize;
    for &k in &slots {
        let name = &o.strength[prev].name;
        if name.is_empty() {
            return Err(format!("strength slot {} has no name but slot {} follows it", prev, k));
        }
        writeln!(w, "  entry: {} {}", k, s(name)).unwrap();
        let e = &o.strength[k].itr;
        writeln!(
            w,
            "    dvx: {} dvy: {} fall: {} arest: {} vrest: {} respond: {} bdefend: {} injury: {} zwidth: {} effect: {}",
            e.dvx, e.dvy, e.fall, e.arest, e.vrest, e.respond, e.bdefend, e.injury, e.zwidth, e.effect
        )
        .unwrap();
        prev = k;
    }
    if !o.strength[prev].name.is_empty() {
        return Err(format!("strength slot {} has a name that no entry line can set", prev));
    }
    w.push_str("<weapon_strength_list_end>\n\n");
    Ok(())
}

pub fn write_background(b: &Background) -> String {
    let mut w = String::new();
    // The loader turns '_' into ' '; spaces cannot appear in a token, so write '_'.
    let name: Vec<u8> = b.name.iter().map(|&c| if c == b' ' { b'_' } else { c }).collect();
    writeln!(w, "name: {}", s(&name)).unwrap();
    writeln!(w, "width: {}", b.width).unwrap();
    writeln!(w, "zboundary: {} {}", b.zboundary.0, b.zboundary.1).unwrap();
    writeln!(w, "perspective: {} {}", b.perspective.0, b.perspective.1).unwrap();
    if !b.shadow.is_empty() {
        writeln!(w, "shadow: {} x {} {}", s(&b.shadow), b.shadow_size.0, b.shadow_size.1).unwrap();
    }
    for l in &b.layers {
        writeln!(
            w,
            "layer: {}\n  transparency: {} width: {} x: {} y: {} height: {} rect32: {} loop: {} cc: {} c1: {} c2: {}\nlayer_end",
            s(&l.file), l.transparency, l.width, l.x, l.y, l.height, l.rect, l.loop_, l.cc, l.c1, l.c2
        )
        .unwrap();
    }
    w
}

pub fn write_stages(stages: &[Stage]) -> String {
    let mut w = String::new();
    for st in stages {
        writeln!(w, "<stage> id: {}", st.id).unwrap();
        for ph in &st.phases {
            w.push_str("  <phase>");
            if ph.bound != -1 || ph.default_x != 500 {
                write!(w, " bound: {}", ph.bound).unwrap();
            }
            w.push('\n');
            if !ph.music.is_empty() {
                writeln!(w, "    music: {}", s(&ph.music)).unwrap();
            }
            if ph.when_clear_goto_phase != -1 {
                writeln!(w, "    when_clear_goto_phase: {}", ph.when_clear_goto_phase).unwrap();
            }
            for sp in &ph.spawns {
                write!(
                    w,
                    "    id: {} x: {} hp: {} reserve: {} join: {} join_reserve: {} ratio: {} y: {} act: {}",
                    sp.id, sp.x, sp.hp, sp.reserve, sp.join, sp.join_reserve, fmt_f64(sp.ratio), sp.y, sp.act
                )
                .unwrap();
                match sp.role {
                    1 => w.push_str(" <soldier>"),
                    2 => w.push_str(" <boss>"),
                    _ => {}
                }
                // `<soldier>` sets times to 50, so times goes last.
                writeln!(w, " times: {}", sp.times).unwrap();
            }
            w.push_str("  <phase_end>\n");
        }
        w.push_str("<stage_end>\n\n");
    }
    w
}
