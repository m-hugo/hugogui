struct TextInput {
	txt: String,
}

impl TextInput {
	fn input(&mut self, s: &str) {
		if s == "\u{8}" {
			self.txt.pop();
			return;
		}
		self.txt += s;
	}
	fn new() -> Self {
		TextInput { txt: String::new() }
	}
}

fn main() {
	let mut ti = TextInput::new();
	ti.input("a");
	ti.input("b");
	ti.input("\u{8}");
	println!("{}", ti.txt);
}
