use tower_lsp::lsp_types::Position;

/// Checks if a character is valid as part of Lane identifiers in source text.
fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Converts an LSP position into a byte offset in UTF-8 source text.
pub(crate) fn byte_offset_for_position(text: &str, position: Position) -> Option<usize> {
    let mut offset = 0;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        let line_text = line.strip_suffix('\n').unwrap_or(line);
        if line_index == position.line as usize {
            return Some(
                offset + byte_offset_for_character(line_text, position.character as usize),
            );
        }
        offset += line.len();
    }
    if position.line as usize == text.lines().count() {
        return Some(text.len());
    }
    None
}

/// Converts LSP UTF-16 character column to byte offset within one line.
fn byte_offset_for_character(line: &str, character: usize) -> usize {
    let mut utf16_units = 0;
    for (index, ch) in line.char_indices() {
        if utf16_units >= character {
            return index;
        }
        utf16_units += ch.len_utf16();
    }
    line.len()
}

/// Returns the identifier token under the cursor, if any.
pub(crate) fn word_at_position(text: &str, position: Position) -> Option<String> {
    let line = text.lines().nth(position.line as usize)?;
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = (position.character as usize).min(chars.len());
    if index == chars.len() && index > 0 {
        index -= 1;
    }
    if index >= chars.len() {
        return None;
    }
    while index > 0 && !is_word_char(chars[index]) && is_word_char(chars[index - 1]) {
        index -= 1;
    }
    if !is_word_char(chars[index]) {
        return None;
    }
    let mut start = index;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = index + 1;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }
    Some(chars[start..end].iter().collect())
}
