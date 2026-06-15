use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn is_language_key(key: KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::F(1)
            | KeyCode::Char('l')
            | KeyCode::Char('L')
            | KeyCode::Char('д')
            | KeyCode::Char('Д')
    )
}

pub fn edit_text(value: &mut String, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char(ch) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                value.push(ch);
                true
            } else {
                false
            }
        }
        KeyCode::Backspace => {
            value.pop();
            true
        }
        KeyCode::Delete => {
            value.clear();
            true
        }
        _ => false,
    }
}

pub fn move_up(selected: &mut usize, len: usize) {
    if len == 0 {
        *selected = 0;
    } else if *selected == 0 {
        *selected = len - 1;
    } else {
        *selected -= 1;
    }
}

pub fn move_down(selected: &mut usize, len: usize) {
    if len == 0 {
        *selected = 0;
    } else {
        *selected = (*selected + 1) % len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_hotkeys_cover_english_and_russian_layouts() {
        for ch in ['l', 'L', 'д', 'Д'] {
            assert!(is_language_key(KeyEvent::from(KeyCode::Char(ch))));
        }
    }
}
