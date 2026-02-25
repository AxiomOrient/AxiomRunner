pub fn hex_encode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }

    out
}

pub fn hex_decode(input: &str) -> Result<String, String> {
    if !input.len().is_multiple_of(2) {
        return Err(format!("hex value has odd length: {input}"));
    }

    let mut bytes = Vec::with_capacity(input.len() / 2);
    let mut chars = input.chars();
    while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
        let high = hex_nibble(high)?;
        let low = hex_nibble(low)?;
        bytes.push((high << 4) | low);
    }

    String::from_utf8(bytes).map_err(|error| format!("invalid utf8 payload: {error}"))
}

fn hex_nibble(ch: char) -> Result<u8, String> {
    match ch {
        '0'..='9' => Ok(ch as u8 - b'0'),
        'a'..='f' => Ok(ch as u8 - b'a' + 10),
        'A'..='F' => Ok(ch as u8 - b'A' + 10),
        _ => Err(format!("invalid hex character '{ch}'")),
    }
}
