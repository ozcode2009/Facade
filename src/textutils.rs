use crate::textutils;

#[derive(Debug, PartialEq,Clone)]
pub enum HtmlToken {
    Word(String),
    HtmlTag {
        name: String,
        is_closing: bool,
    },
    Space,
}

pub fn tokenize_html(input: &str) -> Vec<HtmlToken> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(current) = chars.next() {
        match current {
            // Handle potential HTML tags
            '<' => {
                let mut tag = String::new();
                let mut is_closing = false;

                // Check for different closing tag variations
                if let Some(&next) = chars.peek() {
                    is_closing = match next {
                        '/' | '\\' => true,
                        _ => false
                    };

                    // Consume the closing indicator if present
                    if is_closing {
                        chars.next();
                    }
                }

                // Collect tag name
                while let Some(&c) = chars.peek() {
                    // Stop collecting tag name at closing markers
                    if c == '>' || c == '/' || c == '\\' {
                        chars.next(); // consume the closing marker

                        // Check for potential self-closing tag with />
                        if c == '/' {
                            if let Some(&next) = chars.peek() {
                                if next == '>' {
                                    chars.next(); // consume '>'
                                }
                            }
                        }
                        break;
                    }

                    // Skip whitespace in tag
                    if c.is_whitespace() {
                        chars.next();
                        continue;
                    }

                    tag.push(chars.next().unwrap());
                }

                if !tag.is_empty() {
                    tokens.push(HtmlToken::HtmlTag {
                        name: tag,
                        is_closing,
                    });
                }
            }

            // Handle whitespace
            c if c.is_whitespace() => {
                tokens.push(HtmlToken::Space);
            }

            // Handle words
            _ => {
                let mut word = String::from(current);

                // Collect consecutive non-whitespace, non-tag characters
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || next == '<' {
                        break;
                    }
                    word.push(chars.next().unwrap());
                }

                tokens.push(HtmlToken::Word(word));
            }
        }
    }

    tokens
}
pub fn wrap_html_tokens(tokens: &[HtmlToken], max_line_length: usize) -> Option<Vec<Vec<HtmlToken>>> {
    let mut tokens = Vec::from(tokens);
    let mut lines: Vec<Vec<HtmlToken>> = Vec::new();
    let mut open_tags: Vec<HtmlToken> = Vec::new();

    fn token_length(token: &HtmlToken) -> usize {
        match token {
            HtmlToken::Word(word) => word.len(),
            HtmlToken::Space => 1,
            HtmlToken::HtmlTag { .. } => 0,
        }
    }
    while tokens.len() > 0 && tokens.iter().any(|t| if let HtmlToken::Word(_) = t {true} else {false}) {
        let mut current_line: Vec<HtmlToken> = Vec::new();
        let mut current_line_length = 0;
        for tag in open_tags.iter().rev() {
            if let HtmlToken::HtmlTag {name, is_closing: _is_closing} = tag {
                current_line.push(HtmlToken::HtmlTag {name: name.clone(), is_closing: false});
            }
        }
        while current_line_length < max_line_length && tokens.len() > 0 {
            if let HtmlToken::HtmlTag {name, is_closing } = tokens[0].clone() {
                //If we don't have enough room for the next word, don't insert this tag
                if let Some(HtmlToken::Word(word)) = tokens.iter().find(|x| if let HtmlToken::Word(_) = x { true } else { false }) {
                    if word.len() > (max_line_length - current_line_length) {
                        break;
                    } else {
                        if !is_closing {
                            current_line.push(tokens[0].clone());
                            open_tags.push(tokens[0].clone());
                            tokens.remove(0);
                        } else {
                            if let Some(tag) = open_tags.pop() {
                                if let HtmlToken::HtmlTag {name: name_2, is_closing: _} = tag {
                                    if name == name_2{
                                        current_line.push(tokens[0].clone());
                                        tokens.remove(0);
                                    }else{
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            } else if tokens[0] == HtmlToken::Space {
                //If we don't have enough room for the next word, delete the space
                if let Some(HtmlToken::Word(word)) = tokens.iter().find(|x| if let HtmlToken::Word(_) = x { true } else { false }) {
                    if word.len() + 1 > (max_line_length - current_line_length) {
                        tokens.remove(0);
                        break;
                    } else {
                        current_line.push(tokens[0].clone());
                        tokens.remove(0);
                    }
                } else {
                    break;
                }
            } else {
                current_line.push(tokens[0].clone());
                current_line_length += token_length(&tokens[0]);
                tokens.remove(0);
            }
        }
        for tag in open_tags.iter().rev() {
            if let HtmlToken::HtmlTag {name, is_closing} = tag {
                if !is_closing {
                    current_line.push(HtmlToken::HtmlTag {name: name.clone(), is_closing: true});
                }
            }
        }
        lines.push(current_line);
    }

    Some(lines)
}

pub fn html_tokens_to_string(tokens: Vec<HtmlToken>) -> String {
    let mut tokens = tokens.clone();
    let mut output = String::new();
    while tokens.len() > 0 {
        if let HtmlToken::Word(word) = tokens[0].clone() {
            output.push_str(&word);
            tokens.remove(0);
        } else if tokens[0] == HtmlToken::Space {
            output.push(' ');
            tokens.remove(0);
        } else if let HtmlToken::HtmlTag {name, is_closing} = tokens[0].clone() {
            if is_closing {
                output.push_str(&format!("</{}>", name));
            }else {
                output.push_str(&format!("<{}>", name));
            }
            tokens.remove(0);
        }
    }
    output
}

pub fn hyphenate(html_tokens: &mut Vec<HtmlToken>, max_length: usize){
    for (index, token) in html_tokens.clone().iter().enumerate() {
        if let HtmlToken::Word(word) = token {
            if word.len() > max_length {
                let hyphenated = hyphenate_word(word, max_length);
                html_tokens.splice(index..index+1, hyphenated.into_iter().map(|x| HtmlToken::Word(x)).collect::<Vec<HtmlToken>>());
            }
        }
    }
}
pub fn hyphenate_word(word: &String, max_length: usize) -> Vec<String> {
    if word.len() <= max_length {
        return vec![word.to_string()];
    }

    let mut hyphenated = Vec::new();
    let mut start = 0;

    while start < word.len() {
        let end = (start + max_length).min(word.len());
        let segment = if end < word.len() {
            format!("{}-", &word[start..end])
        } else {
            word[start..end].to_string()
        };
        hyphenated.push(segment);
        start = end;
    }

    hyphenated
}

pub fn generate_centered_text_element(
    text: &str,
    center_x: f64,
    center_y: f64,
    max_chars: usize,
    font_size_pt: f64,
    line_height_factor: f64,
    font_family: &str,
) -> String {
    // Wrap the text (this needs to account for the tags in the wrapping process)
    //let wrapped_lines = wrap_text(text, max_chars);
    let mut wrapped_lines: Vec<String> = Vec::new();
    let mut tokens = textutils::tokenize_html(text);
    hyphenate(&mut tokens, max_chars);
    for line in textutils::wrap_html_tokens(&tokens, max_chars).unwrap() {
        wrapped_lines.push(html_tokens_to_string(line));
    }
    let total_lines = wrapped_lines.len();

    // Convert point size to mm for consistent spacing
    // Approximate conversion: 1pt ≈ 0.35mm
    let font_size_mm = font_size_pt * 0.35;

    // Calculate line height in mm
    let line_height_mm = font_size_mm * line_height_factor;

    // Generate style attribute (keep font-size in pt as it's standard for SVG text)
    let style = format!("font-size:{}pt;font-family:{};text-anchor:middle", font_size_pt, font_family);

    // Start text element
    let mut text_element = format!("<text x=\"{}mm\" y=\"{}mm\" style=\"{}\">\n  ", center_x, center_y, style);

    // Generate tspan elements with proper spacing in mm
    for (i, line) in wrapped_lines.iter().enumerate() {
        let dy = if i == 0 {
            // First line positioning - offset upward by half the total height
            -((total_lines - 1) as f64 * line_height_mm / 2.0)
        } else {
            // Subsequent lines spaced by line_height
            line_height_mm
        };


        // Process the line for underline tags
        if line.contains("<u>") || line.contains("</u>") {
            // Start the tspan for this line
            let mut line_tspan = format!("<tspan x=\"{}mm\" dy=\"{}mm\">", center_x, dy);

            // Split the text at underline tags
            let mut parts: Vec<(String, bool)> = Vec::new();
            let mut is_underlined = false;
            let mut current_text = String::new();

            let mut j = 0;
            let line_chars: Vec<char> = line.chars().collect();

            while j < line_chars.len() {
                if j + 2 < line_chars.len() &&
                    line_chars[j] == '<' &&
                    line_chars[j+1] == 'u' &&
                    line_chars[j+2] == '>' {
                    if !current_text.is_empty() {
                        parts.push((current_text, is_underlined));
                        current_text = String::new();
                    }
                    is_underlined = true;
                    j += 3;
                } else if j + 3 < line_chars.len() &&
                    line_chars[j] == '<' &&
                    line_chars[j+1] == '/' &&
                    line_chars[j+2] == 'u' &&
                    line_chars[j+3] == '>' {
                    if !current_text.is_empty() {
                        parts.push((current_text, is_underlined));
                        current_text = String::new();
                    }
                    is_underlined = false;
                    j += 4;
                } else {
                    current_text.push(line_chars[j]);
                    j += 1;
                }
            }

            // Add any remaining text
            if !current_text.is_empty() {
                parts.push((current_text, is_underlined));
            }

            // Create nested tspans for each part
            for (text_part, underlined) in parts {
                if underlined {
                    line_tspan.push_str(&format!("<tspan text-decoration=\"underline\">{}</tspan>", text_part));
                } else {
                    line_tspan.push_str(&text_part);
                }
            }

            line_tspan.push_str("</tspan>\n  ");
            text_element.push_str(&line_tspan);
        } else {
            // No underline tags in this line, add it normally
            text_element.push_str(&format!("<tspan x=\"{}mm\" dy=\"{}mm\">{}</tspan>\n  ", center_x, dy, line));
        }
    }

    text_element.push_str("</text>");

    text_element
}