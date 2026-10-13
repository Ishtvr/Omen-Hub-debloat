//! Shared, testable keyboard editing. Platform code only decodes native keys.
use crate::curve_editor::{Curve, DragMode, Kind};

#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}
#[derive(Debug, Clone, Copy)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    All,
    Undo,
    Redo,
    Flatten,
    Reset,
    Insert,
    Delete,
    Escape,
}
pub fn edit(curve: &mut Curve, key: Key, m: Modifiers) -> Result<bool, String> {
    let gpu = curve.kind == Kind::Gpu;
    match key {
        Key::Undo => return Ok(curve.undo()),
        Key::Redo => return Ok(curve.redo()),
        Key::Escape => {
            let changed = curve.cancel_gesture();
            curve.clear_selection();
            return Ok(changed);
        }
        Key::Reset => {
            let before = curve.points.clone();
            curve.reset();
            return Ok(before != curve.points);
        }
        Key::All if gpu => {
            curve.select_all();
            return Ok(false);
        }
        Key::Flatten if gpu => return curve.flatten_selection(),
        Key::Home => {
            curve.select(0, m.shift && gpu);
            return Ok(false);
        }
        Key::End => {
            curve.select(curve.points.len() - 1, m.shift && gpu);
            return Ok(false);
        }
        Key::Left | Key::Right if !m.ctrl || gpu => {
            let i = if matches!(key, Key::Left) {
                curve.selected.saturating_sub(1)
            } else {
                (curve.selected + 1).min(curve.points.len() - 1)
            };
            curve.select(i, m.shift && gpu);
            return Ok(false);
        }
        Key::Insert if !gpu => {
            curve.insert()?;
            return Ok(true);
        }
        Key::Delete if !gpu => {
            curve.remove()?;
            return Ok(true);
        }
        _ => {}
    }
    let step = if (!gpu && m.shift) || (m.ctrl && matches!(key, Key::Up | Key::Down)) {
        10
    } else {
        1
    };
    let mut p = curve.points[curve.selected];
    match key {
        Key::Up | Key::Down => {
            let delta = if matches!(key, Key::Up) { step } else { -step };
            if gpu && (m.shift || m.alt || curve.selection().is_some()) {
                let mode = if m.shift {
                    DragMode::ReferenceOffset
                } else {
                    DragMode::CurrentOffset
                };
                return curve.drag_gpu(&curve.points.clone(), mode, delta);
            }
            p.y = p.y.saturating_add(delta);
        }
        Key::Left if !gpu => p.x = p.x.saturating_sub(step),
        Key::Right if !gpu => p.x = p.x.saturating_add(step),
        _ => return Ok(false),
    }
    curve.set(curve.selected, p, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fan_keyboard_and_history_are_independent_of_other_curves() {
        let mut cpu = Curve::fan(Kind::CpuFan, None).unwrap();
        let gpu = Curve::fan(Kind::GpuFan, None).unwrap();
        let original = cpu.points.clone();
        edit(
            &mut cpu,
            Key::Up,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        )
        .unwrap();
        assert_eq!(cpu.points[0].y, 40);
        edit(&mut cpu, Key::Insert, Modifiers::default()).unwrap();
        assert_eq!(cpu.points.len(), 5);
        assert!(edit(&mut cpu, Key::Undo, Modifiers::default()).unwrap());
        assert_eq!(cpu.points.len(), 4);
        assert!(edit(&mut cpu, Key::Undo, Modifiers::default()).unwrap());
        assert_eq!(cpu.points, original);
        assert_eq!(gpu.points, original);
        assert!(edit(&mut cpu, Key::Redo, Modifiers::default()).unwrap());
        cpu.value().unwrap();
    }
}
