use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const STOPS: [Rgb; 5] = [
    Rgb(0x8A, 0x2B, 0xE2),
    Rgb(0x7B, 0x68, 0xEE),
    Rgb(0x93, 0x70, 0xDB),
    Rgb(0x1A, 0x09, 0x33),
    Rgb(0x0D, 0x02, 0x21),
];

pub fn lerp(a: Rgb, b: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    Rgb(
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * amount).round() as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * amount).round() as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * amount).round() as u8,
    )
}

pub fn gradient(count: usize) -> Vec<Rgb> {
    if count == 0 {
        return Vec::new();
    }
    (0..count)
        .map(|index| {
            let position = if count == 1 {
                0.0
            } else {
                index as f32 / (count - 1) as f32
            };
            let scaled = position * (STOPS.len() - 1) as f32;
            let segment = (scaled.floor() as usize).min(STOPS.len() - 2);
            lerp(STOPS[segment], STOPS[segment + 1], scaled - segment as f32)
        })
        .collect()
}

fn nearest_ansi256(rgb: Rgb) -> u8 {
    let levels = [0u8, 95, 135, 175, 215, 255];
    let level = |value: u8| {
        levels
            .iter()
            .enumerate()
            .min_by_key(|(_, candidate)| (**candidate as i16 - value as i16).abs())
            .map(|(index, _)| index as u8)
            .unwrap_or(0)
    };
    16 + 36 * level(rgb.0) + 6 * level(rgb.1) + level(rgb.2)
}

pub fn color(rgb: Rgb) -> Color {
    if std::env::var_os("NO_COLOR").is_some() {
        return Color::Reset;
    }
    let color_term = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if color_term == "truecolor" || color_term == "24bit" {
        Color::Rgb(rgb.0, rgb.1, rgb.2)
    } else if term.contains("256color") {
        Color::Indexed(nearest_ansi256(rgb))
    } else if console::colors_enabled() {
        Color::Indexed(nearest_ansi256(rgb) % 16)
    } else {
        Color::Reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lerp_reaches_endpoints() {
        assert_eq!(lerp(Rgb(0, 0, 0), Rgb(10, 20, 30), 0.0), Rgb(0, 0, 0));
        assert_eq!(lerp(Rgb(0, 0, 0), Rgb(10, 20, 30), 1.0), Rgb(10, 20, 30));
    }
    #[test]
    fn gradient_is_ordered() {
        let values = gradient(5);
        assert_eq!(values.first(), Some(&STOPS[0]));
        assert_eq!(values.last(), Some(&STOPS[4]));
    }
}
