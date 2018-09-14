pub enum Cmp {
	Equal,
	Less,
	Greater,
}

impl Cmp {
	pub fn cmp<T: Ord>(a: &T, b: &T) -> Self {
		if a == b {
			return Cmp::Equal;
		}
		if a < b {
			return Cmp::Less;
		}
		Cmp::Greater
	}
}
