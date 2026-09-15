use playdia_core::InputButtons;

const IDLE: u32 = 0x00404040;
const PRESSED: u32 = 0x0000d8ff;
const ACTION_PRESSED: u32 = 0x00ff9f1a;

pub fn draw(buffer: &mut [u32], width: usize, height: usize, buttons: InputButtons) {
    if width == 0 || height == 0 || buffer.len() < width.saturating_mul(height) {
        return;
    }
    let unit = (width.min(height) / 48).clamp(1, 6);
    let dpad_x = unit * 6;
    let dpad_y = height.saturating_sub(unit * 7);
    button(
        buffer,
        width,
        height,
        dpad_x,
        dpad_y.saturating_sub(unit * 3),
        unit * 2,
        buttons.up,
        PRESSED,
    );
    button(
        buffer,
        width,
        height,
        dpad_x,
        dpad_y + unit * 3,
        unit * 2,
        buttons.down,
        PRESSED,
    );
    button(
        buffer,
        width,
        height,
        dpad_x - unit * 3,
        dpad_y,
        unit * 2,
        buttons.left,
        PRESSED,
    );
    button(
        buffer,
        width,
        height,
        dpad_x + unit * 3,
        dpad_y,
        unit * 2,
        buttons.right,
        PRESSED,
    );

    let action_x = width.saturating_sub(unit * 6);
    let action_y = height.saturating_sub(unit * 7);
    button(
        buffer,
        width,
        height,
        action_x,
        action_y + unit * 3,
        unit * 2,
        buttons.b,
        ACTION_PRESSED,
    );
    button(
        buffer,
        width,
        height,
        action_x + unit * 3,
        action_y,
        unit * 2,
        buttons.a,
        ACTION_PRESSED,
    );

    let system_y = height.saturating_sub(unit * 3);
    button(
        buffer,
        width,
        height,
        (width / 2).saturating_sub(unit * 4),
        system_y,
        unit * 2,
        buttons.select,
        PRESSED,
    );
    button(
        buffer,
        width,
        height,
        width / 2 + unit * 4,
        system_y,
        unit * 2,
        buttons.start,
        PRESSED,
    );
}

#[allow(clippy::too_many_arguments)]
fn button(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    size: usize,
    pressed: bool,
    pressed_color: u32,
) {
    let color = if pressed { pressed_color } else { IDLE };
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;
            if px < width && py < height {
                buffer[py * width + px] = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_is_safe_for_tiny_buffers() {
        let mut buffer = vec![0u32; 2 * 2];
        draw(
            &mut buffer,
            2,
            2,
            InputButtons {
                a: true,
                ..Default::default()
            },
        );
    }
}
