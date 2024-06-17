pub trait NewLine {
    fn add_new_line_if_none(&mut self);
}

impl NewLine for String {
    fn add_new_line_if_none(&mut self) {
        if !self.is_empty() && !self.ends_with('\n') {
            self.push('\n');
        }
    }
}
