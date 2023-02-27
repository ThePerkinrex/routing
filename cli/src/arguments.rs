pub struct ArgumentsIter<'a> {
	string: &'a str,
	index: usize
}

impl<'a> ArgumentsIter<'a> {
    pub const fn new(string: &'a str) -> Self { Self { string, index: 0 } }
}

impl<'a> Iterator for ArgumentsIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.string.len() {
			None
		}else{
			let start = self.index;
			// TODO find strings
			let end = self.string.char_indices().skip_while(|(i, _)| *i < start).find(|(_, c)| c== &' ').map(|(i, _)| i).unwrap_or(self.string.len());
			self.index = end+1;
			Some(&self.string[start..end])
		}
    }
}