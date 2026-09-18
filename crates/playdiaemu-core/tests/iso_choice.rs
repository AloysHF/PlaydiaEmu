//! Manual isolation experiments (ignored by default).
//!
//! Requires a local disc path; not run in CI:
//! ```text
//! PLAYDIA_ISO_DISC=path\to\game.zip cargo test -p playdiaemu-core --test iso_choice -- --ignored --nocapture
//! ```

use playdiaemu_core::player::DiscPlayer;
use playdiaemu_core::InputButtons;
use std::path::PathBuf;

fn disc_path() -> PathBuf {
    PathBuf::from(
        std::env::var("PLAYDIA_ISO_DISC").expect("set PLAYDIA_ISO_DISC to a .zip/.cue path"),
    )
}

fn mode_label() -> String {
    "level+aligned".to_string()
}

/// Hold `a` every frame from frame 0. Expect BUTTON clear within 300 frames
/// under level-triggered selection.
#[test]
#[ignore = "manual: set PLAYDIA_ISO_DISC"]
fn hold_a_from_frame_zero() {
    let mut p = DiscPlayer::new();
    p.load_path(&disc_path()).expect("load disc");
    let mut first_wait: Option<u64> = None;
    let mut cleared_at: Option<u64> = None;
    let a = InputButtons {
        a: true,
        ..Default::default()
    };
    for _ in 0..900 {
        let was_waiting = p.is_waiting_for_input();
        p.set_input(a);
        p.run_frame();
        if first_wait.is_none() && p.is_waiting_for_input() {
            first_wait = Some(p.frame);
        }
        if first_wait.is_some() && was_waiting && !p.is_waiting_for_input() && cleared_at.is_none()
        {
            cleared_at = Some(p.frame);
            break;
        }
    }
    let wait = first_wait;
    let cleared = cleared_at;
    let delta = match (wait, cleared) {
        (Some(w), Some(c)) => Some(c as i64 - w as i64),
        _ => None,
    };
    // Button select if cleared within timeout budget; timeout if >= 300.
    let how = match delta {
        Some(d) if d < 0 => "BUG_NEGATIVE_DELTA",
        Some(d) if d < 300 => "BUTTON",
        Some(d) if d >= 300 => "TIMEOUT_OR_LATE",
        None => "NO_MENU_OR_NO_CLEAR",
        _ => "UNKNOWN",
    };
    println!(
        "HOLD_FROM_0 [{}] wait_start={:?} cleared={:?} delta={:?} how={} final_waiting={} interactive={}",
        mode_label(),
        wait,
        cleared,
        delta,
        how,
        p.is_waiting_for_input(),
        p.interactive.len()
    );
    assert!(
        p.video.frames_decoded > 0,
        "disc must decode video for a valid experiment"
    );
}

/// Control: no input until first menu, then tap A for 5 frames.
#[test]
#[ignore = "manual: set PLAYDIA_ISO_DISC"]
fn tap_a_after_menu_opens() {
    let mut p = DiscPlayer::new();
    p.load_path(&disc_path()).expect("load disc");
    let none = InputButtons::default();
    let a = InputButtons {
        a: true,
        ..Default::default()
    };
    // Phase 1: no input until waiting (or 600 frames).
    let mut wait_start = None;
    for _ in 0..600 {
        p.set_input(none);
        p.run_frame();
        if p.is_waiting_for_input() {
            wait_start = Some(p.frame);
            break;
        }
    }
    let Some(ws) = wait_start else {
        println!(
            "TAP_AFTER [{}] no menu within 600 frames — skip",
            mode_label()
        );
        return;
    };
    // Phase 2: still no input for 10 frames (ensure not timeout edge case).
    for _ in 0..10 {
        p.set_input(none);
        p.run_frame();
    }
    // Phase 3: tap A for 5 frames.
    let mut cleared = None;
    for i in 0..5 {
        p.set_input(a);
        p.run_frame();
        if !p.is_waiting_for_input() {
            cleared = Some(p.frame);
            break;
        }
        let _ = i;
    }
    println!(
        "TAP_AFTER [{}] wait_start={} cleared={} delta={} final_waiting={} interactive={}",
        mode_label(),
        ws,
        cleared
            .map(|c| c.to_string())
            .unwrap_or_else(|| "NONE".into()),
        cleared.map(|c| c as i64 - ws as i64).unwrap_or(-1),
        p.is_waiting_for_input(),
        p.interactive.len()
    );
    assert!(
        cleared.is_some(),
        "tap after menu must select under both edge and level modes"
    );
}

/// Same schedule for every button: wait for menu, hold that button 5 frames.
#[test]
#[ignore = "manual: set PLAYDIA_ISO_DISC"]
fn probe_each_button_after_menu() {
    let buttons: [(&str, InputButtons); 7] = [
        (
            "start",
            InputButtons {
                start: true,
                ..Default::default()
            },
        ),
        (
            "up",
            InputButtons {
                up: true,
                ..Default::default()
            },
        ),
        (
            "down",
            InputButtons {
                down: true,
                ..Default::default()
            },
        ),
        (
            "left",
            InputButtons {
                left: true,
                ..Default::default()
            },
        ),
        (
            "right",
            InputButtons {
                right: true,
                ..Default::default()
            },
        ),
        (
            "a",
            InputButtons {
                a: true,
                ..Default::default()
            },
        ),
        (
            "b",
            InputButtons {
                b: true,
                ..Default::default()
            },
        ),
    ];
    for (name, btn) in buttons {
        let mut p = DiscPlayer::new();
        p.load_path(&disc_path()).expect("load disc");
        let none = InputButtons::default();
        let mut wait_start = None;
        for _ in 0..600 {
            p.set_input(none);
            p.run_frame();
            if p.is_waiting_for_input() {
                wait_start = Some(p.frame);
                break;
            }
        }
        if wait_start.is_none() {
            println!("BTN {name} [{}] no menu", mode_label());
            continue;
        }
        let track_before = p.track_index;
        let mut cleared = false;
        for _ in 0..5 {
            p.set_input(btn);
            p.run_frame();
            if !p.is_waiting_for_input() {
                cleared = true;
                break;
            }
        }
        if !cleared {
            // release and let a few more frames, try again once
            for _ in 0..5 {
                p.set_input(none);
                p.run_frame();
            }
            for _ in 0..5 {
                p.set_input(btn);
                p.run_frame();
                if !p.is_waiting_for_input() {
                    cleared = true;
                    break;
                }
            }
        }
        // Advance a bit after select to see if we hit another menu or loop.
        let mut crcs = Vec::new();
        for _ in 0..90 {
            p.set_input(none);
            p.run_frame();
            if p.frame.is_multiple_of(10) {
                crcs.push(p.frame_crc());
            }
        }
        let unique_crc = {
            let mut c = crcs.clone();
            c.sort_unstable();
            c.dedup();
            c.len()
        };
        println!(
            "BTN {name} [{}] cleared={} track {}→{} interactive={} unique_crc={} final_waiting={}",
            mode_label(),
            cleared,
            track_before,
            p.track_index,
            p.interactive.len(),
            unique_crc,
            p.is_waiting_for_input()
        );
    }
}
